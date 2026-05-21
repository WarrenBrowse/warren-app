package com.warrenbrowse.vpn.feature.vpnsettings.impl.info

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun Ipv6Info(navigator: Navigator) {
    InfoDialog(
        message = stringResource(R.string.ipv6_info),
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
