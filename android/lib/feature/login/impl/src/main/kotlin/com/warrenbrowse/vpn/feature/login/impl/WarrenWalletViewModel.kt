package com.warrenbrowse.vpn.feature.login.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch

/**
 * Orchestrates the D.5 wallet onboarding flow:
 *   - [createWallet]: generate a fresh mnemonic and emit a
 *     [WarrenWalletEvent.BackupGeneratedMnemonic] so the host
 *     NavController can route to [WarrenWalletBackupScreen].
 *   - [importWallet]: parse the user-supplied phrase, persist, and
 *     emit [WarrenWalletEvent.WalletReady].
 *
 * The repository's `state: StateFlow<WalletState>` is re-exposed so the
 * Login screen can observe `WalletState.Ready` and route forward (the
 * post-Ready destination is owned by the app NavGraph, not the
 * ViewModel).
 *
 * Events use a [Channel] rather than a [StateFlow] because the routing
 * triggers should fire exactly once (a re-emission on configuration
 * change would re-navigate). Compose collects with
 * `events.receiveAsFlow().collect { ... }` inside `LaunchedEffect(Unit)`.
 */
class WarrenWalletViewModel(
    private val walletRepository: WalletRepository,
) : ViewModel() {

    val state: StateFlow<WalletState> = walletRepository.state

    private val _events = Channel<WarrenWalletEvent>(Channel.BUFFERED)
    val events: Flow<WarrenWalletEvent> = _events.receiveAsFlow()

    fun createWallet() {
        viewModelScope.launch {
            try {
                val mnemonic = walletRepository.createWallet()
                _events.send(WarrenWalletEvent.BackupGeneratedMnemonic(mnemonic))
            } catch (e: Exception) {
                Logger.w(throwable = e) { "wallet creation failed" }
                _events.send(WarrenWalletEvent.Error(e.message ?: "Wallet creation failed"))
            }
        }
    }

    fun importWallet(phrase: String) {
        viewModelScope.launch {
            val mnemonic = try {
                Mnemonic(phrase)
            } catch (e: IllegalArgumentException) {
                _events.send(WarrenWalletEvent.Error("Invalid recovery phrase"))
                return@launch
            }
            // `use { }` closes the Mnemonic at scope exit so the
            // CharArray is zeroed once the persist + pubkey-derive
            // round-trip completes (or fails). Audit follow-up:
            // without this the Mnemonic constructed from the
            // user-typed phrase lingered on the heap until GC.
            mnemonic.use { m ->
                try {
                    walletRepository.importWallet(m)
                    _events.send(WarrenWalletEvent.WalletReady)
                } catch (e: Exception) {
                    Logger.w(throwable = e) { "wallet import failed" }
                    _events.send(WarrenWalletEvent.Error(e.message ?: "Wallet import failed"))
                }
            }
        }
    }

    /**
     * Confirms the user has backed up the mnemonic; emits
     * [WarrenWalletEvent.WalletReady]. Called from
     * [WarrenWalletBackupScreen] when the user taps "I have written it
     * down".
     */
    fun confirmBackup() {
        viewModelScope.launch { _events.send(WarrenWalletEvent.WalletReady) }
    }
}

sealed interface WarrenWalletEvent {
    /** Fresh mnemonic just generated; navigate to the backup screen. */
    data class BackupGeneratedMnemonic(val mnemonic: Mnemonic) : WarrenWalletEvent

    /** Wallet is persisted and ready; navigate to home. */
    data object WalletReady : WarrenWalletEvent

    /** Operation failed; show the message inline. */
    data class Error(val message: String) : WarrenWalletEvent
}
