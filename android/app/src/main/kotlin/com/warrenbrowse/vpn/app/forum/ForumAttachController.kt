package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * The pending attach-logs consent request (`warren://attach-logs`, or a
 * session id typed by hand that turned out to be one), kept for the connect
 * attach session's lifetime: 1800 s (warren-connect attach.rs, the `attach`
 * entry of the fixture's `pending_ttl_secs`), with the prompt state the host
 * reads.
 */
class ForumAttachController(
    scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main),
    nowMillis: () -> Long = System::currentTimeMillis,
) : PendingForumConsent<ForumAttachLink>(PENDING_LINK_TTL_MILLIS, nowMillis, scope) {

    /** The consent prompt's state, outliving any host that shows it. */
    val prompt = ForumAttachPromptState()

    /**
     * The state outlives the consent on purpose (a rotation), so it is
     * reset here rather than left for the next request to inherit.
     */
    override fun clear() {
        super.clear()
        prompt.reset()
    }

    companion object {
        const val PENDING_LINK_TTL_MILLIS: Long = 1_800_000L
    }
}
