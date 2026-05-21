package com.warrenbrowse.vpn.feature.managedevices.impl.confirmation

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.managedevices.api.ManageDevicesRemoveConfirmationNavResult
import com.warrenbrowse.vpn.feature.managedevices.impl.R
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialogTitleType
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewManageDevicesRemoveConfirmationDialog(
    @PreviewParameter(ManageDeviceRemoveConfirmationPreviewParameterProvider::class) device: Device
) {
    AppTheme { ManageDevicesRemoveConfirmation(EmptyNavigator, device = device) }
}

@Composable
fun ManageDevicesRemoveConfirmation(navigator: Navigator, device: Device) {
    InfoConfirmationDialog(
        onResult = {
            if (it != null) {
                navigator.goBack(result = ManageDevicesRemoveConfirmationNavResult(it))
            } else {
                navigator.goBack()
            }
        },
        confirmValue = device.id,
        titleType = InfoConfirmationDialogTitleType.IconAndTitle(title = device.titleText()),
        confirmButtonTitle = stringResource(R.string.remove_button),
        cancelButtonTitle = stringResource(R.string.cancel),
    ) {
        Text(
            text = stringResource(id = R.string.manage_devices_confirm_removal_description_line2),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.labelLarge,
        )
    }
}

@Composable
private fun Device.titleText(): String =
    stringResource(id = R.string.manage_devices_confirm_removal_description_line1, displayName())
