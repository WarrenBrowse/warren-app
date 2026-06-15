package com.warrenbrowse.vpn.lib.model.wallet

/**
 * Persisted wallet state, observable from the UI. The
 * `com.warrenbrowse.vpn.lib.repository.WalletRepository` (D.5 impl) emits
 * a `StateFlow<WalletState>` that the login / settings / signup screens
 * collect on.
 */
sealed interface WalletState {
    /** No wallet on the device. The user must either generate or import one. */
    data object Absent : WalletState

    /**
     * Wallet present but the mnemonic is still encrypted at rest (Android
     * Keystore): it has not been decrypted for this session. This is the
     * normal resting state. The [pubkey] is the user's PUBLIC Warren address,
     * persisted in cleartext and safe to display without authentication (it is
     * the account identity, like the desktop "Public key" row); only revealing
     * the recovery phrase or signing requires an unlock.
     */
    data class Locked(val pubkey: WalletAddress) : WalletState

    /**
     * Wallet decrypted and ready. The `Mnemonic` is held in memory only
     * for the duration of a sensitive operation (signing call, backup
     * view). Once the operation completes the repository transitions back
     * to [Locked] and the `Mnemonic` reference is released.
     */
    data class Ready(val pubkey: WalletAddress) : WalletState
}
