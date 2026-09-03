package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.WarrenNotice
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The operator broadcast notices this installation is currently to display.
 *
 * Every fetch publishes the whole set, the empty one included, so the banner
 * clears from the same signal that raised it: there is no dismiss, no per
 * notice acknowledgement and no timer on this side. What decides whether a
 * notice may be shown at all (the signature, the anti-rollback generation, the
 * signed expiry, the per-notice TTL, the client-version range) is enforced in
 * Rust, which is why this holds only what came back.
 */
interface WarrenNoticeState {
    /** Notices to display right now, in publication order; empty when there are none. */
    val notices: StateFlow<List<WarrenNotice>>

    /** What the last fetch decided is displayable. */
    fun setNotices(notices: List<WarrenNotice>)
}

/**
 * In-memory home of [WarrenNoticeState]. Deliberately not persisted: a notice
 * is a live statement by the operator, and a copy on disk would only be a way
 * for an erased message to come back on a client that cannot reach the API.
 */
class WarrenNoticeRepository : WarrenNoticeState {
    private val _notices = MutableStateFlow<List<WarrenNotice>>(emptyList())
    override val notices: StateFlow<List<WarrenNotice>> = _notices.asStateFlow()

    override fun setNotices(notices: List<WarrenNotice>) {
        _notices.value = notices
    }
}
