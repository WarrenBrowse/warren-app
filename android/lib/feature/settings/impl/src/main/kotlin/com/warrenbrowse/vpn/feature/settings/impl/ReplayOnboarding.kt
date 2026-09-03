package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.OnboardingNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

/**
 * Re-run the first-launch wizard from the settings root (desktop `ReplayOnboardingListItem`). The
 * gate is lowered BEFORE the navigation, as on desktop, so a start-destination resolution racing
 * the push still lands on the wizard; the wizard's own exits raise it again and root the stack.
 */
internal fun replayOnboarding(settings: WarrenLocalSettingsRepository, navigator: Navigator) {
    settings.setOnboardingCompleted(false)
    navigator.navigate(OnboardingNavKey, clearBackStack = true)
}
