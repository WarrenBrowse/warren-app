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
 * the link's sid like the login prompt's: a link that replaces another while
 * the prompt is open gets a clean prompt, a recomposition binding the same
 * link changes nothing. Plain Kotlin over snapshot state so the transitions
 * are unit-tested off-device.
 */
class ForumAttachPromptState {
    /** The sid of the link the state belongs to; null before the first bind. */
    var sid: String? = null
        private set

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
     * The report collected for the preview, kept until the prompt goes or a
     * fresh collection replaces it, so a rotation cannot leak the file.
     */
    var preview by mutableStateOf<CollectedReport?>(null)
        private set

    /** The collected report on screen, or null when the preview is closed. */
    var previewPath by mutableStateOf<String?>(null)
        private set

    /** Adopt [link]; a different sid than the current one resets everything. */
    fun bind(link: ForumAttachLink) {
        if (link.sid == sid) return
        sid = link.sid
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

    /** The user approved: the upload is in flight. */
    fun begin() {
        busy = true
        failure = null
    }

    /** A non-attached [outcome] came back, rendered as [message]. */
    fun settle(outcome: WarrenForumAttachOutcome, message: String) {
        busy = false
        terminal = isTerminalAttachOutcome(outcome)
        cancelsOnDecline = outcome !is WarrenForumAttachOutcome.Expired
        failure = message
    }

    /** The provider attached (or parked) the report. */
    fun markAttached() {
        busy = false
        attached = true
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

    /** Closes the screen; the file stays for the approval or the next bind. */
    fun closePreview() {
        previewPath = null
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
