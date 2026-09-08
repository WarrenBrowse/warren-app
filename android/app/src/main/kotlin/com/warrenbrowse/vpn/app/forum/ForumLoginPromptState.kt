package com.warrenbrowse.vpn.app.forum

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * The consent prompt's state for one pending link. Owned by
 * [ForumLoginController], never by the composable: a rotation recreates the
 * Activity and every `remember` with it, while the signature it launched is
 * still out, so the state it reports to has to outlive the host. Keyed on
 * the link's sid and the request's token: a link that replaces another while
 * the prompt is open (the user started again from the browser after a
 * terminal refusal) gets a clean prompt, a recomposition binding the same
 * request changes nothing, and a new request naming the sid of a finished
 * attempt starts clean too. Plain Kotlin over snapshot state so the
 * transitions are unit-tested off-device.
 */
class ForumLoginPromptState {
    /** The sid of the link the state belongs to; null before the first bind. */
    var sid: String? = null
        private set

    private var boundToken: Long? = null

    /**
     * The attempt in flight, as [begin] numbered it. A result names the
     * attempt it belongs to and is dropped when a rebind superseded it.
     */
    private var attempt: Long = 0L

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

    /**
     * The provider approved: the host closes the prompt and hands the
     * foreground back. Held here, owned by the controller, so a host
     * recreated by a rotation mid-flight learns it too.
     */
    var approved by mutableStateOf(false)
        private set

    /** Adopt [link] for the request [token]; anything else than the current pair resets. */
    fun bind(link: ForumLoginLink, token: Long) {
        if (link.sid == sid && token == boundToken) return
        reset()
        sid = link.sid
        boundToken = token
    }

    /** Back to the state before any bind: what the controller does when the consent ends. */
    fun reset() {
        sid = null
        boundToken = null
        attempt += 1
        busy = false
        failure = null
        terminal = false
        approved = false
    }

    /** The user approved: the signature is in flight. Returns the attempt's number. */
    fun begin(): Long {
        attempt += 1
        busy = true
        failure = null
        return attempt
    }

    /**
     * A non-approved [outcome] of [attempt] came back, rendered as [message].
     * False when a rebind superseded the attempt: nothing is applied.
     */
    fun settle(attempt: Long, outcome: WarrenForumLoginOutcome, message: String): Boolean {
        if (attempt != this.attempt) return false
        busy = false
        terminal = isTerminalOutcome(outcome)
        failure = message
        return true
    }

    /** The provider accepted the signature of [attempt]; false when superseded. */
    fun markApproved(attempt: Long): Boolean {
        if (attempt != this.attempt) return false
        busy = false
        approved = true
        return true
    }

    /** A message for the current link without an attempt (a stale link). */
    fun fail(message: String) {
        failure = message
    }
}
