package com.warrenbrowse.vpn.screen.splash

import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.repository.UserPreferences
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.mockk
import io.mockk.every
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Test

class SplashViewModelTest {

    private val mockPrefs: UserPreferencesRepository = mockk()
    private val mockSplashComplete: SplashCompleteRepository = mockk(relaxed = true)
    private val mockWalletRepository: WalletRepository = mockk()
    private val mockLocalSettings: WarrenLocalSettingsRepository = mockk()

    private fun makeVm(
        privacyAccepted: Boolean = true,
        walletState: WalletState = WalletState.Absent,
        onboardingCompleted: Boolean = true,
    ): SplashViewModel {
        val prefs: UserPreferences = mockk {
            every { isPrivacyDisclosureAccepted } returns privacyAccepted
        }
        coEvery { mockPrefs.preferences() } returns prefs
        every { mockWalletRepository.state } returns MutableStateFlow(walletState)
        every { mockLocalSettings.onboardingCompleted } returns MutableStateFlow(onboardingCompleted)
        return SplashViewModel(
            userPreferencesRepository = mockPrefs,
            splashCompleteRepository = mockSplashComplete,
            walletRepository = lazyOf(mockWalletRepository),
            localSettings = lazyOf(mockLocalSettings),
        )
    }

    @Test
    fun `the wallet and the settings are resolved off the calling thread`() = runTest {
        // Both repositories load a preferences file when they are first
        // constructed, and the view model is built on the main thread: the
        // decision has to be the one to construct them, somewhere else.
        val prefs: UserPreferences = mockk { every { isPrivacyDisclosureAccepted } returns true }
        coEvery { mockPrefs.preferences() } returns prefs
        every { mockWalletRepository.state } returns MutableStateFlow(WalletState.Absent)
        every { mockLocalSettings.onboardingCompleted } returns MutableStateFlow(true)
        val caller = Thread.currentThread().name
        var walletThread: String? = null
        var settingsThread: String? = null
        val vm =
            SplashViewModel(
                userPreferencesRepository = mockPrefs,
                splashCompleteRepository = mockSplashComplete,
                walletRepository =
                    lazy {
                        walletThread = Thread.currentThread().name
                        mockWalletRepository
                    },
                localSettings =
                    lazy {
                        settingsThread = Thread.currentThread().name
                        mockLocalSettings
                    },
            )

        assertEquals(SplashUiSideEffect.NavigateToWallet, vm.uiSideEffect.first())

        assertNotEquals(caller, walletThread, "the wallet must not be constructed on the caller")
        assertNotEquals(caller, settingsThread, "the settings must not be constructed on the caller")
    }

    @Test
    fun `privacy not accepted routes to PrivacyDisclaimer`() = runTest {
        val vm = makeVm(privacyAccepted = false)
        val side = vm.uiSideEffect.first()
        assertEquals(SplashUiSideEffect.NavigateToPrivacyDisclaimer, side)
    }

    @Test
    fun `wallet Absent with onboarding done routes to Wallet`() = runTest {
        val vm = makeVm(
            privacyAccepted = true,
            walletState = WalletState.Absent,
            onboardingCompleted = true,
        )
        val side = vm.uiSideEffect.first()
        assertEquals(SplashUiSideEffect.NavigateToWallet, side)
    }

    @Test
    fun `wallet Absent with onboarding not done routes to Onboarding`() = runTest {
        val vm = makeVm(
            privacyAccepted = true,
            walletState = WalletState.Absent,
            onboardingCompleted = false,
        )
        val side = vm.uiSideEffect.first()
        assertEquals(SplashUiSideEffect.NavigateToOnboarding, side)
    }

    @Test
    fun `existing user (wallet Ready) never sees onboarding even if flag is false`() = runTest {
        val vm = makeVm(
            privacyAccepted = true,
            walletState = WalletState.Ready(
                WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"),
            ),
            onboardingCompleted = false,
        )
        val side = vm.uiSideEffect.first()
        assertEquals(SplashUiSideEffect.NavigateToConnect, side)
    }

    @Test
    fun `wallet Ready routes to Connect`() = runTest {
        val vm = makeVm(
            privacyAccepted = true,
            walletState = WalletState.Ready(
                WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"),
            ),
        )
        val side = vm.uiSideEffect.first()
        assertEquals(SplashUiSideEffect.NavigateToConnect, side)
    }

    @Test
    fun `wallet Locked also routes to Connect`() = runTest {
        // Locked = wallet persisted but unlock not yet performed. The
        // splash decision tree routes to Connect; the Connect button
        // triggers the unlock when the user taps it.
        val vm = makeVm(
            privacyAccepted = true,
            walletState = WalletState.Locked(
                WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"),
            ),
        )
        val side = vm.uiSideEffect.first()
        assertEquals(SplashUiSideEffect.NavigateToConnect, side)
    }

    @Test
    fun `splash completion side-effect fires after destination is emitted`() = runTest {
        val vm = makeVm(privacyAccepted = true, walletState = WalletState.Absent)
        vm.uiSideEffect.collect { /* drain the flow */ }
        coVerify { mockSplashComplete.onSplashCompleted() }
    }
}
