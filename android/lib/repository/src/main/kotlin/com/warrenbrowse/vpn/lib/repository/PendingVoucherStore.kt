package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import co.touchlab.kermit.Logger
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * A voucher an app-initiated purchase paid for, pulled from the server and not redeemed yet,
 * with the [wallet] (its SS58 address) it was bought for. A redemption credits whichever wallet
 * signs it, so a held voucher is only ever redeemed for its own.
 */
data class PendingVoucher(val wallet: String, val voucher: String) {
    // The voucher is a bearer secret: it must never reach a log line through interpolation.
    override fun toString(): String = "PendingVoucher([redacted])"
}

/**
 * Where a pulled voucher waits for its redemption (warren-core doc 105 section 6). The server
 * hands a voucher out once, and a ban refuses its redemption for up to a year while leaving it
 * unspent, so this is the only lasting copy of a paid secret: it is sealed at rest, and a voucher
 * leaves it only once redeemed or refused for good.
 */
interface PendingVoucherStore {
    fun all(): List<PendingVoucher>

    fun hold(pending: PendingVoucher)

    fun forget(voucher: String)
}

/** Seals bytes with a key the platform keeps out of the app's reach. */
interface BlobSealer {
    fun seal(plain: ByteArray): ByteArray

    /** @throws GeneralSecurityException when the blob was not sealed by this key. */
    fun open(sealed: ByteArray): ByteArray
}

/** Where the sealed blobs live, each under a name of its own. */
interface BlobShelf {
    fun all(): Map<String, ByteArray>

    /** @throws IOException when the blob did not reach the disk. */
    fun put(name: String, blob: ByteArray)

    fun remove(name: String)
}

/**
 * The pending vouchers, each sealed on its own under a random name. A blob this run cannot open
 * (a Keystore that failed once, a key dropped for good) is left exactly where it is: it is not
 * listed, and no other voucher's write or removal touches it.
 */
class SealedPendingVoucherStore(private val sealer: BlobSealer, private val shelf: BlobShelf) :
    PendingVoucherStore {

    @Synchronized override fun all(): List<PendingVoucher> = opened().values.toList()

    /** @throws IOException when the voucher did not reach the disk. */
    @Synchronized
    override fun hold(pending: PendingVoucher) {
        if (pending !in opened().values) {
            shelf.put(newName(), sealer.seal(encode(pending)))
        }
    }

    @Synchronized
    override fun forget(voucher: String) {
        opened().filterValues { it.voucher == voucher }.keys.forEach(shelf::remove)
    }

    private fun opened(): Map<String, PendingVoucher> =
        shelf.all().mapNotNull { (name, blob) -> open(blob)?.let { name to it } }.toMap()

    private fun open(blob: ByteArray): PendingVoucher? =
        try {
            decode(sealer.open(blob))
        } catch (e: GeneralSecurityException) {
            // The class name only: the message could quote the input.
            Logger.w { "A pending voucher is unreadable (${e::class.simpleName})" }
            null
        } catch (e: IllegalArgumentException) {
            Logger.w { "A pending voucher is unreadable (${e::class.simpleName})" }
            null
        }

    // `wallet:voucher`, each field base64 so no separator can collide.
    private fun encode(pending: PendingVoucher): ByteArray =
        "${b64(pending.wallet)}:${b64(pending.voucher)}".toByteArray(Charsets.UTF_8)

    private fun decode(plain: ByteArray): PendingVoucher? =
        try {
            val fields = String(plain, Charsets.UTF_8).split(':')
            if (fields.size == 2) PendingVoucher(unb64(fields[0]), unb64(fields[1])) else null
        } finally {
            plain.fill(0)
        }

    private fun newName(): String =
        ByteArray(NAME_BYTES).also { random.nextBytes(it) }.joinToString("") { "%02x".format(it) }

    private fun b64(value: String): String =
        java.util.Base64.getEncoder().encodeToString(value.toByteArray(Charsets.UTF_8))

    private fun unb64(value: String): String =
        String(java.util.Base64.getDecoder().decode(value), Charsets.UTF_8)

    private companion object {
        const val NAME_BYTES = 16
        val random = java.security.SecureRandom()
    }
}

/**
 * The sealer of the wallet's mnemonic ([AndroidKeystoreWalletRepository]): AES-256-GCM under a
 * key generated inside the Android Keystore that never leaves it, a fresh nonce ahead of every
 * ciphertext. Its own alias, so erasing the wallet (which deletes the wallet's key) cannot make a
 * paid voucher unreadable: the same wallet imported again still redeems it.
 */
class AndroidKeystoreBlobSealer(private val alias: String = KEY_ALIAS) : BlobSealer {
    override fun seal(plain: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, key()) }
        try {
            return cipher.iv + cipher.doFinal(plain)
        } finally {
            plain.fill(0)
        }
    }

    override fun open(sealed: ByteArray): ByteArray {
        if (sealed.size <= IV_BYTES) throw GeneralSecurityException("blob too short")
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(GCM_TAG_BITS, sealed, 0, IV_BYTES))
        return cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES)
    }

    private fun key(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        keyStore.getKey(alias, null)?.let { return it as SecretKey }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(
                    alias,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(KEY_SIZE_BITS)
                .build()
        )
        return generator.generateKey()
    }

    private companion object {
        const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        const val KEY_ALIAS = "warren_pending_vouchers_v1"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val KEY_SIZE_BITS = 256
        const val GCM_TAG_BITS = 128
        const val IV_BYTES = 12
    }
}

/**
 * The blobs in the app's own private preferences file, beside the wallet's ciphertext. The app
 * sets `allowBackup=false` and excludes its preferences from device transfers, so they stay on
 * the device.
 */
class SharedPreferencesBlobShelf(private val prefs: SharedPreferences) : BlobShelf {
    constructor(context: Context) : this(context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE))

    override fun all(): Map<String, ByteArray> =
        prefs.all.mapNotNull { (name, value) ->
            val blob =
                (value as? String)?.let {
                    try {
                        Base64.decode(it, Base64.NO_WRAP)
                    } catch (e: IllegalArgumentException) {
                        Logger.w { "A pending voucher blob is malformed (${e::class.simpleName})" }
                        null
                    }
                }
            blob?.let { name to it }
        }.toMap()

    // commit, not apply: a voucher held for a redemption about to be sent must be on disk before
    // the request leaves.
    override fun put(name: String, blob: ByteArray) {
        if (!prefs.edit().putString(name, Base64.encodeToString(blob, Base64.NO_WRAP)).commit()) {
            throw IOException("the pending voucher did not reach the disk")
        }
    }

    override fun remove(name: String) {
        prefs.edit().remove(name).commit()
    }

    private companion object {
        const val PREFS_NAME = "warren_pending_vouchers"
    }
}
