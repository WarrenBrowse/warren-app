package com.warrenbrowse.vpn.feature.settings.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.settings.api.ForumSignInCodeNavKey
import com.warrenbrowse.vpn.feature.settings.api.ReportPreviewNavKey
import com.warrenbrowse.vpn.feature.settings.api.ReportProblemNavKey
import com.warrenbrowse.vpn.feature.settings.impl.support.ForumSignInCode
import com.warrenbrowse.vpn.feature.settings.impl.support.ReportPreview
import com.warrenbrowse.vpn.feature.settings.impl.support.ReportProblem

/** The support destinations reached from the settings root. */
fun EntryProviderScope<NavKey2>.warrenSupportEntries(navigator: Navigator) {
    entry<ReportProblemNavKey>(metadata = ListDetailSceneStrategy.detailPane()) {
        ReportProblem(navigator = navigator)
    }
    entry<ReportPreviewNavKey>(metadata = ListDetailSceneStrategy.detailPane()) { key ->
        ReportPreview(navigator = navigator, path = key.path)
    }
    entry<ForumSignInCodeNavKey>(metadata = ListDetailSceneStrategy.detailPane()) {
        ForumSignInCode(navigator = navigator)
    }
}
