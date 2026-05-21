package com.warrenbrowse.vpn.screen.splash

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.selects.onTimeout
import kotlinx.coroutines.selects.select
import com.warrenbrowse.vpn.lib.common.util.isBeforeNowInstant
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.AccountRepository
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository

data class SplashScreenState(val splashComplete: Boolean = false)

class SplashViewModel(
    private val userPreferencesRepository: UserPreferencesRepository,
    private val accountRepository: AccountRepository,
    private val deviceRepository: DeviceRepository,
    private val splashCompleteRepository: SplashCompleteRepository,
    private val walletRepository: WalletRepository,
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

        // D.5: Warren-side wallet gate. The Mullvad device-state machine
        // below is preserved for now to keep the rest of the legacy UI
        // compiling, but on Warren mobile, "no wallet persisted" is the
        // first-launch signal. The wallet flow routes to ConnectNavKey
        // on completion, after which the existing device-state branch
        // is bypassed by `clearBackStack = true`.
        if (walletRepository.state.value is WalletState.Absent) {
            return SplashUiSideEffect.NavigateToWallet
        }

        val deviceState =
            deviceRepository.deviceState
                .map {
                    when (it) {
                        is DeviceState.LoggedIn -> ValidStartDeviceState.LoggedIn
                        DeviceState.LoggedOut -> ValidStartDeviceState.LoggedOut
                        DeviceState.Revoked -> ValidStartDeviceState.Revoked
                        null -> null
                    }
                }
                .filterNotNull()
                .first()

        return when (deviceState) {
            ValidStartDeviceState.LoggedOut -> SplashUiSideEffect.NavigateToLogin
            ValidStartDeviceState.Revoked -> SplashUiSideEffect.NavigateToRevoked
            ValidStartDeviceState.LoggedIn -> getLoggedInStartDestination()
        }
    }

    // We know the user is logged in, but we need to find out if their account has expired
    @OptIn(ExperimentalCoroutinesApi::class)
    private suspend fun getLoggedInStartDestination(): SplashUiSideEffect {
        val expiry = viewModelScope.async { accountRepository.accountData.filterNotNull().first() }

        val accountData = select {
            expiry.onAwait { it }
            // If we don't get a response within 1 second, assume the account expiry is Missing
            onTimeout(ACCOUNT_EXPIRY_TIMEOUT_MS) { null }
        }

        return if (accountData != null && accountData.expiryDate.isBeforeNowInstant()) {
            SplashUiSideEffect.NavigateToOutOfTime
        } else {
            SplashUiSideEffect.NavigateToConnect
        }
    }
}

private sealed interface ValidStartDeviceState {
    data object LoggedIn : ValidStartDeviceState

    data object Revoked : ValidStartDeviceState

    data object LoggedOut : ValidStartDeviceState
}

sealed interface SplashUiSideEffect {
    data object NavigateToPrivacyDisclaimer : SplashUiSideEffect

    data object NavigateToRevoked : SplashUiSideEffect

    data object NavigateToLogin : SplashUiSideEffect

    data object NavigateToConnect : SplashUiSideEffect

    data object NavigateToOutOfTime : SplashUiSideEffect

    data object NavigateToWallet : SplashUiSideEffect
}
