package com.warrenbrowse.vpn.feature.problemreport.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.feature.problemreport.api.ViewLogsNavKey
import com.warrenbrowse.vpn.feature.problemreport.impl.viewlogs.ViewLogs

internal fun EntryProviderScope<NavKey2>.viewLogsReportEntry(navigator: Navigator) {
    entry<ViewLogsNavKey>(metadata = slideInHorizontalTransition()) {
        ViewLogs(navigator = navigator)
    }
}
