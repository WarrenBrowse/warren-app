package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The launch announcements this installation is currently to display.
 *
 * Every readable fetch publishes the whole set, the empty one included, so the
 * card clears from the same signal that raised it. What decides whether an
 * announcement may be shown at all (the signature, the anti-rollback
 * generation, the signed expiry, the per-announcement TTL, the client-version
 * range, the call-to-action URL) is enforced in Rust, which is why this holds
 * only what came back.
 *
 * The reader's dismissal is NOT here: it has to outlive the process, so it sits
 * in [UserPreferencesRepository] beside the other things the user has put away.
 */
interface WarrenAnnouncementState {
    /** Announcements to display right now, in publication order; empty when there are none. */
    val announcements: StateFlow<List<WarrenAnnouncement>>

    /** What the last fetch decided is displayable. */
    fun setAnnouncements(announcements: List<WarrenAnnouncement>)
}

/**
 * In-memory home of [WarrenAnnouncementState]. Deliberately not persisted: a
 * withdrawn announcement would otherwise come back on a client that cannot
 * reach the API, and the voucher code it carries would outlive the campaign on
 * disk.
 */
class WarrenAnnouncementRepository : WarrenAnnouncementState {
    private val _announcements = MutableStateFlow<List<WarrenAnnouncement>>(emptyList())
    override val announcements: StateFlow<List<WarrenAnnouncement>> = _announcements.asStateFlow()

    override fun setAnnouncements(announcements: List<WarrenAnnouncement>) {
        _announcements.value = announcements
    }
}
