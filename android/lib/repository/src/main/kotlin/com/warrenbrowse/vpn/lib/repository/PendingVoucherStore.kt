package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import co.touchlab.kermit.Logger
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

/** Where one sealed blob lives; `null` means none. */
interface BlobSlot {
    fun read(): ByteArray?

    fun write(blob: ByteArray?)
}

/** The pending vouchers as one sealed blob, rewritten whole on every change. */
class SealedPendingVoucherStore(private val sealer: BlobSealer, private val slot: BlobSlot) :
    PendingVoucherStore {

    @Synchronized override fun all(): List<PendingVoucher> = load()

    @Synchronized
    override fun hold(pending: PendingVoucher) {
        val held = load()
        if (pending !in held) {
            save(held + pending)
        }
    }

    @Synchronized
    override fun forget(voucher: String) {
        val held = load()
        val kept = held.filterNot { it.voucher == voucher }
        if (kept.size != held.size) {
            save(kept)
        }
    }

    private fun load(): List<PendingVoucher> {
        val blob = slot.read() ?: return emptyList()
        return try {
            decode(sealer.open(blob))
        } catch (e: GeneralSecurityException) {
            // A key the system dropped (a lock-screen reset, a restore on another device) cannot
            // open what it sealed. The class name only: the message could quote the input.
            Logger.w { "Pending vouchers unreadable (${e::class.simpleName})" }
            emptyList()
        } catch (e: IllegalArgumentException) {
            Logger.w { "Pending vouchers unreadable (${e::class.simpleName})" }
            emptyList()
        }
    }

    private fun save(held: List<PendingVoucher>) {
        slot.write(if (held.isEmpty()) null else sealer.seal(encode(held)))
    }

    // One `wallet:voucher` line per voucher, each field base64 so no separator can collide.
    private fun encode(held: List<PendingVoucher>): ByteArray =
        held
            .joinToString("\n") { "${b64(it.wallet)}:${b64(it.voucher)}" }
            .toByteArray(Charsets.UTF_8)

    private fun decode(plain: ByteArray): List<PendingVoucher> =
        try {
            String(plain, Charsets.UTF_8).lines().mapNotNull { line ->
                val fields = line.split(':')
                if (fields.size == 2) PendingVoucher(unb64(fields[0]), unb64(fields[1])) else null
            }
        } finally {
            plain.fill(0)
        }

    private fun b64(value: String): String =
        java.util.Base64.getEncoder().encodeToString(value.toByteArray(Charsets.UTF_8))

    private fun unb64(value: String): String =
        String(java.util.Base64.getDecoder().decode(value), Charsets.UTF_8)
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
 * A blob in the app's private preferences, beside the wallet's ciphertext. The app sets
 * `allowBackup=false`, so it stays on the device.
 */
class SharedPreferencesBlobSlot(private val prefs: SharedPreferences, private val key: String) :
    BlobSlot {
    constructor(
        context: Context
    ) : this(context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE), BLOB_KEY)

    override fun read(): ByteArray? =
        prefs.getString(key, null)?.let {
            try {
                Base64.decode(it, Base64.NO_WRAP)
            } catch (e: IllegalArgumentException) {
                Logger.w { "Pending vouchers blob malformed (${e::class.simpleName})" }
                null
            }
        }

    override fun write(blob: ByteArray?) {
        // commit, not apply: a voucher held for a redemption about to be sent must be on disk
        // before the request leaves.
        val editor = prefs.edit()
        if (blob == null) editor.remove(key) else editor.putString(key, Base64.encodeToString(blob, Base64.NO_WRAP))
        editor.commit()
    }

    private companion object {
        const val PREFS_NAME = "warren_pending_vouchers"
        const val BLOB_KEY = "sealed"
    }
}
