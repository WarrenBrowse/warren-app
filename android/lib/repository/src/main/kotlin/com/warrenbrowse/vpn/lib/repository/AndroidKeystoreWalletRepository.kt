package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.CipherAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
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
 *  - `pubkey_ss58` (string): the Warren SS58 address (`wb…`, 47-49
 *    chars), cached so the UI can display the wallet identifier without
 *    decrypting the mnemonic at every cold start.
 *
 * The Keystore master key is named "warren_wallet_master_v1"; it is
 * generated once on first use with `purposes = ENCRYPT | DECRYPT`,
 * `blockModes = GCM`, `paddings = NoPadding`. The key never leaves the
 * Keystore: encrypt / decrypt operations happen inside the secure
 * subsystem (hardware-backed on devices that support it).
 *
 * Biometric gating: every cleartext read goes through [unlock], which
 * requires the app-supplied [SensitiveOpAuthorizer] (a `BiometricPrompt`
 * in production) to succeed first; [decryptMnemonic] is private and has no
 * other caller. So the mnemonic cannot be decrypted without a user
 * authentication at the application layer.
 *
 * Remaining hardening (deferred):
 *   - Hardware-enforced auth via `setUserAuthenticationRequired(true)` on
 *     the master key spec, so an in-process caller cannot use the Cipher
 *     directly to bypass [unlock]. This needs a `CryptoObject`-bound
 *     `BiometricPrompt` (the op authorized at the Keystore boundary, not a
 *     separate boolean prompt) and changes the create/import flow (encrypt
 *     would also require a recent auth), so it is tracked as its own task.
 *   - Tamper-evidence: a MAC over `(pubkey_ss58, ciphertext, iv)` so a
 *     swapped-out wallet file is detected at boot.
 */
class AndroidKeystoreWalletRepository(
    context: Context,
    private val jni: WarrenJniBridge,
) : WalletRepository {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    private val _state = MutableStateFlow<WalletState>(loadInitialState())
    override val state: StateFlow<WalletState> = _state.asStateFlow()

    private val lock = Mutex()

    override suspend fun createWallet(): Mnemonic = lock.withLock {
        withContext(Dispatchers.IO) {
            val phrase = jni.generateMnemonic()
            check(phrase.isNotEmpty()) { "WarrenJniBridge.generateMnemonic returned empty string" }
            val mnemonic = Mnemonic(phrase)
            persist(mnemonic)
            mnemonic
        }
    }

    override suspend fun importWallet(mnemonic: Mnemonic): WalletAddress = lock.withLock {
        withContext(Dispatchers.IO) {
            val address = mnemonic.useAsString { jni.mnemonicPubkeySs58(it) }
            persist(mnemonic, address)
            WalletAddress(address)
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
        if (HARDWARE_AUTH && authorizer is CipherAuthorizer) {
            // Hardware-bound path: the Keystore key requires per-use auth, so
            // the decrypt Cipher itself is authorized through a CryptoObject
            // prompt; an in-process caller cannot doFinal without it. The
            // prompt must run on the main thread, so it stays OUT of the IO
            // dispatcher; only the doFinal is offloaded.
            val cipher = buildDecryptCipherOrNull()
                ?: throw IllegalStateException(
                    "no wallet on disk - call createWallet/importWallet first"
                )
            val authed = authorizer.authorizeCipher(cipher, reason)
                ?: throw WalletAuthorizationDeniedException(
                    "User declined or device cannot authenticate"
                )
            val chars = withContext(Dispatchers.IO) { finishDecrypt(authed) }
            return@withLock Mnemonic.fromCharArrayOwning(chars)
        }
        if (!authorizer.authorize(reason)) {
            throw WalletAuthorizationDeniedException(
                "User declined or device cannot authenticate"
            )
        }
        withContext(Dispatchers.IO) {
            val chars = decryptMnemonic()
                ?: throw IllegalStateException(
                    "no wallet on disk - call createWallet/importWallet first"
                )
            // Ownership transfer: the new Mnemonic now owns `chars`
            // and will zero it on close(). We MUST NOT mutate `chars`
            // afterwards.
            Mnemonic.fromCharArrayOwning(chars)
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
        val address = prefs.getString(KEY_PUBKEY_SS58, null) ?: return WalletState.Absent
        return try {
            WalletState.Locked.also {
                // We don't decrypt at boot - UI must call `unlock()` to
                // get the cleartext mnemonic for a signing operation.
                @Suppress("UNUSED_EXPRESSION") WalletAddress(address)
            }
        } catch (e: IllegalArgumentException) {
            Logger.w(throwable = e) { "persisted address is malformed; treating wallet as absent" }
            WalletState.Absent
        }
    }

    private fun persist(mnemonic: Mnemonic, pubkeySs58: String? = null) {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.ENCRYPT_MODE, getOrCreateMasterKey())
        }
        // Use `useAsString` so the temporary cleartext-bytes
        // intermediate dies at the lambda boundary. The CharArray
        // backing the Mnemonic stays owned by the caller (we MUST
        // NOT close it here - the caller may still need it for the
        // immediate connect/signing path).
        val ciphertext = mnemonic.useAsString { phrase ->
            cipher.doFinal(phrase.toByteArray(Charsets.UTF_8))
        }
        val iv = cipher.iv

        val address = pubkeySs58 ?: mnemonic.useAsString { jni.mnemonicPubkeySs58(it) }

        prefs.edit()
            .putString(KEY_CIPHERTEXT, Base64.encodeToString(ciphertext, Base64.NO_WRAP))
            .putString(KEY_IV, Base64.encodeToString(iv, Base64.NO_WRAP))
            .putString(KEY_PUBKEY_SS58, address)
            .apply()

        _state.value = WalletState.Ready(WalletAddress(address))
    }

    /**
     * Decrypt the persisted mnemonic into a freshly allocated
     * [CharArray] and zero the intermediate plaintext byte buffer
     * before returning. Returns `null` when no wallet is on disk.
     *
     * Returning a [CharArray] instead of a [String] avoids
     * materialising a long-lived JVM String for the secret; the
     * caller (`unlock`) wraps the array directly into a [Mnemonic]
     * which owns its zero-on-close lifecycle.
     */
    private fun decryptMnemonic(): CharArray? {
        val cipher = buildDecryptCipherOrNull() ?: return null
        return finishDecrypt(cipher)
    }

    /**
     * Build a DECRYPT-mode [Cipher] initialised with the stored IV, or `null`
     * when no wallet is on disk. When the master key requires user auth
     * ([HARDWARE_AUTH]) the returned cipher is NOT yet usable: it must first
     * be authorised via a `CryptoObject` prompt ([CipherAuthorizer]) before
     * [finishDecrypt] can `doFinal`.
     */
    private fun buildDecryptCipherOrNull(): Cipher? {
        val ivB64 = prefs.getString(KEY_IV, null) ?: return null
        if (prefs.getString(KEY_CIPHERTEXT, null) == null) return null
        val iv = Base64.decode(ivB64, Base64.NO_WRAP)
        return Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, getOrCreateMasterKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
        }
    }

    /**
     * Decrypt the persisted ciphertext with an (already authorised, if the key
     * requires it) [cipher] into a freshly allocated [CharArray], zeroing the
     * byte intermediate. The caller owns the array's zero-on-close lifecycle.
     */
    private fun finishDecrypt(cipher: Cipher): CharArray {
        val ciphertextB64 = requireNotNull(prefs.getString(KEY_CIPHERTEXT, null)) {
            "ciphertext vanished between cipher build and decrypt"
        }
        val ciphertext = Base64.decode(ciphertextB64, Base64.NO_WRAP)
        val plaintext = cipher.doFinal(ciphertext)
        try {
            val decoded = Charsets.UTF_8.decode(java.nio.ByteBuffer.wrap(plaintext))
            // Copy CharBuffer -> CharArray with exact length (the
            // buffer's underlying array may be larger).
            val chars = CharArray(decoded.remaining())
            decoded.get(chars)
            return chars
        } finally {
            // Zero the byte intermediate eagerly so the plaintext
            // does not linger as a raw byte array on the heap.
            plaintext.fill(0)
        }
    }

    private fun getOrCreateMasterKey(): SecretKey {
        val ks = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        ks.getKey(KEY_ALIAS, null)?.let { return it as SecretKey }

        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        val builder = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(KEY_SIZE_BITS)
        if (HARDWARE_AUTH) {
            // Hardware-enforce a fresh user authentication for EVERY use of the
            // key: the crypto op must be bound to a BiometricPrompt.CryptoObject
            // (see unlock()'s hardware path), so an in-process caller cannot use
            // the cipher by calling doFinal directly. On API 30+ a timeout of 0
            // means per-use; pre-30, setUserAuthenticationRequired(true) alone
            // already means per-use via CryptoObject.
            builder.setUserAuthenticationRequired(true)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                builder.setUserAuthenticationParameters(
                    0,
                    KeyProperties.AUTH_BIOMETRIC_STRONG,
                )
            }
        }
        // Decryption is also gated at the app layer (unlock() requires the
        // BiometricPrompt authorizer) regardless of HARDWARE_AUTH.
        gen.init(builder.build())
        return gen.generateKey()
    }

    private companion object {
        const val PREFS_NAME = "warren_wallet"
        const val KEY_CIPHERTEXT = "mnemonic_ciphertext"
        const val KEY_IV = "mnemonic_iv"
        const val KEY_PUBKEY_SS58 = "pubkey_ss58"

        const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        const val KEY_ALIAS = "warren_wallet_master_v1"
        const val KEY_SIZE_BITS = 256
        const val GCM_TAG_BITS = 128
        const val TRANSFORMATION = "AES/GCM/NoPadding"

        // Hardware-bound, per-use auth on the master key (defence-in-depth on
        // top of the app-layer unlock() gate). OFF by default: the unlock()
        // decrypt path is wired (CryptoObject), but turning this on also makes
        // the ENCRYPT at create/import require a CryptoObject prompt, which is
        // NOT wired yet (createWallet/importWallet take no authorizer). Enable
        // ONLY after wiring + on-device verification of the encrypt path, or
        // new-wallet creation will fail with UserNotAuthenticatedException.
        const val HARDWARE_AUTH = false
    }
}
