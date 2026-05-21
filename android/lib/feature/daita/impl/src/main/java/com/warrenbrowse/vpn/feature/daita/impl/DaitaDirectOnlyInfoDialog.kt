package com.warrenbrowse.vpn.feature.daita.impl

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewDaitaDirectOnlyInfoDialog() {
    AppTheme { DaitaDirectOnlyInfo(EmptyNavigator) }
}

@Composable
fun DaitaDirectOnlyInfo(navigator: Navigator) {
    InfoDialog(
        message =
            stringResource(
                id = R.string.daita_info,
                stringResource(id = R.string.direct_only),
                stringResource(id = R.string.daita),
            ),
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
