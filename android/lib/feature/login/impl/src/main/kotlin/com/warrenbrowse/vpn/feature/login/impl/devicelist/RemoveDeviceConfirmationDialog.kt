package com.warrenbrowse.vpn.feature.login.impl.devicelist

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.core.text.HtmlCompat
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.RemoveDeviceConfirmationDialogResult
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.toAnnotatedString
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewRemoveDeviceConfirmationDialog(
    @PreviewParameter(RemoveDeviceConfirmationPreviewParameterProvider::class) device: Device
) {
    AppTheme { RemoveDeviceConfirmation(EmptyNavigator, device = device) }
}

@Composable
fun RemoveDeviceConfirmation(navigator: Navigator, device: Device) {
    val htmlFormattedString =
        stringResource(id = R.string.max_devices_confirm_removal_description, device.displayName())
    val message =
        HtmlCompat.fromHtml(htmlFormattedString, HtmlCompat.FROM_HTML_MODE_COMPACT)
            .toAnnotatedString(
                boldSpanStyle =
                    SpanStyle(
                        color = MaterialTheme.colorScheme.onSurface,
                        fontWeight = FontWeight.Bold,
                    )
            )

    NegativeConfirmationDialog(
        message = message,
        messageStyle = MaterialTheme.typography.bodyMedium,
        messageColor = MaterialTheme.colorScheme.onSurfaceVariant,
        confirmationText = stringResource(id = R.string.confirm_removal),
        cancelText = stringResource(id = R.string.back),
        onBack = dropUnlessResumed { navigator.goBack() },
        onConfirm =
            dropUnlessResumed {
                navigator.goBack(result = RemoveDeviceConfirmationDialogResult(device.id))
            },
    )
}
