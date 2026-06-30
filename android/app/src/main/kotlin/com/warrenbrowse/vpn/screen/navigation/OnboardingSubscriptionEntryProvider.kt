package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.screen.onboarding.OnboardingSubscriptionScreen

fun EntryProviderScope<NavKey2>.onboardingSubscriptionEntry(navigator: Navigator) {
    entry<OnboardingSubscriptionNavKey> { OnboardingSubscriptionScreen(navigator = navigator) }
}
