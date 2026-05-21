package com.warrenbrowse.vpn.feature.login.impl

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialogTitleType
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Preview
@Composable
private fun PreviewCreateAccountConfirmationDialog() {
    AppTheme { CreateAccountConfirmation(EmptyNavigator) }
}

@Parcelize data class CreateAccountConfirmationDialogResult(val confirmed: Boolean) : NavResult

@Composable
fun CreateAccountConfirmation(navigator: Navigator) {
    InfoConfirmationDialog(
        onResult = {
            if (it != null) {
                navigator.goBack(result = CreateAccountConfirmationDialogResult(true))
            } else {
                navigator.goBack(result = CreateAccountConfirmationDialogResult(false))
            }
        },
        titleType = InfoConfirmationDialogTitleType.IconOnly,
        confirmButtonTitle = stringResource(R.string.create_new_account),
        cancelButtonTitle = stringResource(R.string.cancel),
    ) {
        Text(
            text = stringResource(id = R.string.create_new_account_warning_paragraph1),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.labelLarge,
            modifier = Modifier.fillMaxWidth(),
        )

        Spacer(modifier = Modifier.height(Dimens.verticalSpace))

        Text(
            text = stringResource(id = R.string.create_new_account_warning_paragraph2),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.labelLarge,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
