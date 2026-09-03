package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * A tap on the forum notification asking for the activity panel. Held until
 * the main flow is on screen and consumed there: the tap can cold-start the
 * app, and the navigator does not exist yet when the intent arrives.
 */
class ForumActivityOpenRequests {
    private val _pending = MutableStateFlow(false)
    val pending: StateFlow<Boolean> = _pending.asStateFlow()

    fun request() {
        _pending.value = true
    }

    fun consume() {
        _pending.value = false
    }
}
