package com.warrenbrowse.vpn.feature.vpnsettings.impl.info

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.HTML_NEWLINE_STRING
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewLocalNetworkSharingInfoDialog() {
    AppTheme { LocalNetworkSharingInfo(EmptyNavigator) }
}

@Composable
fun LocalNetworkSharingInfo(navigator: Navigator) {
    InfoDialog(
        message = stringResource(id = R.string.local_network_sharing_info),
        additionalInfo =
            buildString {
                appendLine(stringResource(id = R.string.local_network_sharing_additional_info))
                appendLine(stringResource(id = R.string.local_network_sharing_ip_ranges))
                // A html linebreak is specifically added since a normal linebreak is
                // removed by the html parser
                appendLine(HTML_NEWLINE_STRING)
                appendLine(
                    stringResource(
                        id = R.string.local_network_sharing_info_block_connections_warning
                    )
                )
            },
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
