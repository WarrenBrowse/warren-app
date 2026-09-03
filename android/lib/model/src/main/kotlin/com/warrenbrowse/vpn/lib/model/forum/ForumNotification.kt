package com.warrenbrowse.vpn.lib.model.forum

/**
 * What happened, as far as the panel needs to distinguish it. The tokens are
 * the shared crate's (`warren_forum::ForumNotificationKind`), which validated
 * the row before it crossed the FFI.
 */
enum class ForumNotificationKind(val token: String) {
    MENTIONED("mentioned"),
    REPLIED("replied"),
    QUOTED("quoted"),
    LIKED("liked"),
    PRIVATE_MESSAGE("private_message"),
    POSTED("posted"),
    LINKED("linked"),
    GRANTED_BADGE("granted_badge"),
    WATCHING_FIRST_POST("watching_first_post"),
    ANNOUNCEMENT("announcement"),
    /** A kind this version has no wording of its own for; still rendered. */
    OTHER("other");

    companion object {
        fun fromToken(token: String?): ForumNotificationKind =
            entries.firstOrNull { it.token == token } ?: OTHER
    }
}

/**
 * One row of the forum activity panel, as the shared crate handed it over:
 * every field already validated, the path pinned to the shapes the forum
 * produces (absent when the notification points at nothing openable).
 */
data class ForumNotification(
    val id: Long,
    val kind: ForumNotificationKind,
    /** Unread by Discourse's own rule, so an unread row is one the badge counted. */
    val unread: Boolean,
    /** Unix epoch seconds. */
    val createdAt: Long,
    val title: String?,
    val actor: String?,
    val excerpt: String?,
    /** Forum-relative, e.g. `/t/86/4`; opened in the browser. */
    val path: String?,
)
