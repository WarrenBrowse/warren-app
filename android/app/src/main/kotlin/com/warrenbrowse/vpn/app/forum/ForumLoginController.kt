package com.warrenbrowse.vpn.app.forum

/**
 * The pending forum-login consent request (`warren://forum-login`), kept for
 * the connect login session's lifetime: 300 s (warren-connect sessions.rs,
 * the `login` entry of the fixture's `pending_ttl_secs`).
 */
class ForumLoginController(
    nowMillis: () -> Long = System::currentTimeMillis,
) : PendingForumConsent<ForumLoginLink>(PENDING_LINK_TTL_MILLIS, nowMillis) {

    companion object {
        const val PENDING_LINK_TTL_MILLIS: Long = 300_000L
    }
}
