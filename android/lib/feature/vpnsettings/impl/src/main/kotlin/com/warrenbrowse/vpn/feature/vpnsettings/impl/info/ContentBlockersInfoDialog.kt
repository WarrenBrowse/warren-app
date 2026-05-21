package com.warrenbrowse.vpn.feature.vpnsettings.impl.info

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun ContentBlockersInfo(navigator: Navigator) {
    InfoDialog(
        message =
            buildString {
                appendLine(stringResource(id = R.string.dns_content_blockers_info))
                append(stringResource(id = R.string.dns_content_blockers_warning))
            },
        additionalInfo =
            buildString {
                appendLine(stringResource(id = R.string.dns_content_blockers_custom_dns_warning))
                appendLine(
                    stringResource(id = R.string.settings_changes_effect_warning_content_blocker)
                )
            },
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
