package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * The pending forum-login consent request (`warren://forum-login`), kept for
 * the connect login session's lifetime: 300 s (warren-connect sessions.rs,
 * the `login` entry of the fixture's `pending_ttl_secs`), with the prompt
 * state the host reads.
 */
class ForumLoginController(
    scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main),
    nowMillis: () -> Long = System::currentTimeMillis,
) : PendingForumConsent<ForumLoginLink>(PENDING_LINK_TTL_MILLIS, nowMillis, scope) {

    /** The consent prompt's state, outliving any host that shows it. */
    val prompt = ForumLoginPromptState()

    /**
     * The state outlives the consent on purpose (a rotation), so it is
     * reset here rather than left for the next request to inherit.
     */
    override fun clear() {
        super.clear()
        prompt.reset()
    }

    companion object {
        const val PENDING_LINK_TTL_MILLIS: Long = 300_000L
    }
}
