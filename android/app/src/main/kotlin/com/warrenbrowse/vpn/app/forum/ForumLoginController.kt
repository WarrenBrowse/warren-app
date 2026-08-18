package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Holds the pending forum-login consent request so the deep-link handler
 * (`MainActivity`) and the Compose consent prompt (`ForumLoginPromptHost`) are
 * decoupled: a `warren://forum-login` link that cold-starts the app arrives
 * before the prompt exists, so the handler stashes it here and the prompt reads
 * it when it composes. Only the latest unanswered request is kept.
 */
class ForumLoginController(
    // Injected for the JVM tests; production uses the wall clock.
    private val nowMillis: () -> Long = System::currentTimeMillis,
) {
    private val _pending = MutableStateFlow<ForumLoginLink?>(null)
    private var requestedAtMillis: Long = 0L

    /** The pending consent request, or null when there is none to show. */
    val pending: StateFlow<ForumLoginLink?> = _pending.asStateFlow()

    fun request(link: ForumLoginLink) {
        requestedAtMillis = nowMillis()
        _pending.value = link
    }

    /**
     * True when the pending link outlived the connect login session (300 s,
     * warren-connect sessions.rs), so approving it can only produce a
     * dead-session failure. Mirrors the desktop's PENDING_LOGIN_MAX_AGE_MS.
     */
    fun isStale(): Boolean {
        if (_pending.value == null) return false
        return nowMillis() - requestedAtMillis > PENDING_LINK_TTL_MILLIS
    }

    fun clear() {
        _pending.value = null
    }

    companion object {
        const val PENDING_LINK_TTL_MILLIS: Long = 300_000L
    }
}
