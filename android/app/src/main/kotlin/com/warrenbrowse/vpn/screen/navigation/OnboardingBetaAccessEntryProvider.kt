package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.wizardForwardTransition
import com.warrenbrowse.vpn.screen.onboarding.OnboardingBetaAccessScreen

fun EntryProviderScope<NavKey2>.onboardingBetaAccessEntry(navigator: Navigator) {
    entry<OnboardingBetaAccessNavKey>(metadata = wizardForwardTransition()) {
        OnboardingBetaAccessScreen(navigator = navigator)
    }
}
