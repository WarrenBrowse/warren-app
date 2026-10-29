package com.warrenbrowse.vpn.lib.ui.component.dialog

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Preview
@Composable
private fun PreviewLogoutConfirmationDialog() {
    AppTheme { LogoutConfirmationDialog(onLogOut = {}, onBackUpPhrase = {}, onDismiss = {}) }
}

/**
 * Logging out erases the wallet from the device, and the recovery phrase is the only way back, so
 * the destructive action waits for an explicit "I backed up my phrase" (desktop logout modal).
 * Shared by the account screen and the device list, whose "Log out" is the same act. The escape
 * hatch sits with the actions, not in the message.
 */
@Composable
fun LogoutConfirmationDialog(
    onLogOut: () -> Unit,
    onBackUpPhrase: () -> Unit,
    onDismiss: () -> Unit,
) {
    var backedUp by remember { mutableStateOf(false) }
    NegativeConfirmationDialog(
        message = stringResource(R.string.wallet_settings_erase_confirm_title),
        messageStyle = MaterialTheme.typography.titleMedium,
        confirmationText = stringResource(R.string.wallet_settings_erase_confirm_action),
        isConfirmEnabled = backedUp,
        body = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
                Text(stringResource(R.string.account_erase_body_device))
                Text(stringResource(R.string.account_erase_body_backup))
                Row(
                    modifier = Modifier.fillMaxWidth().clickable { backedUp = !backedUp },
                    horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = backedUp, onCheckedChange = { backedUp = it })
                    Text(stringResource(R.string.wallet_settings_erase_backup_ack))
                }
            }
        },
        secondaryAction = {
            VariantButton(
                onClick = onBackUpPhrase,
                text = stringResource(R.string.account_backup_my_phrase),
            )
        },
        onConfirm = onLogOut,
        onBack = onDismiss,
    )
}
