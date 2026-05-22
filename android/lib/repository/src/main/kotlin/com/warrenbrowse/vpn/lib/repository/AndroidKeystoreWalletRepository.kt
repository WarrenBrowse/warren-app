package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletPubkeyHex
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * Production [WalletRepository] impl: persists the BIP39 mnemonic encrypted
 * by an Android Keystore-bound AES-256-GCM master key.
 *
 * Storage layout (`SharedPreferences` named "warren_wallet"):
 *  - `mnemonic_ciphertext` (base64): AES-GCM ciphertext of the mnemonic
 *    UTF-8 bytes.
 *  - `mnemonic_iv` (base64): the 12-byte GCM nonce used at encryption
 *    time. GCM requires a fresh IV per encrypt; we store it alongside
 *    the ciphertext rather than re-deriving it (deterministic IVs in GCM
 *    are a foot-gun).
 *  - `pubkey_hex` (string): the 64-char lowercase hex pubkey, cached so
 *    the UI can display the wallet identifier without decrypting the
 *    mnemonic at every cold start.
 *
 * The Keystore master key is named "warren_wallet_master_v1"; it is
 * generated once on first use with `purposes = ENCRYPT | DECRYPT`,
 * `blockModes = GCM`, `paddings = NoPadding`. The key never leaves the
 * Keystore: encrypt / decrypt operations happen inside the secure
 * subsystem (hardware-backed on devices that support it).
 *
 * D.5 deferred items (documented as TODO inline):
 *   - `BiometricPrompt` gating around [unlock] + [createWallet] backup
 *     view (currently no biometric gate - any caller in the same
 *     process can decrypt the mnemonic).
 *   - Tamper-evidence: a MAC over `(pubkey_hex, ciphertext, iv)` so a
 *     swapped-out wallet file is detected at boot.
 *   - `setUserAuthenticationRequired(true)` on the master key spec once
 *     BiometricPrompt is in place.
 */
class AndroidKeystoreWalletRepository(context: Context) : WalletRepository {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    private val _state = MutableStateFlow<WalletState>(loadInitialState())
    override val state: StateFlow<WalletState> = _state.asStateFlow()

    private val lock = Mutex()

    override suspend fun createWallet(): Mnemonic = lock.withLock {
        withContext(Dispatchers.IO) {
            val phrase = WarrenJni.generateMnemonic()
            check(phrase.isNotEmpty()) { "WarrenJni.generateMnemonic returned empty string" }
            val mnemonic = Mnemonic(phrase)
            persist(mnemonic)
            mnemonic
        }
    }

    override suspend fun importWallet(mnemonic: Mnemonic): WalletPubkeyHex = lock.withLock {
        withContext(Dispatchers.IO) {
            val hex = WarrenJni.mnemonicPubkeyHex(mnemonic.phrase)
            persist(mnemonic, hex)
            WalletPubkeyHex(hex)
        }
    }

    override suspend fun unlock(
        authorizer: SensitiveOpAuthorizer,
        reason: String,
    ): Mnemonic = lock.withLock {
        // Gate the cleartext read behind the user's biometric / device
        // credential prompt. The repository never owns the prompt UI -
        // the `authorizer` is supplied by the UI layer (typically
        // `lib/ui/component/wallet/BiometricGate.promptBiometric`).
        if (!authorizer.authorize(reason)) {
            throw WalletAuthorizationDeniedException(
                "User declined or device cannot authenticate"
            )
        }
        withContext(Dispatchers.IO) {
            val phrase = decryptMnemonic()
                ?: throw IllegalStateException(
                    "no wallet on disk - call createWallet/importWallet first"
                )
            Mnemonic(phrase)
        }
    }

    override suspend fun erase() = lock.withLock {
        withContext(Dispatchers.IO) {
            prefs.edit().clear().apply()
            try {
                val ks = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
                if (ks.containsAlias(KEY_ALIAS)) ks.deleteEntry(KEY_ALIAS)
            } catch (e: Exception) {
                Logger.w(throwable = e) { "Keystore entry deletion failed (continuing)" }
            }
            _state.value = WalletState.Absent
        }
    }

    // ----------------------------------------------------------------------

    private fun loadInitialState(): WalletState {
        val hex = prefs.getString(KEY_PUBKEY_HEX, null) ?: return WalletState.Absent
        return try {
            WalletState.Locked.also {
                // We don't decrypt at boot - UI must call `unlock()` to
                // get the cleartext mnemonic for a signing operation.
                @Suppress("UNUSED_EXPRESSION") WalletPubkeyHex(hex)
            }
        } catch (e: IllegalArgumentException) {
            Logger.w(throwable = e) { "persisted pubkey is malformed; treating wallet as absent" }
            WalletState.Absent
        }
    }

    private fun persist(mnemonic: Mnemonic, pubkeyHex: String? = null) {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.ENCRYPT_MODE, getOrCreateMasterKey())
        }
        val ciphertext = cipher.doFinal(mnemonic.phrase.toByteArray(Charsets.UTF_8))
        val iv = cipher.iv

        val hex = pubkeyHex ?: WarrenJni.mnemonicPubkeyHex(mnemonic.phrase)

        prefs.edit()
            .putString(KEY_CIPHERTEXT, Base64.encodeToString(ciphertext, Base64.NO_WRAP))
            .putString(KEY_IV, Base64.encodeToString(iv, Base64.NO_WRAP))
            .putString(KEY_PUBKEY_HEX, hex)
            .apply()

        _state.value = WalletState.Ready(WalletPubkeyHex(hex))
    }

    private fun decryptMnemonic(): String? {
        val ciphertextB64 = prefs.getString(KEY_CIPHERTEXT, null) ?: return null
        val ivB64 = prefs.getString(KEY_IV, null) ?: return null
        val ciphertext = Base64.decode(ciphertextB64, Base64.NO_WRAP)
        val iv = Base64.decode(ivB64, Base64.NO_WRAP)
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, getOrCreateMasterKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
        }
        val plaintext = cipher.doFinal(ciphertext)
        return String(plaintext, Charsets.UTF_8)
    }

    private fun getOrCreateMasterKey(): SecretKey {
        val ks = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        ks.getKey(KEY_ALIAS, null)?.let { return it as SecretKey }

        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        val spec = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(KEY_SIZE_BITS)
            // TODO (D.5 step 2): setUserAuthenticationRequired(true) +
            //   setUserAuthenticationParameters once BiometricPrompt is
            //   wired. Until then any caller in this process can decrypt.
            .build()
        gen.init(spec)
        return gen.generateKey()
    }

    private companion object {
        const val PREFS_NAME = "warren_wallet"
        const val KEY_CIPHERTEXT = "mnemonic_ciphertext"
        const val KEY_IV = "mnemonic_iv"
        const val KEY_PUBKEY_HEX = "pubkey_hex"

        const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        const val KEY_ALIAS = "warren_wallet_master_v1"
        const val KEY_SIZE_BITS = 256
        const val GCM_TAG_BITS = 128
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}
