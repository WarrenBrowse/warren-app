package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.forum.unreadForSlot

/**
 * Turns the broadcast forum digest into a badge, a notification and an
 * indicator. The desktop `forum-activity-monitor.ts`, rule for rule.
 *
 * Everything it needs is already here: the Rust side has checked the
 * document's signature and freshness, and only this process knows which slot
 * belongs to this installation. So the whole feature costs no request, and
 * the server is never told that this account is watching.
 *
 * Two rules do most of the work.
 *
 * A notification is for activity that arrived while this run was watching.
 * The count already waiting when the app starts gets the badge but no
 * notification, otherwise every relaunch would re-announce the same
 * notifications.
 *
 * An absent digest means unknown, never zero. The fetch drops the document
 * when it cannot refresh it, and reading that gap as "all read" would fire a
 * notification for what the user has already seen as soon as it came back.
 *
 * Reading on the forum through any other channel needs no handling: it
 * advances the reader's own bookmark there, the next digest carries a lower
 * count, and the badge, the notification and the indicator follow the same
 * number.
 */
class ForumActivityMonitor(private val delegate: Delegate) {

    interface Delegate {
        /** A rise above what this run had accounted for, with the new count. */
        fun notify(unread: Int)

        /** Whether anything is waiting: drives the launcher badge and the notification's life. */
        fun showIndicator(unread: Boolean)

        /** Hands every surface the same number, so the bell cannot disagree. */
        fun publishUnread(count: Int)
    }

    private var digest: String? = null
    private var slot: Int? = null
    private var enabled = true
    private var indicator = false

    // Count this run has already accounted for, null until a digest has
    // actually been seen for the current slot: what separates "nothing new"
    // from "nothing known yet".
    private var acknowledged: Int? = null

    // What the app saw for itself, by reading the panel or by marking the
    // list seen, and the digest that was current at the time. The digest is
    // up to a server refresh plus a client poll behind, so without this the
    // badge would sit on a stale number for minutes after the user acted.
    // Held only until the digest is rebuilt: a changed document has either
    // seen our write or carries something newer, and either way it is the
    // better source. Pinning it to the document itself rather than to a clock
    // is what makes that handover exact.
    private var observed: Observed? = null

    private var lastPublished: Int? = null

    private data class Observed(val unread: Int, val digest: String?)

    fun setDigest(digest: String?) {
        this.digest = digest
        refresh()
    }

    /** What a panel read or a mark-seen just proved, effective immediately. */
    fun setObservedUnread(unread: Int) {
        observed = Observed(unread, digest)
        // The user is looking at the panel or has just acted in it. Whatever
        // the number does here, it is not news to them.
        acknowledged = unread
        refresh()
    }

    fun setSlot(slot: Int?) {
        if (slot == this.slot) return
        // Another forum account, or none: its predecessor's count says
        // nothing about this one.
        this.slot = slot
        acknowledged = null
        refresh()
    }

    fun setEnabled(enabled: Boolean) {
        if (enabled == this.enabled) return
        this.enabled = enabled
        refresh()
    }

    private fun refresh() {
        observed?.let { if (it.digest != digest) observed = null }
        val unread = observed?.unread ?: unreadForSlot(digest, slot)

        showIndicator(enabled && unread > 0)
        publish(unread)

        // Keep the watermark: a missing digest or slot is a gap in what we
        // know, not a read.
        if (digest == null || slot == null) return

        val previous = acknowledged
        // Advanced even while the setting is off, so turning it back on does
        // not announce what happened in the meantime.
        acknowledged = unread

        if (previous == null || unread <= previous || !enabled) return

        delegate.notify(unread)
    }

    private fun publish(unread: Int) {
        if (unread == lastPublished) return
        lastPublished = unread
        delegate.publishUnread(unread)
    }

    private fun showIndicator(value: Boolean) {
        if (value == indicator) return
        indicator = value
        delegate.showIndicator(value)
    }
}
