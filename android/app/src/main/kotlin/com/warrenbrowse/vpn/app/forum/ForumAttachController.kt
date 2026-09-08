package com.warrenbrowse.vpn.app.forum

/**
 * The pending attach-logs consent request (`warren://attach-logs`, or a
 * session id typed by hand that turned out to be one), kept for the connect
 * attach session's lifetime: 1800 s (warren-connect attach.rs, the `attach`
 * entry of the fixture's `pending_ttl_secs`).
 */
class ForumAttachController(
    nowMillis: () -> Long = System::currentTimeMillis,
) : PendingForumConsent<ForumAttachLink>(PENDING_LINK_TTL_MILLIS, nowMillis) {

    companion object {
        const val PENDING_LINK_TTL_MILLIS: Long = 1_800_000L
    }
}
