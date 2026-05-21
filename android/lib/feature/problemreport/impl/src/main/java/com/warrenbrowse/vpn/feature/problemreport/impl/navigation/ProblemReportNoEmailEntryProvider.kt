package com.warrenbrowse.vpn.feature.problemreport.impl.navigation

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.scene.DialogSceneStrategy
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.problemreport.api.ProblemReportNoEmailNavKey
import com.warrenbrowse.vpn.feature.problemreport.impl.noemail.ReportProblemNoEmail

internal fun EntryProviderScope<NavKey2>.problemReportNoEmailEntry(navigator: Navigator) {
    entry<ProblemReportNoEmailNavKey>(metadata = DialogSceneStrategy.dialog()) {
        ReportProblemNoEmail(navigator = navigator)
    }
}
