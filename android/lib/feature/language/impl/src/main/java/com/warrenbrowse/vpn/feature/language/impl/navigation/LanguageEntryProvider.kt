package com.warrenbrowse.vpn.feature.language.impl.navigation

import androidx.annotation.RequiresApi
import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.language.api.LanguageNavKey
import com.warrenbrowse.vpn.feature.language.impl.Language

@RequiresApi(android.os.Build.VERSION_CODES.TIRAMISU)
fun EntryProviderScope<NavKey2>.languageEntry(navigator: Navigator) {
    entry<LanguageNavKey>(metadata = slideInHorizontalTransition()) {
        Language(navigator = navigator)
    }
}
