package com.warrenbrowse.vpn.feature.login.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.wallet.normalizeMnemonic
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch

/**
 * Orchestrates the wallet onboarding flow:
 *   - [createWallet]: generate a fresh mnemonic and emit a
 *     [WarrenWalletEvent.BackupGeneratedMnemonic] so the host
 *     NavController can route to [WarrenWalletBackupScreen].
 *   - [importWallet]: parse the phrase held in [importPhrase], persist, and
 *     emit [WarrenWalletEvent.WalletReady].
 *
 * The repository's `state: StateFlow<WalletState>` is re-exposed so the
 * Login screen can observe `WalletState.Ready` and route forward (the
 * post-Ready destination is owned by the app NavGraph, not the
 * ViewModel).
 *
 * The phrase being typed lives here rather than in composable state because a
 * rotation or a trip to a password manager recreates the Activity and would
 * otherwise wipe twelve hand-typed words. It deliberately stays out of
 * `rememberSaveable`: the saved-state Bundle is system-managed storage, which
 * is exactly what the MnemonicCache handoff exists to keep secrets out of.
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

    private val _busy = MutableStateFlow(false)

    /** True while a create or import round trip (BIP39 + Keystore + JNI) runs. */
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    private val _importPhrase = MutableStateFlow("")

    /** Raw text of the phrase being typed, verbatim so editing stays natural. */
    val importPhrase: StateFlow<String> = _importPhrase.asStateFlow()

    fun setImportPhrase(value: String) {
        _importPhrase.value = value
    }

    fun createWallet(authorizer: SensitiveOpAuthorizer? = null) {
        if (_busy.value) return
        viewModelScope.launch {
            _busy.value = true
            try {
                val mnemonic = walletRepository.createWallet(authorizer)
                _events.send(WarrenWalletEvent.BackupGeneratedMnemonic(mnemonic))
            } catch (e: Exception) {
                Logger.w(throwable = e) { "wallet creation failed" }
                _events.send(WarrenWalletEvent.Error(WalletErrorReason.CreateFailed))
            } finally {
                _busy.value = false
            }
        }
    }

    fun importWallet(authorizer: SensitiveOpAuthorizer? = null) {
        if (_busy.value) return
        viewModelScope.launch {
            _busy.value = true
            try {
                val mnemonic =
                    try {
                        Mnemonic(normalizeMnemonic(_importPhrase.value))
                    } catch (e: IllegalArgumentException) {
                        // Wrong word count: the phrase is not 12 or 24 words.
                        // Distinct from a checksum/spelling failure, which the
                        // daemon catches below during importWallet.
                        Logger.w(throwable = e) { "mnemonic word count rejected" }
                        _events.send(WarrenWalletEvent.Error(WalletErrorReason.WrongWordCount))
                        return@launch
                    }
                // `use { }` closes the Mnemonic at scope exit so the
                // CharArray is zeroed once the persist + pubkey-derive
                // round-trip completes (or fails). Without this the Mnemonic
                // constructed from the user-typed phrase would linger on the
                // heap until GC.
                mnemonic.use { m ->
                    try {
                        walletRepository.importWallet(m, authorizer)
                        _importPhrase.value = ""
                        _events.send(WarrenWalletEvent.WalletReady)
                    } catch (e: Exception) {
                        Logger.w(throwable = e) { "wallet import failed" }
                        // Daemon rejected the BIP39 checksum/wordlist: a typo
                        // or a wrong word order.
                        _events.send(WarrenWalletEvent.Error(WalletErrorReason.InvalidPhrase))
                    }
                }
            } finally {
                _busy.value = false
            }
        }
    }

    /**
     * Confirms the user has backed up the mnemonic; emits
     * [WarrenWalletEvent.WalletReady]. Called from
     * [WarrenWalletBackupScreen] when the user confirms the backup.
     */
    fun confirmBackup() {
        viewModelScope.launch { _events.send(WarrenWalletEvent.WalletReady) }
    }
}

/**
 * Why a wallet operation failed, as a typed value the screen resolves to a
 * localized string. The engine's own message never reaches the user: it is not
 * translated and can carry paths or key material.
 */
enum class WalletErrorReason {
    /** Not 12 or 24 words. */
    WrongWordCount,

    /** Right word count, rejected by the BIP39 wordlist or checksum. */
    InvalidPhrase,

    /** Generation or Keystore persistence failed. */
    CreateFailed,
}

sealed interface WarrenWalletEvent {
    /** Fresh mnemonic just generated; navigate to the backup screen. */
    data class BackupGeneratedMnemonic(val mnemonic: Mnemonic) : WarrenWalletEvent

    /** Wallet is persisted and ready; navigate to home. */
    data object WalletReady : WarrenWalletEvent

    /** Operation failed; show the mapped message inline. */
    data class Error(val reason: WalletErrorReason) : WarrenWalletEvent
}
