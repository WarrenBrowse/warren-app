package com.warrenbrowse.vpn.feature.customlist.impl.screen.discard

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.DiscardCustomListChangesConfirmedNavResult
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialogTitleType
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewDiscardChangesDialog() {
    AppTheme { DiscardChanges(EmptyNavigator) }
}

@Composable
fun DiscardChanges(navigator: Navigator) {
    InfoConfirmationDialog(
        onResult = {
            if (it != null) {
                navigator.goBack(result = DiscardCustomListChangesConfirmedNavResult)
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
