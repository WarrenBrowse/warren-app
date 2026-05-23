package com.warrenbrowse.vpn.feature.problemreport.impl

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.common.compose.SecureScreenWhileInView
import com.warrenbrowse.vpn.common.compose.isTv
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.problemreport.api.ProblemReportNoEmailConfirmedNavResult
import com.warrenbrowse.vpn.feature.problemreport.api.ProblemReportNoEmailNavKey
import com.warrenbrowse.vpn.feature.problemreport.api.ViewLogsNavKey
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.textfield.ErrorSupportingText
import com.warrenbrowse.vpn.lib.ui.component.textfield.warrenDarkTextFieldColors
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning
import org.koin.androidx.compose.koinViewModel

@Preview("Default|IncludeAccountNumber|ShowWarning|Sending|Success|Error")
@Composable
private fun PreviewReportProblemScreen(
    @PreviewParameter(ReportProblemUiStatePreviewParameterProvider::class)
    state: ReportProblemUiState
) {
    AppTheme {
        ReportProblemScreen(
            state = state,
            onSendReport = {},
            onClearSendResult = {},
            onNavigateToViewLogs = {},
            onEmailChanged = {},
            onDescriptionChanged = {},
            onBackClick = {},
        )
    }
}

@Composable
fun ReportProblem(navigator: Navigator) {
    val vm = koinViewModel<ReportProblemViewModel>()
    val state by vm.uiState.collectAsStateWithLifecycle()
    // D.6: BiometricPrompt (raised by WarrenSendProblemReportUseCase
    // to unlock the wallet mnemonic that signs /v1/support) needs a
    // FragmentActivity host. Resolved lazily inside each send-trigger
    // lambda via a safe `as?` cast — if the host activity is somehow
    // not a FragmentActivity (custom TV shell, instrumentation harness,
    // ...) the click is a no-op rather than a runtime crash.
    val context = androidx.compose.ui.platform.LocalContext.current

    fun triggerSend(skipEmptyEmailCheck: Boolean = false) {
        val activity = context as? androidx.fragment.app.FragmentActivity
        if (activity == null) {
            co.touchlab.kermit.Logger.w(
                "ReportProblem: host context is not a FragmentActivity; ignoring send tap"
            )
            return
        }
        vm.sendReport(activity, state.email, state.description, skipEmptyEmailCheck)
    }

    CollectSideEffectWithLifecycle(vm.uiSideEffect) {
        when (it) {
            is ReportProblemSideEffect.ShowConfirmNoEmail ->
                navigator.navigate(ProblemReportNoEmailNavKey)
        }
    }

    LocalResultStore.current.consumeResult<ProblemReportNoEmailConfirmedNavResult> {
        triggerSend(skipEmptyEmailCheck = true)
    }

    ReportProblemScreen(
        state = state,
        onSendReport = { triggerSend() },
        onClearSendResult = vm::clearSendResult,
        onNavigateToViewLogs = dropUnlessResumed { navigator.navigate(ViewLogsNavKey) },
        onEmailChanged = vm::updateEmail,
        onDescriptionChanged = vm::updateDescription,
        onBackClick = dropUnlessResumed { navigator.goBack() },
    )
}

@Composable
private fun ReportProblemScreen(
    state: ReportProblemUiState,
    onSendReport: () -> Unit,
    onClearSendResult: () -> Unit,
    onNavigateToViewLogs: () -> Unit,
    onEmailChanged: (String) -> Unit,
    onDescriptionChanged: (String) -> Unit,
    onBackClick: () -> Unit,
) {

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.report_a_problem),
        navigationIcon = { unlessIsDetail { NavigateBackIconButton(onNavigateBack = onBackClick) } },
    ) { modifier ->
        // Show sending states
        if (state.sendingState != null) {
            Column(
                modifier =
                    modifier.padding(
                        vertical = Dimens.mediumPadding,
                        horizontal = Dimens.sideMargin,
                    )
            ) {
                when (state.sendingState) {
                    SendingReportUiState.Sending -> SendingContent()
                    is SendingReportUiState.Error -> ErrorContent(onSendReport, onClearSendResult)
                    is SendingReportUiState.Success -> SentContent(state.sendingState)
                }
            }
        } else {
            val scrollState = rememberScrollState()
            Column(
                modifier =
                    Modifier
                        .imePadding() // imePadding needs to be applied before the parent modifier.
                        .then(modifier)
                        .drawVerticalScrollbar(
                            state = scrollState,
                            color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                        )
                        .verticalScroll(state = scrollState)
                        .padding(
                            start = Dimens.sideMargin,
                            end = Dimens.sideMargin,
                            bottom = Dimens.screenBottomMargin,
                        )
                        .height(IntrinsicSize.Max)
                        .animateContentSize(),
                verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
            ) {
                InputContent(
                    state = state,
                    onEmailChanged = onEmailChanged,
                    onDescriptionChanged = onDescriptionChanged,
                    onNavigateToViewLogs = onNavigateToViewLogs,
                    onSendReport = onSendReport,
                )
            }
        }
    }
}

@Composable
private fun InputContent(
    state: ReportProblemUiState,
    onEmailChanged: (String) -> Unit,
    onDescriptionChanged: (String) -> Unit,
    onNavigateToViewLogs: () -> Unit,
    onSendReport: () -> Unit,
) {
    Description()

    TextField(
        modifier = Modifier.fillMaxWidth(),
        value = state.email,
        onValueChange = onEmailChanged,
        maxLines = 1,
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(textDirection = TextDirection.Ltr),
        placeholder = { Text(text = stringResource(id = R.string.user_email_hint)) },
        colors = warrenDarkTextFieldColors(),
        keyboardOptions =
            KeyboardOptions(
                autoCorrectEnabled = false,
                keyboardType = KeyboardType.Email,
                imeAction = ImeAction.Next,
            ),
    )

    ProblemMessageTextField(
        value = state.description,
        isError = state.descriptionError != null,
        onDescriptionChanged = onDescriptionChanged,
    )

    // D.4 step 57: "include my account ID" checkbox + privacy-policy warning
    // dropped (Mullvad account-number is dead on Warren ; the warren-api
    // /v1/support endpoint will sign with the BIP39 wallet pubkey in D.6).

    Column {
        PrimaryButton(
            onClick = onNavigateToViewLogs,
            text = stringResource(id = R.string.view_logs),
            isEnabled = state.logCollectingState == LogCollectingState.Success,
            isLoading = state.logCollectingState == LogCollectingState.Loading,
        )
        Spacer(modifier = Modifier.height(Dimens.buttonSpacing))
        VariantButton(onClick = onSendReport, text = stringResource(id = R.string.send))
    }
}

@Composable
private fun Description() {
    Column {
        Text(
            text = stringResource(id = R.string.problem_report_description),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.labelLarge,
        )
    }
}

// D.4 step 57: IncludeAccountInformationCheckBox + AccountInformationWarning
// helpers + the entire account-token disclosure UI dropped (Mullvad account-
// token flow dead on Warren).

@Composable
private fun ProblemMessageTextField(
    modifier: Modifier = Modifier,
    value: String,
    isError: Boolean,
    onDescriptionChanged: (String) -> Unit,
) {

    TextField(
        modifier =
            modifier
                .fillMaxWidth()
                .defaultMinSize(minHeight = Dimens.problemReportTextFieldMinHeight),
        value = value,
        onValueChange = onDescriptionChanged,
        placeholder = { Text(stringResource(R.string.user_message_hint)) },
        isError = isError,
        supportingText =
            if (isError) {
                { ErrorSupportingText(stringResource(R.string.report_problem_message_is_empty)) }
            } else null,
        colors = warrenDarkTextFieldColors(),
        keyboardOptions =
            KeyboardOptions(
                capitalization = KeyboardCapitalization.Sentences,
                keyboardType = KeyboardType.Text,
                imeAction = if (isTv()) ImeAction.Next else ImeAction.Unspecified,
            ),
    )
}

@Composable
private fun ColumnScope.SendingContent() {
    WarrenCircularProgressIndicatorLarge(modifier = Modifier.align(Alignment.CenterHorizontally))
    Spacer(modifier = Modifier.height(Dimens.mediumSpacer))
    Text(
        text = stringResource(id = R.string.sending),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun ColumnScope.SentContent(sendingState: SendingReportUiState.Success) {
    SecureScreenWhileInView()
    Icon(
        painter = painterResource(id = R.drawable.icon_success),
        contentDescription = stringResource(id = R.string.sent),
        modifier = Modifier.align(Alignment.CenterHorizontally),
        tint = Color.Unspecified,
    )

    Spacer(modifier = Modifier.height(Dimens.mediumSpacer))
    Text(
        text = stringResource(id = R.string.sent),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
    )
    Text(
        text =
            buildAnnotatedString {
                withStyle(SpanStyle(color = MaterialTheme.colorScheme.positive)) {
                    append(stringResource(id = R.string.sent_thanks))
                }
                append(" ")
                withStyle(SpanStyle(color = MaterialTheme.colorScheme.onSurface)) {
                    append(stringResource(id = R.string.we_will_look_into_this))
                }
            },
        style = MaterialTheme.typography.bodyMedium,
        modifier = Modifier.fillMaxWidth(),
    )

    Spacer(modifier = Modifier.height(Dimens.smallPadding))
    sendingState.email?.let {
        val emailTemplate = stringResource(R.string.sent_contact)
        val annotatedEmailString =
            remember(it) {
                val emailStart = emailTemplate.indexOf('%')

                buildAnnotatedString {
                    append(emailTemplate.take(emailStart))
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold)) {
                        append(sendingState.email)
                    }
                }
            }

        Text(
            text = annotatedEmailString,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun ColumnScope.ErrorContent(retry: () -> Unit, onDismiss: () -> Unit) {
    Icon(
        painter = painterResource(id = R.drawable.icon_fail),
        contentDescription = stringResource(id = R.string.failed_to_send),
        modifier = Modifier.align(Alignment.CenterHorizontally),
        tint = Color.Unspecified,
    )
    Spacer(modifier = Modifier.height(Dimens.mediumSpacer))
    Text(
        text = stringResource(id = R.string.failed_to_send),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
    )
    Text(
        text = stringResource(id = R.string.failed_to_send_details),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(modifier = Modifier.weight(1f))
    PrimaryButton(
        modifier =
            Modifier.fillMaxWidth()
                .padding(top = Dimens.mediumPadding, bottom = Dimens.buttonSpacing),
        onClick = onDismiss,
        text = stringResource(id = R.string.edit_message),
    )
    VariantButton(
        modifier = Modifier.fillMaxWidth(),
        onClick = retry,
        text = stringResource(id = R.string.try_again),
    )
}
