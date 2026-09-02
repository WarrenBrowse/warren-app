package com.warrenbrowse.vpn.feature.settings.impl.support

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.createUriHook
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.ReportPreviewNavKey
import com.warrenbrowse.vpn.lib.repository.ReportArea
import com.warrenbrowse.vpn.lib.repository.ReportFrequency
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorSmall
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenExposedDropdownMenuBox
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel

/**
 * The in-app bug report: the forum's "Report a bug" form, filed with the
 * wallet signature and the redacted logs through the connect broker, so a user
 * who cannot complete the browser sign-in can still be heard. The description
 * is public under the anonymous forum name; the logs go privately to the
 * support team, and can be read here before they leave the device (the
 * desktop's "View the logs").
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReportProblem(navigator: Navigator) {
    val viewModel = koinViewModel<ReportProblemViewModel>()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val uriHandler = LocalUriHandler.current
    val openHelpForm = uriHandler.createUriHook(stringResource(R.string.help_page_url))

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.report_problem),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
    ) { modifier ->
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .then(modifier)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            Text(
                text = stringResource(R.string.report_problem_intro),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            val fieldColors = ExposedDropdownMenuDefaults.textFieldColors()
            WarrenExposedDropdownMenuBox(
                label = stringResource(R.string.report_problem_area_label),
                title =
                    state.area?.let { stringResource(it.labelRes()) }
                        ?: stringResource(R.string.report_problem_pick_one),
                colors = fieldColors,
            ) { close ->
                ReportArea.entries.forEach { area ->
                    DropdownMenuItem(
                        text = { Text(stringResource(area.labelRes())) },
                        onClick = {
                            viewModel.setArea(area)
                            close()
                        },
                    )
                }
            }

            OutlinedTextField(
                value = state.whatHappened,
                onValueChange = viewModel::setWhatHappened,
                modifier = Modifier.fillMaxWidth().heightIn(min = 120.dp),
                label = { Text(stringResource(R.string.report_problem_what_happened_label)) },
                placeholder = { Text(stringResource(R.string.report_problem_what_happened_hint)) },
                supportingText = {
                    Text(
                        text =
                            stringResource(
                                R.string.report_problem_chars,
                                state.descriptionChars,
                                REPORT_MIN_DESCRIPTION_CHARS,
                            )
                    )
                },
                minLines = 4,
                enabled = !state.sending,
            )

            OutlinedTextField(
                value = state.steps,
                onValueChange = viewModel::setSteps,
                modifier = Modifier.fillMaxWidth(),
                label = { Text(stringResource(R.string.report_problem_steps_label)) },
                placeholder = { Text(stringResource(R.string.report_problem_steps_hint)) },
                minLines = 2,
                enabled = !state.sending,
            )

            WarrenExposedDropdownMenuBox(
                label = stringResource(R.string.report_problem_frequency_label),
                title =
                    state.frequency?.let { stringResource(it.labelRes()) }
                        ?: stringResource(R.string.report_problem_pick_one),
                colors = fieldColors,
            ) { close ->
                ReportFrequency.entries.forEach { frequency ->
                    DropdownMenuItem(
                        text = { Text(stringResource(frequency.labelRes())) },
                        onClick = {
                            viewModel.setFrequency(frequency)
                            close()
                        },
                    )
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(R.string.report_problem_include_logs),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(
                        text = stringResource(R.string.report_problem_include_logs_description),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                WarrenSwitch(
                    checked = state.includeLogs,
                    onCheckedChange = viewModel::setIncludeLogs,
                )
            }

            if (state.includeLogs) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    WarrenTextButton(
                        onClick = {
                            viewModel.collect { report ->
                                navigator.navigate(ReportPreviewNavKey(report.file.absolutePath))
                            }
                        },
                        enabled = !state.collecting && !state.sending,
                    ) {
                        Text(stringResource(R.string.report_problem_view_logs))
                    }
                    if (state.collecting) {
                        WarrenCircularProgressIndicatorSmall()
                        Text(
                            text = stringResource(R.string.report_problem_collecting),
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(start = Dimens.smallPadding),
                        )
                    }
                }
                if (state.collectFailed) {
                    Text(
                        text = stringResource(R.string.report_problem_collect_failed),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }

            state.outcome?.let { outcome ->
                ReportOutcomeNotice(
                    outcome = outcome,
                    onOpenTopic = { url -> uriHandler.openUri(url) },
                    onOpenHelpForm = openHelpForm,
                    onSendWithoutLogs = viewModel::sendWithoutLogs,
                )
            }

            PrimaryButton(
                text =
                    if (state.sending) stringResource(R.string.report_problem_sending)
                    else stringResource(R.string.report_problem_send),
                onClick = viewModel::send,
                isEnabled = state.canSend,
                modifier = Modifier.fillMaxWidth(),
                leadingIcon =
                    if (state.sending) {
                        { WarrenCircularProgressIndicatorSmall() }
                    } else null,
            )
        }
    }
}

@Composable
private fun ReportOutcomeNotice(
    outcome: ReportSubmitOutcome,
    onOpenTopic: (String) -> Unit,
    onOpenHelpForm: () -> Unit,
    onSendWithoutLogs: () -> Unit,
) {
    when (outcome) {
        is ReportSubmitOutcome.Created -> {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                Text(
                    text =
                        when (outcome.logs) {
                            "attached" -> stringResource(R.string.report_problem_created_with_logs)
                            "partial" -> stringResource(R.string.report_problem_created_partial)
                            else -> stringResource(R.string.report_problem_created)
                        },
                    color = MaterialTheme.colorScheme.tertiary,
                    style = MaterialTheme.typography.bodyMedium,
                )
                outcome.identity?.let { identity ->
                    Text(
                        text = stringResource(R.string.report_problem_posted_as, identity.handle),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                if (outcome.topicUrl.isNotEmpty()) {
                    WarrenTextButton(onClick = { onOpenTopic(outcome.topicUrl) }) {
                        Text(stringResource(R.string.report_problem_open_topic))
                    }
                }
            }
        }
        ReportSubmitOutcome.SubscriptionRequired ->
            ErrorNotice(stringResource(R.string.report_problem_error_subscription)) {
                WarrenTextButton(onClick = onOpenHelpForm) {
                    Text(stringResource(R.string.report_problem_open_help_form))
                }
            }
        ReportSubmitOutcome.ClockSkew -> ErrorNotice(stringResource(R.string.report_problem_error_clock))
        ReportSubmitOutcome.RateLimited -> ErrorNotice(stringResource(R.string.report_problem_error_rate_limited))
        ReportSubmitOutcome.TooLarge ->
            ErrorNotice(stringResource(R.string.report_problem_error_too_large)) {
                WarrenTextButton(onClick = onSendWithoutLogs) {
                    Text(stringResource(R.string.report_problem_send_without_logs))
                }
            }
        ReportSubmitOutcome.UploadTimedOut ->
            ErrorNotice(stringResource(R.string.report_problem_error_upload_timeout)) {
                WarrenTextButton(onClick = onSendWithoutLogs) {
                    Text(stringResource(R.string.report_problem_send_without_logs))
                }
            }
        ReportSubmitOutcome.Invalid -> ErrorNotice(stringResource(R.string.report_problem_error_invalid))
        ReportSubmitOutcome.ServerError -> ErrorNotice(stringResource(R.string.report_problem_error_server))
        ReportSubmitOutcome.WalletNotReady -> ErrorNotice(stringResource(R.string.forum_login_result_wallet_not_ready))
        is ReportSubmitOutcome.Deferred -> ErrorNotice(stringResource(R.string.forum_tunnel_busy))
        is ReportSubmitOutcome.Failure -> ErrorNotice(stringResource(R.string.report_problem_error_generic))
    }
}

@Composable
private fun ErrorNotice(text: String, action: @Composable () -> Unit = {}) {
    Column(verticalArrangement = Arrangement.spacedBy(Dimens.tinyPadding)) {
        Text(
            text = text,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyMedium,
        )
        action()
    }
}

private fun ReportArea.labelRes(): Int =
    when (this) {
        ReportArea.BROWSING -> R.string.report_problem_area_browsing
        ReportArea.CONNECTION -> R.string.report_problem_area_connection
        ReportArea.WALLET -> R.string.report_problem_area_wallet
        ReportArea.INSTALL -> R.string.report_problem_area_install
        ReportArea.OTHER -> R.string.report_problem_area_other
    }

private fun ReportFrequency.labelRes(): Int =
    when (this) {
        ReportFrequency.ALWAYS -> R.string.report_problem_frequency_always
        ReportFrequency.SOMETIMES -> R.string.report_problem_frequency_sometimes
        ReportFrequency.ONCE -> R.string.report_problem_frequency_once
    }
