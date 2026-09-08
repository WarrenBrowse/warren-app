package com.warrenbrowse.vpn.app.forum

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.warrenbrowse.vpn.lib.repository.CollectedReport

/**
 * The attach consent's state for one pending link. Owned by
 * [ForumAttachController], never by the composable: a rotation recreates the
 * Activity and every `remember` with it, while the upload it launched is
 * still out, so the state it reports to has to outlive the host. Keyed on
 * the link's sid and the request's token: a link that replaces another while
 * the prompt is open gets a clean prompt, a recomposition binding the same
 * request changes nothing, and a new request naming the sid of a finished
 * attempt (the broker hands the same sid back for a pending topic, and a
 * pre-topic session stays alive once its report is parked) starts clean
 * too. Plain Kotlin over snapshot state so the transitions are unit-tested
 * off-device.
 */
class ForumAttachPromptState {
    /** The sid of the link the state belongs to; null before the first bind. */
    var sid: String? = null
        private set

    private var boundToken: Long? = null

    /**
     * The attempt in flight, as [begin] numbered it. A result names the
     * attempt it belongs to and is dropped when a rebind superseded it.
     */
    private var attempt: Long = 0L

    /** An upload is out: Approve and Cancel are disabled. */
    var busy by mutableStateOf(false)
        private set

    /** The inline message of the last non-attached outcome, if any. */
    var failure by mutableStateOf<String?>(null)
        private set

    /**
     * No retry can change the outcome (not the author, a dead session, a
     * report over the cap), so Approve is disarmed.
     */
    var terminal by mutableStateOf(false)
        private set

    /**
     * Leaving the prompt tells the provider the user declined, so the forum
     * page stops waiting. False only once the provider reported the session
     * gone: a refusal as author or a report over the cap leaves the session
     * pending on the provider, and the page polling it.
     */
    var cancelsOnDecline by mutableStateOf(true)
        private set

    /** The upload was attached (or parked); the host closes the prompt. */
    var attached by mutableStateOf(false)
        private set

    /** "View the logs" is collecting the report. */
    var collecting by mutableStateOf(false)
        private set

    var collectFailed by mutableStateOf(false)
        private set

    /**
     * The report collected for the preview, kept until [takePreview] hands
     * it out for deletion, so a rotation cannot leak the file.
     */
    var preview by mutableStateOf<CollectedReport?>(null)
        private set

    /** The collected report on screen, or null when the preview is closed. */
    var previewPath by mutableStateOf<String?>(null)
        private set

    /** Adopt [link] for the request [token]; anything else than the current pair resets. */
    fun bind(link: ForumAttachLink, token: Long) {
        if (link.sid == sid && token == boundToken) return
        reset()
        sid = link.sid
        boundToken = token
    }

    /**
     * Back to the state before any bind: what the controller does when the
     * consent ends. The preview file, if any, is the caller's to delete
     * through [takePreview] first.
     */
    fun reset() {
        sid = null
        boundToken = null
        attempt += 1
        busy = false
        failure = null
        terminal = false
        cancelsOnDecline = true
        attached = false
        collecting = false
        collectFailed = false
        preview = null
        previewPath = null
    }

    val canApprove: Boolean
        get() = !busy && !terminal

    /** The user approved: the upload is in flight. Returns the attempt's number. */
    fun begin(): Long {
        attempt += 1
        busy = true
        failure = null
        return attempt
    }

    /**
     * A non-attached [outcome] of [attempt] came back, rendered as [message].
     * False when a rebind superseded the attempt: nothing is applied.
     */
    fun settle(attempt: Long, outcome: WarrenForumAttachOutcome, message: String): Boolean {
        if (attempt != this.attempt) return false
        busy = false
        terminal = isTerminalAttachOutcome(outcome)
        cancelsOnDecline = outcome !is WarrenForumAttachOutcome.Expired
        failure = message
        return true
    }

    /** The provider attached (or parked) the report of [attempt]; false when superseded. */
    fun markAttached(attempt: Long): Boolean {
        if (attempt != this.attempt) return false
        busy = false
        attached = true
        return true
    }

    /** A message for the current link without an attempt (a stale link). */
    fun fail(message: String) {
        failure = message
    }

    fun beginCollect() {
        collecting = true
        collectFailed = false
    }

    fun previewReady(report: CollectedReport) {
        collecting = false
        preview = report
        previewPath = report.file.absolutePath
    }

    fun previewFailed() {
        collecting = false
        collectFailed = true
    }

    /** Closes the screen; the file stays for the next view or the drop. */
    fun closePreview() {
        previewPath = null
    }

    /**
     * Hands the collected report out, once, for deletion, and forgets it:
     * the screen over a deleted file closes with it, and the next "View the
     * logs" collects afresh.
     */
    fun takePreview(): CollectedReport? {
        val report = preview
        preview = null
        previewPath = null
        return report
    }
}

/**
 * True when no retry from this device can change the outcome: the provider
 * refused the signer as author, the session is gone, or the report is over
 * the cap. A clock fix, a settled tunnel or a recovered provider are retries
 * worth offering.
 */
internal fun isTerminalAttachOutcome(outcome: WarrenForumAttachOutcome): Boolean =
    outcome is WarrenForumAttachOutcome.NotAuthor ||
        outcome is WarrenForumAttachOutcome.Expired ||
        outcome is WarrenForumAttachOutcome.TooLarge
