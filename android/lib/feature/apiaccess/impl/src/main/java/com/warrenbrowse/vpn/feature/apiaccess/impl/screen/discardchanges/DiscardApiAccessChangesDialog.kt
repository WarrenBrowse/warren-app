package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.discardchanges

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.DiscardApiAccessChangesConfirmedNavResult
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialogTitleType
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewApiAccessDiscardChangesDialog() {
    AppTheme { DiscardApiAccessChanges(EmptyNavigator) }
}

@Composable
fun DiscardApiAccessChanges(navigator: Navigator) {
    InfoConfirmationDialog(
        onResult = {
            if (it != null) {
                navigator.goBack(result = DiscardApiAccessChangesConfirmedNavResult)
            } else {
                navigator.goBack()
            }
        },
        titleType =
            InfoConfirmationDialogTitleType.TitleOnly(stringResource(R.string.discard_changes)),
        confirmButtonTitle = stringResource(R.string.discard),
        cancelButtonTitle = stringResource(R.string.cancel),
    )
}
