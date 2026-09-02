package com.warrenbrowse.vpn.app.forum

import android.app.Activity
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorSmall
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
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
 */
@Composable
fun ForumLoginPromptHost() {
    val controller = koinInject<ForumLoginController>()
    val useCase = koinInject<WarrenForumLoginUseCase>()
    val pending by controller.pending.collectAsState()
    val link = pending ?: return

    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // Keyed on the link's sid inside: a link replacing another while the
    // prompt is open starts clean instead of inheriting a disarmed Approve.
    val state = remember { ForumLoginPromptState() }
    state.bind(link)

    val approvedMessage = stringResource(R.string.forum_login_result_approved)
    val subscriptionRequiredMessage =
        stringResource(R.string.forum_login_result_subscription_required)
    val walletNotReadyMessage = stringResource(R.string.forum_login_result_wallet_not_ready)
    val clockSkewMessage = stringResource(R.string.forum_login_result_clock_skew)
    val expiredMessage = stringResource(R.string.forum_login_result_expired)
    val tunnelBusyMessage = stringResource(R.string.forum_tunnel_busy)
    val failureMessage = stringResource(R.string.forum_login_result_failure)

    // Declining notifies the provider so the waiting browser page unblocks
    // (mirrors the desktop), then dismisses the prompt. After a terminal
    // refusal the provider already knows; only the prompt is left to close.
    val onDecline = {
        if (!state.busy) {
            if (!state.terminal) useCase.cancel(link)
            controller.clear()
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
        text = {
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
                        modifier =
                            Modifier.semantics { liveRegion = LiveRegionMode.Assertive },
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
        },
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
                onClick = {
                    if (!state.busy && controller.isStale()) {
                        // The server session died while the prompt sat here;
                        // signing now can only fail on a dead sid.
                        state.fail(expiredMessage)
                    } else if (!state.busy) {
                        // Keep the request pending (dialog stays) until the call
                        // returns, so this composable does not leave composition
                        // and cancel the coroutine mid-flight.
                        state.begin()
                        scope.launch {
                            when (val outcome = useCase.signIn(link)) {
                                is WarrenForumLoginOutcome.Approved -> {
                                    controller.clear()
                                    Toast.makeText(
                                            context,
                                            approvedMessage,
                                            Toast.LENGTH_LONG,
                                        )
                                        .show()
                                    // The browser page is what completes the
                                    // login, and it only re-polls once it is
                                    // visible again: hand the foreground back
                                    // to it, as the desktop hides its window.
                                    (context as? Activity)?.moveTaskToBack(true)
                                }
                                else ->
                                    state.settle(
                                        outcome,
                                        failureMessageFor(
                                            outcome = outcome,
                                            subscriptionRequired = subscriptionRequiredMessage,
                                            walletNotReady = walletNotReadyMessage,
                                            clockSkew = clockSkewMessage,
                                            expired = expiredMessage,
                                            tunnelBusy = tunnelBusyMessage,
                                            generic = failureMessage,
                                        ),
                                    )
                            }
                        }
                    }
                },
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
