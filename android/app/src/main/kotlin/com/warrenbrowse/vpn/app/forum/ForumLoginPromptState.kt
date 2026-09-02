package com.warrenbrowse.vpn.app.forum

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * The consent prompt's state for one pending link. Keyed on the link's sid:
 * a link that replaces another while the prompt is open (the user started
 * again from the browser after a terminal refusal) gets a clean prompt, and
 * a recomposition binding the same link changes nothing. Plain Kotlin over
 * snapshot state so the transitions are unit-tested off-device.
 */
class ForumLoginPromptState {
    /** The sid of the link the state belongs to; null before the first bind. */
    var sid: String? = null
        private set

    /** A signature is out: Approve and Cancel are disabled. */
    var busy by mutableStateOf(false)
        private set

    /** The inline message of the last non-approved outcome, if any. */
    var failure by mutableStateOf<String?>(null)
        private set

    /**
     * The provider has closed the door on this sid (it cancels the session on a
     * clock-skew or subscription refusal), so Approve is disarmed: a retry can
     * only answer "unknown session" and land on the generic message.
     */
    var terminal by mutableStateOf(false)
        private set

    /** Adopt [link]; a different sid than the current one resets everything. */
    fun bind(link: ForumLoginLink) {
        if (link.sid == sid) return
        sid = link.sid
        busy = false
        failure = null
        terminal = false
    }

    /** The user approved: the signature is in flight. */
    fun begin() {
        busy = true
        failure = null
    }

    /** A non-approved [outcome] came back, rendered as [message]. */
    fun settle(outcome: WarrenForumLoginOutcome, message: String) {
        busy = false
        terminal = isTerminalOutcome(outcome)
        failure = message
    }

    /** A message for the current link without an attempt (a stale link). */
    fun fail(message: String) {
        failure = message
    }
}
