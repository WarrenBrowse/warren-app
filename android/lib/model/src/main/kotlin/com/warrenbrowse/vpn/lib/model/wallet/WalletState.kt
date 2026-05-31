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
     * Wallet locked behind a biometric / device-credential prompt. The
     * mnemonic is on disk encrypted by Android Keystore but has not yet
     * been decrypted for this session.
     */
    data object Locked : WalletState

    /**
     * Wallet decrypted and ready. The `Mnemonic` is held in memory only
     * for the duration of a sensitive operation (signing call, backup
     * view). Once the operation completes the repository transitions back
     * to [Locked] and the `Mnemonic` reference is released.
     */
    data class Ready(val pubkey: WalletAddress) : WalletState
}
