package com.warrenbrowse.vpn.app.forum

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * The attach consent's state for one pending link. Keyed on the link's sid
 * like the login prompt's: a link that replaces another while the prompt is
 * open gets a clean prompt, a recomposition binding the same link changes
 * nothing. Plain Kotlin over snapshot state so the transitions are
 * unit-tested off-device.
 */
class ForumAttachPromptState {
    /** The sid of the link the state belongs to; null before the first bind. */
    var sid: String? = null
        private set

    private var linkTopicId: Long? = null

    /**
     * The link carries no topic (a session id typed by hand), so the prompt
     * shows the topic field; an empty field means a report still being
     * composed.
     */
    var needsTopic by mutableStateOf(false)
        private set

    /** What the topic field holds: digits only, whatever was typed or pasted. */
    var topicInput by mutableStateOf("")
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

    /** "View the logs" is collecting the report. */
    var collecting by mutableStateOf(false)
        private set

    var collectFailed by mutableStateOf(false)
        private set

    /** The collected report on screen, or null when the preview is closed. */
    var previewPath by mutableStateOf<String?>(null)
        private set

    /** Adopt [link]; a different sid than the current one resets everything. */
    fun bind(link: ForumAttachLink) {
        if (link.sid == sid) return
        sid = link.sid
        linkTopicId = link.topicId
        needsTopic = link.topicId == null
        topicInput = ""
        busy = false
        failure = null
        terminal = false
        collecting = false
        collectFailed = false
        previewPath = null
    }

    fun updateTopicInput(text: String) {
        topicInput = text.filter { it.isDigit() }
    }

    /**
     * The topic the approval sends: the link's own, else the field's, an
     * empty field standing for a report still being composed. Null when the
     * field holds a number no topic can have.
     */
    fun topicIdOrNull(): Long? =
        linkTopicId
            ?: if (topicInput.isEmpty()) ForumAttachLink.PRE_TOPIC else parseForumTopicId(topicInput)

    val canApprove: Boolean
        get() = !busy && !terminal && topicIdOrNull() != null

    /** The user approved: the upload is in flight. */
    fun begin() {
        busy = true
        failure = null
    }

    /** A non-attached [outcome] came back, rendered as [message]. */
    fun settle(outcome: WarrenForumAttachOutcome, message: String) {
        busy = false
        terminal = isTerminalAttachOutcome(outcome)
        failure = message
    }

    /** A message for the current link without an attempt (a stale link). */
    fun fail(message: String) {
        failure = message
    }

    fun beginCollect() {
        collecting = true
        collectFailed = false
    }

    fun previewReady(path: String) {
        collecting = false
        previewPath = path
    }

    fun previewFailed() {
        collecting = false
        collectFailed = true
    }

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
