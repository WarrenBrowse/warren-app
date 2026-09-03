package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.runtime.toMutableStateList
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavigationState
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.ResultStore
import com.warrenbrowse.vpn.feature.login.api.OnboardingNavKey
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import io.mockk.mockk
import io.mockk.verify
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class ReplayOnboardingTest {

    private val settings: WarrenLocalSettingsRepository = mockk(relaxed = true)

    private fun navigatorAt(vararg keys: NavKey2) =
        Navigator(
            state = NavigationState(keys.toList().toMutableStateList()),
            resultStore = ResultStore(),
            screenIsListDetailTargetWidth = false,
        )

    @Test
    fun `replaying lowers the onboarding gate and roots the wizard`() {
        val navigator = navigatorAt(SettingsNavKey)

        replayOnboarding(settings, navigator)

        verify(exactly = 1) { settings.setOnboardingCompleted(false) }
        assertEquals(listOf<NavKey2>(OnboardingNavKey), navigator.backStack.toList())
    }
}
