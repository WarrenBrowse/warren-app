package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.fragment.app.FragmentActivity
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorSmall
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * Voucher redemption dialog, mirroring the desktop `RedeemVoucherAlert`: a
 * Crockford-32 input filtered live and displayed in 4-4-4-4 groups, a Redeem
 * action enabled only on a complete code, and a success state naming both the
 * credited duration and the new expiry. Shared by the account screen and the
 * out-of-time screen.
 *
 * The failure message is a single generic line: the JNI voucher result carries
 * only a loggable error string, not the desktop's typed invalid / already-used
 * / expired verdicts, so pretending to distinguish them would be guesswork.
 */
@Composable
fun RedeemVoucherDialog(onDismiss: () -> Unit) {
    val activity = LocalContext.current as FragmentActivity
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val scope = rememberCoroutineScope()

    var input by remember { mutableStateOf("") }
    var submitting by remember { mutableStateOf(false) }
    var errorText by remember { mutableStateOf<String?>(null) }
    var successExpiry by remember { mutableStateOf<Long?>(null) }
    // How much time the voucher added: the redeem answer carries the new expiry
    // only, so the credit is the distance from where the account stood before.
    var credited by remember { mutableStateOf<RemainingTime>(RemainingTime.None) }

    val redeemed = successExpiry
    if (redeemed != null) {
        AlertDialog(
            onDismissRequest = onDismiss,
            icon = {
                Icon(
                    imageVector = Icons.Rounded.CheckCircle,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.positive,
                    modifier = Modifier.size(Dimens.mediumIconSize),
                )
            },
            title = { Text(stringResource(R.string.voucher_success_title)) },
            text = {
                val expiry = formatExpiryDateTime(redeemed)
                Text(
                    when (credited) {
                        RemainingTime.None ->
                            stringResource(R.string.voucher_success_paid_until, expiry)
                        else ->
                            stringResource(
                                R.string.voucher_success_credited,
                                remainingTimeText(credited),
                                expiry,
                            )
                    }
                )
            },
            confirmButton = {
                WarrenTextButton(onClick = onDismiss) { Text(stringResource(R.string.got_it)) }
            },
        )
        return
    }

    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(Unit) { focusRequester.requestFocus() }

    // Shared by the Redeem button and the IME Done key, so a complete code can
    // be submitted without dismissing the keyboard to reach the button.
    val submit: () -> Unit = {
        submitting = true
        errorText = null
        scope.launch {
            val previousExpiry = settings.cachedSubscriptionExpiry.value
            val outcome = subscriptionInvoker.redeemVoucher(activity, input)
            submitting = false
            if (outcome is WarrenVoucherOutcome.Success) {
                // Write the credited expiry to the shared StateFlow so
                // every observer (paid-until row, home footer, the
                // out-of-time gate) refreshes reactively.
                settings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                credited =
                    creditedTime(
                        previousExpiryUnixSecs = previousExpiry,
                        newExpiryUnixSecs = outcome.expiresAtUnixSecs,
                        nowSecs = System.currentTimeMillis() / 1000,
                    )
                successExpiry = outcome.expiresAtUnixSecs
            } else {
                errorText = voucherLabel(activity, outcome)
            }
        }
    }

    AlertDialog(
        onDismissRequest = { if (!submitting) onDismiss() },
        title = { Text(stringResource(R.string.voucher_enter_code)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                OutlinedTextField(
                    value = input,
                    onValueChange = {
                        input = filterVoucherInput(it)
                        errorText = null
                    },
                    modifier = Modifier.fillMaxWidth().focusRequester(focusRequester),
                    placeholder = { Text("XXXX-XXXX-XXXX-XXXX") },
                    textStyle = MaterialTheme.typography.bodyLarge.copy(
                        fontFamily = FontFamily.Monospace,
                    ),
                    visualTransformation = VoucherVisualTransformation,
                    singleLine = true,
                    enabled = !submitting,
                    // A voucher is uppercase Crockford-32: suggestions and
                    // sentence capitalization only get in the way of a code the
                    // dictionary will never contain.
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Characters,
                        autoCorrectEnabled = false,
                        keyboardType = KeyboardType.Ascii,
                        imeAction = ImeAction.Done,
                    ),
                    keyboardActions = KeyboardActions(
                        onDone = { if (isCompleteVoucher(input) && !submitting) submit() },
                    ),
                )
                if (submitting) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
                    ) {
                        WarrenCircularProgressIndicatorSmall()
                        Text(
                            text = stringResource(R.string.voucher_verifying),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
                errorText?.let { msg ->
                    Text(
                        text = msg,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = Dimens.tinyPadding),
                    )
                }
            }
        },
        confirmButton = {
            WarrenTextButton(
                enabled = isCompleteVoucher(input) && !submitting,
                onClick = submit,
            ) { Text(stringResource(R.string.voucher_redeem)) }
        },
        dismissButton = {
            WarrenTextButton(enabled = !submitting, onClick = onDismiss) {
                Text(stringResource(R.string.cancel))
            }
        },
    )
}

/**
 * Display-only 4-4-4-4 grouping of the raw 16-char voucher: a dash is drawn
 * after every full group that has a following character, so the stored value
 * stays the raw secret and the cursor math survives paste and mid-edit.
 */
internal object VoucherVisualTransformation : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val grouped = buildString {
            raw.forEachIndexed { index, c ->
                if (index > 0 && index % 4 == 0) append('-')
                append(c)
            }
        }
        return TransformedText(
            AnnotatedString(grouped),
            object : OffsetMapping {
                // Chars at raw offset o are preceded by (o - 1) / 4 dashes.
                override fun originalToTransformed(offset: Int): Int =
                    if (offset <= 0) 0 else offset + (offset - 1) / 4

                override fun transformedToOriginal(offset: Int): Int =
                    offset - offset / 5
            },
        )
    }
}
