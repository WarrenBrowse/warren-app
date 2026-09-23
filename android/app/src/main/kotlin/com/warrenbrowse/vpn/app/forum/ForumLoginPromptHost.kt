package com.warrenbrowse.vpn.app.forum

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.SecureFlagPolicy
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorSmall
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * Consent prompt for the community-forum wallet login (doc 55). The app NEVER
 * signs into the forum silently: signing happens only after the user approves
 * here. Observes [ForumLoginController.pending]; when a `warren://forum-login`
 * link has been captured it shows the prompt, and on approval runs
 * [WarrenForumLoginUseCase] (which signs + POSTs in Rust). Declining just
 * dismisses the prompt (the server session expires on its own in 5 minutes).
 *
 * A failure keeps the prompt open with the reason inline. Clearing it instead
 * discarded the captured link, so recovering from a transient failure meant
 * restarting the whole browser round trip.
 *
 * The prompt state and the signature live on the controller, not here: a
 * rotation recreates this composable while the signature is out, and a host
 * that owned them would re-arm Approve over it and drop the outcome.
 */
@Composable
fun ForumLoginPromptHost() {
    val controller = koinInject<ForumLoginController>()
    val useCase = koinInject<WarrenForumLoginUseCase>()
    val journal = koinInject<ForumJournal>()
    val pending by controller.pending.collectAsState()
    val link = pending ?: return

    val context = LocalContext.current
    // Keyed on the link's sid and the request's token inside: a link
    // replacing another while the prompt is open, or a new request naming
    // the sid of a finished attempt, starts clean instead of inheriting a
    // disarmed Approve or a spent approved flag.
    val state = controller.prompt
    state.bind(link, controller.requestToken)
    val messages = promptMessages()

    // The browser page is what completes the login, and it only re-polls
    // once it is visible again: hand the foreground back to it, as the
    // desktop hides its window. Reacted to from the state so a host recreated
    // mid-flight does it too.
    LaunchedEffect(state.approved) {
        if (state.approved) {
            controller.clear()
            Toast.makeText(context, messages.approved, Toast.LENGTH_LONG).show()
            (context as? Activity)?.moveTaskToBack(true)
        }
    }

    state.completion?.let { return BoundCompletion(it, state, controller) }

    // Declining notifies the provider so the waiting browser page unblocks
    // (mirrors the desktop), then dismisses the prompt. After a terminal
    // refusal the provider already knows; only the prompt is left to close.
    val onDecline = {
        if (!state.busy) {
            if (!state.terminal) useCase.cancel(link)
            controller.clear()
        }
    }

    val onApprove = {
        if (!state.busy && controller.isStale()) {
            // The server session died while the prompt sat here; signing now
            // can only fail on a dead sid.
            state.fail(messages.expired)
        } else if (!state.busy) {
            val attempt = state.begin()
            controller.scope.launch {
                val outcome = useCase.signIn(link)
                // A second link mid-signature rebinds the prompt: this result
                // belongs to the link that launched it and is dropped rather
                // than applied to the one now on screen.
                val applied =
                    if (outcome is WarrenForumLoginOutcome.Approved) {
                        state.complete(attempt, link, outcome.completion, System.currentTimeMillis())
                    } else {
                        state.settle(attempt, outcome, messages.failureFor(outcome))
                    }
                if (!applied) journal.record(ForumEvent.LOGIN_RESULT, JournalField.Class("superseded"))
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDecline,
        title = {
            Text(
                stringResource(
                    if (link.crossDevice) R.string.forum_login_title_cross_device
                    else R.string.forum_login_title
                )
            )
        },
        text = { PromptBody(link, state) },
        confirmButton = {
            PrimaryButton(
                text = stringResource(R.string.forum_login_approve),
                isEnabled = !state.busy && !state.terminal,
                // Beside the label rather than in place of it: the action stays
                // named while the signature is in flight.
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
}

/**
 * A bound approval: after a same-device link the handoff page opens once in
 * the default browser (a failed open falls back to the code on screen), and
 * the screen closes with the session behind the code.
 */
@Composable
private fun BoundCompletion(
    completion: ForumCompletionView,
    state: ForumLoginPromptState,
    controller: ForumLoginController,
) {
    val context = LocalContext.current
    LaunchedEffect(state.hasHandoffToOpen) {
        state.takeHandoffToOpen(System.currentTimeMillis())?.let { url ->
            if (!openInBrowser(context, url)) state.revealCode()
        }
    }
    // The wall clock is read every second rather than waited on once: a delay
    // does not count while the device sleeps, and one that slept past the
    // session would keep a dead code on screen.
    LaunchedEffect(completion) {
        while (!state.codeExpired(System.currentTimeMillis())) delay(EXPIRY_CHECK_MILLIS)
        controller.clear()
    }
    CompletionDialog(
        view = completion,
        codeRevealed = state.codeRevealed,
        onReveal = state::revealCode,
        onFinishInBrowser = {
            state.takeFinishUrl(System.currentTimeMillis())?.let { openInBrowser(context, it) }
        },
        onClose = controller::clear,
    )
}

private const val EXPIRY_CHECK_MILLIS = 1_000L

/**
 * The screen of a bound approval (warren-connect `docs/FORUM-LOGIN-V2.md`):
 * the browser that opened the sign-in must present the one-time code. After a
 * same-device link the handoff page is already open in the browser and the
 * code waits behind "Show the code"; after a QR or a typed code the code is
 * the screen, and after a same-device link answered as a QR's it comes under
 * the warning that the link was relayed. The code is never copied for the
 * person: one on the clipboard is one paste away from a chat window.
 */
@Composable
private fun CompletionDialog(
    view: ForumCompletionView,
    codeRevealed: Boolean,
    onReveal: () -> Unit,
    onFinishInBrowser: () -> Unit,
    onClose: () -> Unit,
) {
    val finishing = view.screen == ForumCompletionScreen.FINISHING_IN_BROWSER
    AlertDialog(
        onDismissRequest = onClose,
        // No screenshot, no screen recording and no app-switcher snapshot of
        // the code: the person leaves the app to type it in the browser, which
        // is exactly when the system writes that snapshot.
        properties = DialogProperties(securePolicy = SecureFlagPolicy.SecureOn),
        title = {
            Text(
                stringResource(
                    if (finishing) R.string.forum_login_handoff_title else R.string.forum_login_code_title
                )
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                if (finishing) Text(stringResource(R.string.forum_login_handoff_body))
                if (view.screen == ForumCompletionScreen.SHOW_CODE_RELAYED) {
                    Text(
                        text = stringResource(R.string.forum_login_code_relayed_warning),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                if (codeRevealed) {
                    Text(
                        text = view.code,
                        style = MaterialTheme.typography.displaySmall,
                        fontFamily = FontFamily.Monospace,
                        fontWeight = FontWeight.SemiBold,
                        letterSpacing = 6.sp,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Text(
                        text = stringResource(R.string.forum_login_code_warning),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else {
                    TextButton(onClick = onReveal, colors = readableTextButton()) {
                        Text(stringResource(R.string.forum_login_show_code))
                    }
                }
            }
        },
        confirmButton = {
            if (view.finishInBrowser) {
                PrimaryButton(
                    text = stringResource(R.string.forum_login_finish_here),
                    onClick = onFinishInBrowser,
                )
            } else {
                PrimaryButton(text = stringResource(R.string.close), onClick = onClose)
            }
        },
        dismissButton =
            if (view.finishInBrowser) {
                {
                    TextButton(onClick = onClose, colors = readableTextButton()) {
                        Text(stringResource(R.string.close))
                    }
                }
            } else null,
    )
}

// The theme's primary is a charcoal one shade off the dialog surface, which
// leaves a default text button invisible.
@Composable
private fun readableTextButton() =
    ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)

/**
 * Opens [url] in the default browser. False when no app can: the caller falls
 * back to the code. The URL carries the code and the sid, so only the class
 * of a failure is logged.
 */
private fun openInBrowser(context: Context, url: String): Boolean =
    try {
        context.startActivity(
            Intent(Intent.ACTION_VIEW, Uri.parse(url))
                .addCategory(Intent.CATEGORY_BROWSABLE)
                .apply { if (context !is Activity) addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
        )
        true
    } catch (e: ActivityNotFoundException) {
        Logger.w("ForumLoginPromptHost: no browser for the sign-in handoff")
        false
    }

/** The prompt's two sentences, the inline failure and the in-flight line. */
@Composable
private fun PromptBody(link: ForumLoginLink, state: ForumLoginPromptState) {
    Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
        Text(
            stringResource(
                if (link.crossDevice) R.string.forum_login_body_first_cross_device
                else R.string.forum_login_body_first
            )
        )
        Text(
            stringResource(
                if (link.crossDevice) R.string.forum_login_body_second_cross_device
                else R.string.forum_login_body_second
            )
        )
        state.failure?.let { reason ->
            Text(
                text = reason,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive },
            )
        }
        if (state.busy) {
            Text(
                text = stringResource(R.string.forum_login_signing),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
    }
}

/**
 * The prompt's copy, resolved in composition so the click handlers, which run
 * outside it, hold plain strings.
 */
private class PromptMessages(
    val approved: String,
    private val subscriptionRequired: String,
    private val walletNotReady: String,
    private val clockSkew: String,
    val expired: String,
    private val tunnelBusy: String,
    private val generic: String,
) {
    fun failureFor(outcome: WarrenForumLoginOutcome): String =
        failureMessageFor(
            outcome = outcome,
            subscriptionRequired = subscriptionRequired,
            walletNotReady = walletNotReady,
            clockSkew = clockSkew,
            expired = expired,
            tunnelBusy = tunnelBusy,
            generic = generic,
        )
}

@Composable
private fun promptMessages(): PromptMessages =
    PromptMessages(
        approved = stringResource(R.string.forum_login_result_approved),
        subscriptionRequired = stringResource(R.string.forum_login_result_subscription_required),
        walletNotReady = stringResource(R.string.forum_login_result_wallet_not_ready),
        clockSkew = stringResource(R.string.forum_login_result_clock_skew),
        expired = stringResource(R.string.forum_login_result_expired),
        tunnelBusy = stringResource(R.string.forum_tunnel_busy),
        generic = stringResource(R.string.forum_login_result_failure),
    )

/**
 * True when the provider has cancelled the session behind this outcome, so the
 * same sid cannot be approved any more whatever the user changes on the device.
 */
internal fun isTerminalOutcome(outcome: WarrenForumLoginOutcome): Boolean =
    outcome is WarrenForumLoginOutcome.ClockSkew ||
        outcome is WarrenForumLoginOutcome.SubscriptionRequired ||
        outcome is WarrenForumLoginOutcome.Expired

/** The inline error for a non-approved outcome; pure so it stays unit-mappable. */
internal fun failureMessageFor(
    outcome: WarrenForumLoginOutcome,
    subscriptionRequired: String,
    walletNotReady: String,
    clockSkew: String,
    expired: String,
    tunnelBusy: String,
    generic: String,
): String =
    when (outcome) {
        is WarrenForumLoginOutcome.SubscriptionRequired -> subscriptionRequired
        is WarrenForumLoginOutcome.WalletNotReady -> walletNotReady
        is WarrenForumLoginOutcome.ClockSkew -> clockSkew
        is WarrenForumLoginOutcome.Expired -> expired
        is WarrenForumLoginOutcome.Deferred -> tunnelBusy
        else -> generic
    }
