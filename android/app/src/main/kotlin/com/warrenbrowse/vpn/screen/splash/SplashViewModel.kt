package com.warrenbrowse.vpn.screen.splash

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

data class SplashScreenState(val splashComplete: Boolean = false)

/**
 * D.4 step 21: Warren-native splash decision tree.
 *
 * Drops the entire Mullvad device-state / account-expiry machinery
 * (the consumed `DeviceRepository` + `AccountRepository` source from a
 * dead gRPC daemon and the `.first()` await blocked indefinitely on
 * Warren mobile). The tree is now exhaustive:
 *
 *   1. Privacy disclosure not accepted → [PrivacyDisclaimer]
 *   2. Wallet absent & onboarding not done → [Onboarding] (welcome, once)
 *   3. Wallet absent                   → [Wallet]
 *   4. Wallet ready or locked          → [Connect]
 *
 * The onboarding welcome is gated on a wallet still being absent, so
 * existing users (who already have a wallet) never see it on update.
 *
 * There is no "out of time" or "revoked device" branch on Warren - the
 * subscription model + multi-device accounting are Mullvad-only and
 * have no equivalent in the BIP39 wallet identity model.
 */
class SplashViewModel(
    private val userPreferencesRepository: UserPreferencesRepository,
    private val splashCompleteRepository: SplashCompleteRepository,
    private val walletRepository: WalletRepository,
    private val localSettings: WarrenLocalSettingsRepository,
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
        val walletAbsent = walletRepository.state.value is WalletState.Absent
        return when {
            walletAbsent && !localSettings.onboardingCompleted.value ->
                SplashUiSideEffect.NavigateToOnboarding
            walletAbsent -> SplashUiSideEffect.NavigateToWallet
            else -> SplashUiSideEffect.NavigateToConnect
        }
    }
}

sealed interface SplashUiSideEffect {
    data object NavigateToPrivacyDisclaimer : SplashUiSideEffect

    data object NavigateToConnect : SplashUiSideEffect

    data object NavigateToWallet : SplashUiSideEffect

    data object NavigateToOnboarding : SplashUiSideEffect
}
