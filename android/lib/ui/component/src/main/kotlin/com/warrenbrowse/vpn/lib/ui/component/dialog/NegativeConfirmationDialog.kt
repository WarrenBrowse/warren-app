package com.warrenbrowse.vpn.lib.ui.component.dialog

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Error
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Preview
@Composable
private fun PreviewDeleteConfirmationDialog() {
    AppTheme {
        NegativeConfirmationDialog(
            message = "Do you want to delete Cookie?",
            errorMessage = null,
            onConfirm = {},
            onBack = {},
        )
    }
}

@Preview
@Composable
private fun PreviewDeleteConfirmationDialogError() {
    AppTheme {
        NegativeConfirmationDialog(
            message = "Do you want to delete Cookie?",
            errorMessage = "An error occurred",
            onConfirm = {},
            onBack = {},
        )
    }
}

@Composable
fun NegativeConfirmationDialog(
    message: String,
    messageStyle: TextStyle? = null,
    messageColor: Color? = null,
    errorMessage: String? = null,
    confirmationText: String = stringResource(id = R.string.delete),
    cancelText: String = stringResource(id = R.string.cancel),
    isConfirmEnabled: Boolean = true,
    body: (@Composable () -> Unit)? = null,
    secondaryAction: (@Composable () -> Unit)? = null,
    onConfirm: () -> Unit,
    onBack: () -> Unit,
) {
    NegativeConfirmationDialog(
        message = AnnotatedString(message),
        messageStyle = messageStyle,
        messageColor = messageColor,
        errorMessage = errorMessage,
        confirmationText = confirmationText,
        cancelText = cancelText,
        isConfirmEnabled = isConfirmEnabled,
        body = body,
        secondaryAction = secondaryAction,
        onConfirm = onConfirm,
        onBack = onBack,
    )
}

/**
 * Destructive confirmation: error icon, the consequence as the message, and a
 * red confirm that is never the safe default (Cancel takes the initial focus).
 *
 * [body] carries whatever the caller must show under the message, typically an
 * acknowledgement checkbox gating [isConfirmEnabled]. [secondaryAction] is an
 * escape hatch button that belongs with the other actions rather than buried in
 * the message, mirroring the desktop modal button group.
 */
@Composable
fun NegativeConfirmationDialog(
    message: AnnotatedString,
    messageStyle: TextStyle? = null,
    messageColor: Color? = null,
    errorMessage: String? = null,
    confirmationText: String = stringResource(id = R.string.delete),
    cancelText: String = stringResource(id = R.string.cancel),
    isConfirmEnabled: Boolean = true,
    body: (@Composable () -> Unit)? = null,
    secondaryAction: (@Composable () -> Unit)? = null,
    onConfirm: () -> Unit,
    onBack: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onBack,
        text = body,
        icon = {
            Icon(
                modifier = Modifier.fillMaxWidth().height(Dimens.dialogIconHeight),
                imageVector = Icons.Rounded.Error,
                contentDescription = stringResource(id = R.string.remove_button),
                tint = MaterialTheme.colorScheme.error,
            )
        },
        title = {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    text = message,
                    style = messageStyle ?: LocalTextStyle.current,
                    color = messageColor ?: LocalTextStyle.current.color,
                )
                if (errorMessage != null) {
                    Text(
                        text = errorMessage,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = Dimens.smallPadding),
                    )
                }
            }
        },
        dismissButton = {
            val focusRequester = remember { FocusRequester() }
            // The safe action takes the focus, like the desktop autofocused
            // Cancel: a confirm that already holds focus is one stray Enter
            // away from running.
            LaunchedEffect(Unit) { focusRequester.requestFocus() }
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing)) {
                secondaryAction?.invoke()
                PrimaryButton(
                    modifier = Modifier.focusRequester(focusRequester),
                    onClick = onBack,
                    text = cancelText,
                )
            }
        },
        confirmButton = {
            NegativeButton(
                onClick = onConfirm,
                text = confirmationText,
                isEnabled = isConfirmEnabled,
            )
        },
        containerColor = MaterialTheme.colorScheme.surface,
    )
}
