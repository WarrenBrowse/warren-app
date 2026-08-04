package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.wizardForwardTransition
import com.warrenbrowse.vpn.screen.onboarding.OnboardingDoneScreen

fun EntryProviderScope<NavKey2>.onboardingDoneEntry(navigator: Navigator) {
    entry<OnboardingDoneNavKey>(metadata = wizardForwardTransition()) {
        OnboardingDoneScreen(navigator = navigator)
    }
}
