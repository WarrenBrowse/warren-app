package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.info

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewApiAccessMethodInfoDialog() {
    AppTheme { ApiAccessMethodInfo(EmptyNavigator) }
}

@Composable
fun ApiAccessMethodInfo(navigator: Navigator) {
    InfoDialog(
        message =
            buildString {
                appendLine(stringResource(id = R.string.api_access_method_info_first_line))
                appendLine()
                appendLine(stringResource(id = R.string.api_access_method_info_second_line))
                appendLine()
                appendLine(stringResource(id = R.string.api_access_method_info_third_line))
                appendLine()
                appendLine(stringResource(id = R.string.api_access_method_info_fourth_line))
            },
        onDismiss = navigator::goBack,
    )
}
