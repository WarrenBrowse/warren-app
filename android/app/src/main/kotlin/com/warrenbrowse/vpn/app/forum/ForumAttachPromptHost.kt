package com.warrenbrowse.vpn.app.forum

import android.app.Activity
import android.widget.Toast
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
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

/** Largest prefix of the report read for the preview; the file itself is sent whole. */
private const val PREVIEW_MAX_BYTES = 400_000

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
 * The prompt state and the upload live on the controller, not here: a
 * rotation recreates this composable while the upload is out, and a host
 * that owned them would re-arm Approve over it and drop the outcome.
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
    val scope = controller.scope
    // Keyed on the link's sid inside: a link replacing another while the
    // prompt is open starts clean instead of inheriting a disarmed Approve.
    val state = controller.prompt
    state.bind(link)
    val messages = attachMessages()

    val dropPreview = {
        state.preview?.let(useCase::discard)
    }

    // The provider attached (or parked) the report: the prompt closes and
    // the foreground goes back to the forum page, which is what completes
    // the flow and only re-polls once it is visible again (as the desktop
    // hides its window). Reacted to from the state so a host recreated
    // mid-flight does it too.
    LaunchedEffect(state.attached) {
        if (state.attached) {
            dropPreview()
            controller.clear()
            Toast.makeText(context, messages.attachedFor(link), Toast.LENGTH_LONG).show()
            (context as? Activity)?.moveTaskToBack(true)
        }
    }

    // Declining tells the provider so the waiting forum page shows
    // "cancelled" (mirrors the desktop), then dismisses the prompt. Only a
    // session the provider already reported gone is left alone: a refusal as
    // author or a report over the cap leaves it pending, and the page waiting.
    val onDecline = {
        if (!state.busy) {
            if (state.cancelsOnDecline) useCase.cancel(link)
            dropPreview()
            controller.clear()
        }
    }

    val onApprove = {
        if (!state.busy && controller.isStale()) {
            // The attach session died while the prompt sat here; sending now
            // can only fail on a dead sid.
            state.fail(messages.expired)
        } else if (!state.busy) {
            state.begin()
            scope.launch {
                when (val outcome = useCase.attach(link, link.topicId)) {
                    WarrenForumAttachOutcome.Attached -> state.markAttached()
                    else -> state.settle(outcome, messages.failureFor(outcome))
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
                            state.previewReady(report)
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

/** What the request is, what leaves the device, the preview, the inline notices. */
@Composable
private fun AttachPromptBody(
    link: ForumAttachLink,
    state: ForumAttachPromptState,
    onViewLogs: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
        Text(
            if (link.isPreTopic) {
                stringResource(R.string.forum_attach_body_pre_topic)
            } else {
                stringResource(R.string.forum_attach_body_topic, link.topicId)
            }
        )
        Text(stringResource(R.string.forum_attach_body_second))
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
 * The first [PREVIEW_MAX_BYTES] of the report, read without loading the rest:
 * a report can run to tens of megabytes and the screen shows one slice of it.
 */
private fun readPreview(file: File): String {
    val buffer = ByteArray(PREVIEW_MAX_BYTES + 1)
    var read = 0
    file.inputStream().use { input ->
        while (read < buffer.size) {
            val n = input.read(buffer, read, buffer.size - read)
            if (n < 0) break
            read += n
        }
    }
    val shown = String(buffer, 0, minOf(read, PREVIEW_MAX_BYTES), Charsets.UTF_8)
    return if (read > PREVIEW_MAX_BYTES) shown + "\n[preview truncated]" else shown
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
        text = withContext(Dispatchers.IO) { runCatching { readPreview(File(path)) }.getOrNull() } ?: ""
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
    private val attached: String,
    private val received: String,
    private val notAuthor: String,
    val expired: String,
    private val tooLarge: String,
    private val clockSkew: String,
    private val server: String,
    private val walletNotReady: String,
    private val tunnelBusy: String,
    private val generic: String,
) {
    /**
     * A pre-topic report is parked, not delivered: it reaches the support
     * team once the forum tab posts the topic and binds it, so the toast
     * sends the person back there rather than announcing a delivery.
     */
    fun attachedFor(link: ForumAttachLink): String = if (link.isPreTopic) received else attached

    fun failureFor(outcome: WarrenForumAttachOutcome): String =
        when (outcome) {
            WarrenForumAttachOutcome.NotAuthor -> notAuthor
            WarrenForumAttachOutcome.Expired -> expired
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
        received = stringResource(R.string.forum_attach_result_received),
        notAuthor = stringResource(R.string.forum_attach_result_not_author),
        expired = stringResource(R.string.forum_attach_result_expired),
        tooLarge = stringResource(R.string.report_problem_error_too_large),
        clockSkew = stringResource(R.string.report_problem_error_clock),
        server = stringResource(R.string.forum_attach_result_server),
        walletNotReady = stringResource(R.string.forum_login_result_wallet_not_ready),
        tunnelBusy = stringResource(R.string.forum_tunnel_busy),
        generic = stringResource(R.string.forum_attach_result_failure),
    )
