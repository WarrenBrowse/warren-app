package com.warrenbrowse.vpn.feature.home.impl.welcome

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun DeviceNameInfo(navigator: Navigator) {
    InfoDialog(
        message =
            buildString {
                appendLine(stringResource(id = R.string.device_name_info_first_paragraph))
                appendLine()
                appendLine(stringResource(id = R.string.device_name_info_second_paragraph))
                appendLine()
                append(stringResource(id = R.string.device_name_info_third_paragraph))
            },
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
