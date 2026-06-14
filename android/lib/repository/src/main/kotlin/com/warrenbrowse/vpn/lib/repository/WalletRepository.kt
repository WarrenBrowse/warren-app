package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import kotlinx.coroutines.flow.StateFlow

/**
 * Wallet persistence contract.
 *
 * The single source of truth for the device's Warren wallet (mnemonic
 * encrypted by Android Keystore + EncryptedSharedPreferences). The
 * concrete `AndroidKeystoreWalletRepository` impl lives in
 * `lib/feature/wallet/impl/` (D.5).
 *
 * All methods that touch the mnemonic in cleartext are suspending so
 * callers must opt in to the I/O cost; the suspending boundary also
 * lets a future impl gate the decrypt step behind a `BiometricPrompt`
 * without changing the surface.
 */
interface WalletRepository {
    /** Observable wallet state. Emits a fresh value on every transition. */
    val state: StateFlow<WalletState>

    /**
     * Generate a fresh 12-word BIP39 mnemonic via `WarrenJni.generateMnemonic`,
     * persist it (Keystore-encrypted), and transition `state` to
     * [WalletState.Ready] with the derived pubkey. Returns the mnemonic
     * exactly once so the UI can prompt the user to back it up; subsequent
     * reads must go through [unlock].
     *
     * [authorizer] is only consulted when hardware-bound keystore auth is
     * enabled (a `CipherAuthorizer` then authorises the encrypt CryptoObject);
     * `null` keeps the current no-prompt-at-create behaviour.
     */
    suspend fun createWallet(authorizer: SensitiveOpAuthorizer? = null): Mnemonic

    /**
     * Import an existing 12 / 24-word BIP39 mnemonic, persist it, and
     * transition `state` to [WalletState.Ready]. Returns the derived pubkey
     * for the caller to confirm against (e.g. server-side wallet binding).
     * See [createWallet] for [authorizer].
     */
    suspend fun importWallet(
        mnemonic: Mnemonic,
        authorizer: SensitiveOpAuthorizer? = null,
    ): WalletAddress

    /**
     * Decrypt the persisted mnemonic just-in-time. The repository invokes
     * [authorizer.authorize] with a human-readable reason before reading
     * the cleartext; if the user cancels or hardware authentication is
     * unavailable the call throws [WalletAuthorizationDeniedException].
     *
     * The returned `Mnemonic` reference must NOT be stored long-term -
     * the caller passes it to the signing operation and lets it drop.
     */
    @Throws(WalletAuthorizationDeniedException::class)
    suspend fun unlock(
        authorizer: SensitiveOpAuthorizer,
        reason: String = "Confirm to access your Warren wallet",
    ): Mnemonic

    /**
     * Erase the persisted wallet. Irreversible; the user is expected to
     * have backed up the mnemonic if they want to recover the same wallet
     * later via [importWallet].
     */
    suspend fun erase()
}

/** Thrown when [WalletRepository.unlock] is called but the user cancels
 * or the device cannot authenticate. */
class WalletAuthorizationDeniedException(message: String) : Exception(message)
