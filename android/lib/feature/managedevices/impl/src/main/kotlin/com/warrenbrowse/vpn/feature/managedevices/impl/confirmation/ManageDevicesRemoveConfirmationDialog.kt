package com.warrenbrowse.vpn.feature.managedevices.impl.confirmation

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewManageDevicesRemoveConfirmationDialog() {
    AppTheme { ManageDevicesRemoveConfirmation(name = "quiet heron", onConfirm = {}, onBack = {}) }
}

/**
 * Confirms disconnecting another device of the account: it is revoked, and only the recovery
 * phrase brings it back, which is also why a lost or stolen device calls for a new account.
 */
@Composable
fun ManageDevicesRemoveConfirmation(name: String, onConfirm: () -> Unit, onBack: () -> Unit) {
    NegativeConfirmationDialog(
        message = stringResource(R.string.device_disconnect_confirm_title, name),
        messageStyle = MaterialTheme.typography.titleMedium,
        confirmationText = stringResource(R.string.device_disconnect),
        body = {
            Text(
                text = stringResource(R.string.device_disconnect_confirm_body),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.labelLarge,
            )
        },
        onConfirm = onConfirm,
        onBack = onBack,
    )
}
