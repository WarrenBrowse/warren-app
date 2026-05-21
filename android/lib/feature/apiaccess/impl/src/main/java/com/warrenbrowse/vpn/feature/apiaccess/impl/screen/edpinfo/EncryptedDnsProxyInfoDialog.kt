package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.edpinfo

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
private fun PreviewEncryptedDnsProxyInfo() {
    AppTheme { EncryptedDnsProxyInfo(EmptyNavigator) }
}

@Composable
fun EncryptedDnsProxyInfo(navigator: Navigator) {
    InfoDialog(
        message =
            buildString {
                appendLine(stringResource(id = R.string.encrypted_dns_proxy_info_message_part1))
                appendLine()
                appendLine(stringResource(id = R.string.encrypted_dns_proxy_info_message_part2))
            },
        onDismiss = navigator::goBack,
    )
}
