package com.warrenbrowse.vpn.feature.vpnsettings.impl.info

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewCustomDnsInfoDialog() {
    AppTheme { CustomDnsInfo(EmptyNavigator) }
}

@Composable
fun CustomDnsInfo(navigator: Navigator) {
    InfoDialog(
        message = stringResource(id = R.string.settings_changes_effect_warning_content_blocker),
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
