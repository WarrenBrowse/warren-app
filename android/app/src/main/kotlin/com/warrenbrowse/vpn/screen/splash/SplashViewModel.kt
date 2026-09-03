package com.warrenbrowse.vpn.screen.splash

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.withContext
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

data class SplashScreenState(val splashComplete: Boolean = false)

/**
 * Warren-native splash decision tree.
 *
 * The tree is exhaustive:
 *
 *   1. Privacy disclosure not accepted → [PrivacyDisclaimer]
 *   2. Wallet absent & onboarding not done → [Onboarding] (welcome, once)
 *   3. Wallet absent                   → [Wallet]
 *   4. Wallet ready or locked          → [Connect]
 *
 * The onboarding welcome is gated on a wallet still being absent, so
 * existing users (who already have a wallet) never see it on update.
 *
 * The splash itself has no "out of time" branch: an expired subscription is
 * handled by the app-level gate in `WarrenApp` (`outOfTimeGateActive`), which
 * overlays the main flow after this routing settles. There is still no
 * "revoked device" branch: the BIP39 wallet identity model has no per-device
 * registry to revoke.
 *
 * The wallet and the local settings are [Lazy]: both read their preferences
 * file when constructed, and this view model is built on the main thread, so
 * they are resolved inside the decision, on IO, where a first construction
 * costs no frame.
 */
class SplashViewModel(
    private val userPreferencesRepository: UserPreferencesRepository,
    private val splashCompleteRepository: SplashCompleteRepository,
    private val walletRepository: Lazy<WalletRepository>,
    private val localSettings: Lazy<WarrenLocalSettingsRepository>,
) : ViewModel() {

    val uiSideEffect = flow {
        emit(getStartDestination())
        splashCompleteRepository.onSplashCompleted()
    }

    private val _uiState = MutableStateFlow(SplashScreenState(false))
    val uiState: StateFlow<SplashScreenState> = _uiState

    private suspend fun getStartDestination(): SplashUiSideEffect {
        if (!userPreferencesRepository.preferences().isPrivacyDisclosureAccepted) {
            return SplashUiSideEffect.NavigateToPrivacyDisclaimer
        }
        return withContext(Dispatchers.IO) {
            val walletAbsent = walletRepository.value.state.value is WalletState.Absent
            when {
                walletAbsent && !localSettings.value.onboardingCompleted.value ->
                    SplashUiSideEffect.NavigateToOnboarding
                walletAbsent -> SplashUiSideEffect.NavigateToWallet
                else -> SplashUiSideEffect.NavigateToConnect
            }
        }
    }
}

sealed interface SplashUiSideEffect {
    data object NavigateToPrivacyDisclaimer : SplashUiSideEffect

    data object NavigateToConnect : SplashUiSideEffect

    data object NavigateToWallet : SplashUiSideEffect

    data object NavigateToOnboarding : SplashUiSideEffect
}
