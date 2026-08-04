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
class ForumLoginController {
    private val _pending = MutableStateFlow<ForumLoginLink?>(null)

    /** The pending consent request, or null when there is none to show. */
    val pending: StateFlow<ForumLoginLink?> = _pending.asStateFlow()

    fun request(link: ForumLoginLink) {
        _pending.value = link
    }

    fun clear() {
        _pending.value = null
    }
}
