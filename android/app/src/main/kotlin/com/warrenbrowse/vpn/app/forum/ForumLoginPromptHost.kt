package com.warrenbrowse.vpn.app.forum

import android.widget.Toast
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * Consent prompt for the community-forum wallet login (doc 55). The app NEVER
 * signs into the forum silently: signing happens only after the user approves
 * here. Observes [ForumLoginController.pending]; when a `warren://forum-login`
 * link has been captured it shows the prompt, and on approval runs
 * [WarrenForumLoginUseCase] (which signs + POSTs in Rust) and reports the
 * outcome as a toast. Declining just dismisses the prompt (the server session
 * expires on its own in 10 minutes).
 */
@Composable
fun ForumLoginPromptHost() {
    val controller = koinInject<ForumLoginController>()
    val useCase = koinInject<WarrenForumLoginUseCase>()
    val pending by controller.pending.collectAsState()
    val link = pending ?: return

    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = { if (!busy) controller.clear() },
        title = { Text("Sign in to the Warren community forum?") },
        text = {
            Text(
                "Your app will sign a one time challenge with your wallet key to prove it is you. " +
                    "No email and no password are used, and you appear under an anonymous handle " +
                    "that cannot be linked to your Warren account. Only approve if you started " +
                    "this sign in.",
            )
        },
        confirmButton = {
            PrimaryButton(
                text = "Approve",
                onClick = {
                    if (!busy) {
                        // Keep the request pending (dialog stays) until the call
                        // returns, so this composable does not leave composition
                        // and cancel the coroutine mid-flight.
                        busy = true
                        scope.launch {
                            val outcome = useCase.signIn(link)
                            controller.clear()
                            Toast.makeText(context, messageFor(outcome), Toast.LENGTH_LONG).show()
                        }
                    }
                },
            )
        },
        dismissButton = {
            TextButton(onClick = { if (!busy) controller.clear() }) { Text("Cancel") }
        },
    )
}

private fun messageFor(outcome: WarrenForumLoginOutcome): String =
    when (outcome) {
        is WarrenForumLoginOutcome.Approved -> "Signed in to the Warren forum."
        is WarrenForumLoginOutcome.SubscriptionRequired ->
            "Forum access requires a Warren subscription. This wallet has never subscribed."
        is WarrenForumLoginOutcome.WalletNotReady -> "Set up your Warren wallet first."
        is WarrenForumLoginOutcome.Failure -> "Sign in failed. Please try again in a moment."
    }
