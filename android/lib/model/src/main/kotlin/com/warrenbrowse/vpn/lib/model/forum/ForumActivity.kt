package com.warrenbrowse.vpn.lib.model.forum

/**
 * How the broadcast activity digest becomes one user's badge, and what the
 * header's forum slot carries. Mirrors the desktop `forum-identity.ts`, the
 * one place the three surfaces (bell, notification, launcher badge) take
 * their rules from, so none of them can be left behind the others.
 */

/** Highest count one digest character can carry, above which it saturates. */
const val UNREAD_SATURATED = 15

/**
 * Unread count this installation's [slot] carries in [digest] (one lowercase
 * hex character per slot).
 *
 * Zero whenever there is nothing to show: no fresh document, no slot yet, or a
 * slot past what the server has published (the normal state of an account
 * that registered since the last rebuild). The Rust side has already checked
 * the signature and the freshness, so this only indexes.
 */
fun unreadForSlot(digest: String?, slot: Int?): Int =
    digest?.getOrNull(slot ?: -1)?.digitToIntOrNull(radix = 16) ?: 0

/** The count as shown, saturating rather than growing the badge. */
fun unreadLabel(unread: Int): String =
    if (unread >= UNREAD_SATURATED) "$UNREAD_SATURATED+" else unread.toString()

/**
 * Whether the app shows forum ACTIVITY at all: the header bell, the local
 * notification and the launcher badge alike. Off means off everywhere, and a
 * wallet with no forum account has no activity to report.
 */
fun showsForumActivity(hasAccount: Boolean, enabled: Boolean): Boolean = hasAccount && enabled

/** What the header's forum slot carries: the bell, the lifebuoy, or nothing. */
enum class ForumHeaderButton {
    ACTIVITY,
    COMMUNITY,
    NONE,
}

/**
 * Which button the header's forum slot shows.
 *
 * A wallet with no forum account gets a lifebuoy straight to the forum rather
 * than an empty slot: the bell would be inert for them, but the forum is the
 * one thing they might actually want, and it is where an account comes from.
 * The setting still governs the whole slot, lifebuoy included, so a user who
 * turned the forum off gets no forum in their header either.
 */
fun forumHeaderButton(hasAccount: Boolean, enabled: Boolean): ForumHeaderButton =
    when {
        !enabled -> ForumHeaderButton.NONE
        hasAccount -> ForumHeaderButton.ACTIVITY
        else -> ForumHeaderButton.COMMUNITY
    }

/**
 * What the notification about a rise says. One digest character per slot, so
 * the count stops climbing at its ceiling; saying "15" there would be a number
 * the user can check and find wrong, hence [MoreThan] with the highest count
 * the app can measure exactly.
 */
sealed interface ForumActivityWording {
    data object Single : ForumActivityWording

    data class Several(val count: Int) : ForumActivityWording

    data class MoreThan(val count: Int) : ForumActivityWording
}

/**
 * Whether a notification's age still reads better as "2 h ago" than as a
 * date: past a week, "5 weeks ago" tells a reader less than the day it
 * happened (the desktop `relativeTime` threshold).
 */
fun forumNotificationAgeIsRelative(createdAtSecs: Long, nowSecs: Long): Boolean =
    nowSecs - createdAtSecs < RELATIVE_AGE_LIMIT_SECS

private const val RELATIVE_AGE_LIMIT_SECS = 7L * 24 * 3600

fun forumActivityWording(unread: Int): ForumActivityWording =
    when {
        unread >= UNREAD_SATURATED -> ForumActivityWording.MoreThan(UNREAD_SATURATED - 1)
        unread == 1 -> ForumActivityWording.Single
        else -> ForumActivityWording.Several(unread)
    }
