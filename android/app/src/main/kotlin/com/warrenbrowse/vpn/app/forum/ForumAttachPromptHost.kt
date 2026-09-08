package com.warrenbrowse.vpn.app.forum

import android.app.Activity
import android.widget.Toast
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.warrenbrowse.vpn.lib.repository.CollectedReport
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorSmall
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.koin.compose.koinInject

/** Largest slice of the report shown; the file itself is sent whole. */
private const val PREVIEW_MAX_CHARS = 400_000

/**
 * Consent prompt for attaching the redacted problem report to a forum bug
 * report (doc 55), the mirror of the desktop `ForumAttachPrompt`. The app
 * NEVER uploads silently: the report is collected and sent only after the
 * user approves here, and the exact report can be read first ("View the
 * logs"). Observes [ForumAttachController.pending]; a `warren://attach-logs`
 * link, or a session id typed by hand that the broker holds as an attach
 * session, shows the prompt. Declining tells the provider so the waiting
 * forum page shows "cancelled".
 *
 * A failure keeps the prompt open with the reason inline, as the login
 * prompt does: clearing it would discard the captured link and send the
 * person back through the browser for a transient failure.
 */
@Composable
fun ForumAttachPromptHost() {
    val controller = koinInject<ForumAttachController>()
    val useCase = koinInject<WarrenForumAttachUseCase>()
    val pending by controller.pending.collectAsState()
    val link = pending ?: return

    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // Keyed on the link's sid inside: a link replacing another while the
    // prompt is open starts clean instead of inheriting a disarmed Approve.
    val state = remember { ForumAttachPromptState() }
    state.bind(link)
    // The collected file behind the preview: deleted when the prompt goes or
    // a fresh collection replaces it, never left in the cache.
    var preview by remember { mutableStateOf<CollectedReport?>(null) }
    val messages = attachMessages()

    val dropPreview = {
        preview?.let(useCase::discard)
        preview = null
    }

    // Declining notifies the provider so the waiting forum page shows
    // "cancelled" (mirrors the desktop), then dismisses the prompt. After a
    // terminal refusal the provider already knows; only the prompt is left.
    val onDecline = {
        if (!state.busy) {
            if (!state.terminal) useCase.cancel(link)
            dropPreview()
            controller.clear()
        }
    }

    val onApprove = {
        val topicId = state.topicIdOrNull()
        if (!state.busy && controller.isStale()) {
            // The attach session died while the prompt sat here; sending now
            // can only fail on a dead sid.
            state.fail(messages.expiredFor(link))
        } else if (!state.busy && topicId != null) {
            // Keep the request pending (dialog stays) until the call returns,
            // so this composable does not leave composition and cancel the
            // coroutine mid-flight.
            state.begin()
            scope.launch {
                val outcome = useCase.attach(link, topicId)
                if (outcome is WarrenForumAttachOutcome.Attached) {
                    dropPreview()
                    controller.clear()
                    Toast.makeText(context, messages.attached, Toast.LENGTH_LONG).show()
                    // The forum page is what completes the flow, and it only
                    // re-polls once it is visible again: hand the foreground
                    // back to it, as the desktop hides its window.
                    (context as? Activity)?.moveTaskToBack(true)
                } else {
                    state.settle(outcome, messages.failureFor(outcome, link))
                }
            }
        }
    }

    // Nothing leaves the device for the preview: the live probes are taken by
    // the approval's own collection, not here.
    val onViewLogs = {
        if (!state.collecting && !state.busy) {
            state.beginCollect()
            scope.launch {
                useCase
                    .collectForPreview()
                    .fold(
                        onSuccess = { report ->
                            dropPreview()
                            preview = report
                            state.previewReady(report.file.absolutePath)
                        },
                        onFailure = { state.previewFailed() },
                    )
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDecline,
        title = { Text(stringResource(R.string.forum_attach_title)) },
        text = { AttachPromptBody(link, state, onViewLogs) },
        confirmButton = {
            PrimaryButton(
                text = stringResource(R.string.forum_attach_approve),
                isEnabled = state.canApprove,
                // Beside the label rather than in place of it: the action stays
                // named while the upload is in flight.
                leadingIcon =
                    if (state.busy) {
                        { WarrenCircularProgressIndicatorSmall() }
                    } else null,
                onClick = onApprove,
            )
        },
        dismissButton = {
            // Explicit colours: the theme's primary is a charcoal one shade
            // off the dialog surface, which left "Cancel" invisible.
            TextButton(
                onClick = onDecline,
                enabled = !state.busy,
                colors =
                    ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.onSurface,
                        disabledContentColor =
                            MaterialTheme.colorScheme.onSurface.copy(alpha = 0.4f),
                    ),
            ) {
                Text(stringResource(R.string.forum_login_cancel))
            }
        },
    )

    state.previewPath?.let { path -> AttachReportPreview(path = path, onClose = state::closePreview) }
}

/** What the request is, what leaves the device, the topic field, the preview, the inline notices. */
@Composable
private fun AttachPromptBody(
    link: ForumAttachLink,
    state: ForumAttachPromptState,
    onViewLogs: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
        val topicId = link.topicId
        Text(
            when {
                topicId == null -> stringResource(R.string.forum_attach_body_code)
                topicId == ForumAttachLink.PRE_TOPIC -> stringResource(R.string.forum_attach_body_pre_topic)
                else -> stringResource(R.string.forum_attach_body_topic, topicId)
            }
        )
        Text(stringResource(R.string.forum_attach_body_second))
        if (state.needsTopic) {
            OutlinedTextField(
                value = state.topicInput,
                onValueChange = state::updateTopicInput,
                modifier = Modifier.fillMaxWidth(),
                enabled = !state.busy,
                label = { Text(stringResource(R.string.forum_attach_topic_label)) },
                supportingText = { Text(stringResource(R.string.forum_attach_topic_hint)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            WarrenTextButton(onClick = onViewLogs, enabled = !state.collecting && !state.busy) {
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
                text = stringResource(R.string.forum_attach_collect_failed),
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
            )
        }
        state.failure?.let { reason ->
            Text(
                text = reason,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive },
            )
        }
        if (state.busy) {
            Text(
                text = stringResource(R.string.forum_attach_sending),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
    }
}

/**
 * The exact redacted report about to be sent, full screen over the prompt:
 * the Report-a-problem screen's preview, reachable from a dialog that has no
 * navigator of its own. Read-only; the approval stays on the prompt behind.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AttachReportPreview(path: String, onClose: () -> Unit) {
    var text by remember(path) { mutableStateOf<String?>(null) }
    LaunchedEffect(path) {
        text =
            withContext(Dispatchers.IO) {
                runCatching {
                    val content = File(path).readText()
                    if (content.length > PREVIEW_MAX_CHARS) {
                        content.take(PREVIEW_MAX_CHARS) + "\n[preview truncated]"
                    } else {
                        content
                    }
                }
                    .getOrNull()
            }
                ?: ""
    }
    Dialog(
        onDismissRequest = onClose,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        ScaffoldWithSmallTopBar(
            appBarTitle = stringResource(R.string.report_problem_preview_title),
            navigationIcon = { NavigateBackIconButton(onNavigateBack = onClose) },
        ) { modifier ->
            Column(
                modifier =
                    Modifier.fillMaxSize()
                        .then(modifier)
                        .verticalScroll(rememberScrollState())
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding)
            ) {
                Text(
                    text = text ?: stringResource(R.string.report_problem_collecting),
                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                    softWrap = false,
                )
            }
        }
    }
}

/**
 * The prompt's copy, resolved in composition so the click handlers, which run
 * outside it, hold plain strings.
 */
private class AttachMessages(
    val attached: String,
    private val notAuthor: String,
    private val expired: String,
    private val expiredCode: String,
    private val tooLarge: String,
    private val clockSkew: String,
    private val server: String,
    private val walletNotReady: String,
    private val tunnelBusy: String,
    private val generic: String,
) {
    /**
     * A typed code carries a topic number the person supplied, and the
     * provider answers a mismatch with the same 404 as an expiry, so that
     * message names both.
     */
    fun expiredFor(link: ForumAttachLink): String = if (link.topicId == null) expiredCode else expired

    fun failureFor(outcome: WarrenForumAttachOutcome, link: ForumAttachLink): String =
        when (outcome) {
            WarrenForumAttachOutcome.NotAuthor -> notAuthor
            WarrenForumAttachOutcome.Expired -> expiredFor(link)
            WarrenForumAttachOutcome.TooLarge -> tooLarge
            WarrenForumAttachOutcome.ClockSkew -> clockSkew
            WarrenForumAttachOutcome.ServerError -> server
            WarrenForumAttachOutcome.WalletNotReady -> walletNotReady
            is WarrenForumAttachOutcome.Deferred -> tunnelBusy
            WarrenForumAttachOutcome.Attached,
            is WarrenForumAttachOutcome.Failure -> generic
        }
}

@Composable
private fun attachMessages(): AttachMessages =
    AttachMessages(
        attached = stringResource(R.string.forum_attach_result_attached),
        notAuthor = stringResource(R.string.forum_attach_result_not_author),
        expired = stringResource(R.string.forum_attach_result_expired),
        expiredCode = stringResource(R.string.forum_attach_result_expired_code),
        tooLarge = stringResource(R.string.report_problem_error_too_large),
        clockSkew = stringResource(R.string.report_problem_error_clock),
        server = stringResource(R.string.forum_attach_result_server),
        walletNotReady = stringResource(R.string.forum_login_result_wallet_not_ready),
        tunnelBusy = stringResource(R.string.forum_tunnel_busy),
        generic = stringResource(R.string.forum_attach_result_failure),
    )
