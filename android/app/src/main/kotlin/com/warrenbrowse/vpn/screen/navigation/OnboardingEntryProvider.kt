package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.wizardForwardTransition
import com.warrenbrowse.vpn.feature.login.api.OnboardingNavKey
import com.warrenbrowse.vpn.screen.onboarding.OnboardingScreen

fun EntryProviderScope<NavKey2>.onboardingEntry(navigator: Navigator) {
    entry<OnboardingNavKey>(metadata = wizardForwardTransition { navigator.reachedFromSplash() }) {
        OnboardingScreen(navigator = navigator)
    }
}

/**
 * True when the previous top of the stack was the splash screen, which resolves the start
 * destination and swaps the whole stack for it. That swap carries no spatial relationship, so the
 * wizard steps reachable from it crossfade instead of sliding.
 */
internal fun Navigator.reachedFromSplash(): Boolean = previousBackStack.lastOrNull() is SplashNavKey
