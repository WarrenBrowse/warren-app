package com.warrenbrowse.vpn.screen.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.wizardForwardTransition
import com.warrenbrowse.vpn.screen.privacy.PrivacyDisclaimer

fun EntryProviderScope<NavKey2>.privacyDisclaimerEntry(navigator: Navigator) {
    entry<PrivacyDisclaimerNavKey>(
        metadata = wizardForwardTransition { navigator.reachedFromSplash() }
    ) {
        PrivacyDisclaimer(navigator = navigator)
    }
}
