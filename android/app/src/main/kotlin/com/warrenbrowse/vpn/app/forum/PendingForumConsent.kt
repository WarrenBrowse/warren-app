package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Holds one pending consent request so the deep-link handler (`MainActivity`)
 * and the Compose prompt that answers it are decoupled: a link that
 * cold-starts the app arrives before the prompt exists, so the handler
 * stashes it here and the prompt reads it when it composes. Only the latest
 * unanswered request is kept. The login and the attach-logs flows each keep
 * one, with their own broker session lifetime as [ttlMillis].
 *
 * [scope] is where a prompt runs the signature or the upload it launches: a
 * composition scope dies with the Activity on a rotation, taking the outcome
 * with it and leaving Approve re-armed over a request still in flight.
 */
open class PendingForumConsent<T : Any>(
    private val ttlMillis: Long,
    // Injected for the JVM tests; production uses the wall clock.
    private val nowMillis: () -> Long = System::currentTimeMillis,
    val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main),
) {
    private val _pending = MutableStateFlow<T?>(null)
    private var requestedAtMillis: Long = 0L

    /** The pending consent request, or null when there is none to show. */
    val pending: StateFlow<T?> = _pending.asStateFlow()

    private var seenAny = false

    fun request(link: T) {
        requestedAtMillis = nowMillis()
        seenAny = true
        _pending.value = link
    }

    /** Whether any link reached this process yet (the cold-start marker). */
    fun hasSeenAnyLink(): Boolean = seenAny

    /**
     * True when the pending link outlived its broker session, so approving it
     * can only produce a dead-session failure. Mirrors the desktop's pending
     * request max age.
     */
    fun isStale(): Boolean {
        if (_pending.value == null) return false
        return nowMillis() - requestedAtMillis > ttlMillis
    }

    fun clear() {
        _pending.value = null
    }
}
