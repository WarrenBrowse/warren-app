package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.runtime.toMutableStateList
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavigationState
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.ResultStore
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.screen.navigation.OnboardingNavKey
import io.mockk.mockk
import io.mockk.verify
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class OnboardingFlowTest {

    private val settings: WarrenLocalSettingsRepository = mockk(relaxed = true)

    private fun navigatorAt(vararg keys: NavKey2) =
        Navigator(
            state = NavigationState(keys.toList().toMutableStateList()),
            resultStore = ResultStore(),
            screenIsListDetailTargetWidth = false,
        )

    @Test
    fun `the welcome CTA never marks the wizard completed`() {
        val navigator = navigatorAt(OnboardingNavKey)

        enterWalletStep(navigator)

        verify(exactly = 0) { settings.setOnboardingCompleted(any()) }
    }

    @Test
    fun `the welcome CTA keeps the welcome step reachable`() {
        val navigator = navigatorAt(OnboardingNavKey)

        enterWalletStep(navigator)

        assertEquals(
            listOf(OnboardingNavKey, WarrenWalletNavKey(onboarding = true)),
            navigator.backStack.toList(),
        )
    }

    @Test
    fun `leaving the wizard marks it completed`() {
        val navigator = navigatorAt(OnboardingNavKey)

        leaveWizard(settings, navigator, ConnectNavKey)

        verify(exactly = 1) { settings.setOnboardingCompleted(true) }
    }

    @Test
    fun `leaving the wizard roots the destination so the wizard is not walkable back into`() {
        val navigator = navigatorAt(OnboardingNavKey, WarrenWalletNavKey(onboarding = true))

        leaveWizard(settings, navigator, ConnectNavKey)

        assertEquals(listOf(ConnectNavKey), navigator.backStack.toList())
    }

    @Test
    fun `the funding step advances once credit lands`() {
        assertTrue(shouldAdvanceFromFundingStep(alreadyAdvanced = false, funded = true))
    }

    @Test
    fun `the funding step does not advance while the account is unfunded`() {
        assertFalse(shouldAdvanceFromFundingStep(alreadyAdvanced = false, funded = false))
    }

    @Test
    fun `the funding step advances at most once`() {
        // Walking back into a funded step must not throw the user forward again.
        assertFalse(shouldAdvanceFromFundingStep(alreadyAdvanced = true, funded = true))
    }

    @Test
    fun `the funding step holds while a modal the user is reading is up`() {
        assertFalse(
            shouldAdvanceFromFundingStep(alreadyAdvanced = false, funded = true, held = true),
        )
        // Holding must not burn the one-shot: the advance still happens on close.
        assertTrue(
            shouldAdvanceFromFundingStep(alreadyAdvanced = false, funded = true, held = false),
        )
    }
}
