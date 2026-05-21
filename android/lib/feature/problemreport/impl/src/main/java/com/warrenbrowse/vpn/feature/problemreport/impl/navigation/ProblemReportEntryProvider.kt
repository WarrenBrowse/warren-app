package com.warrenbrowse.vpn.feature.problemreport.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.slideInHorizontalTransition
import com.warrenbrowse.vpn.core.scene.ListDetailSceneStrategy
import com.warrenbrowse.vpn.feature.problemreport.api.ProblemReportNavKey
import com.warrenbrowse.vpn.feature.problemreport.impl.ReportProblem

fun EntryProviderScope<NavKey2>.problemReportEntry(navigator: Navigator) {
    entry<ProblemReportNavKey>(
        metadata = ListDetailSceneStrategy.detailPane() + slideInHorizontalTransition()
    ) {
        ReportProblem(navigator = navigator)
    }

    problemReportNoEmailEntry(navigator)
    viewLogsReportEntry(navigator)
}
