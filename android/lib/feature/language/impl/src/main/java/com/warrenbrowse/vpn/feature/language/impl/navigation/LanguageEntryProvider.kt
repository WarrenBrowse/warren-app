package com.warrenbrowse.vpn.feature.language.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.language.api.LanguageNavKey
import com.warrenbrowse.vpn.feature.language.impl.Language

fun EntryProviderScope<NavKey2>.languageEntry(navigator: Navigator) {
    entry<LanguageNavKey>(metadata = slideInHorizontalTransition()) {
        Language(navigator = navigator)
    }
}
