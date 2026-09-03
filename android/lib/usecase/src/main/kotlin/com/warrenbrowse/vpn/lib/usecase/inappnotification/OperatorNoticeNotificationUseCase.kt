package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * The operator broadcast banner (desktop `WarrenNoticeNotificationProvider`).
 *
 * The single banner slot shows one notice at a time, and it is the first the
 * operator published: the set arrives in publication order, and reordering it
 * here would make which message a user sees depend on a rule nobody publishing
 * one can see.
 *
 * Whether a notice may be displayed at all was settled in Rust against the
 * pinned server key, so an empty list means exactly "there is nothing to
 * show", whether it was erased, lapsed, or never verified.
 *
 * An informational notice the user has read can be put away, and only that
 * one: the slot is ranked with the notice on top, so a message the operator
 * leaves up for a week would otherwise hide the update prompt and the expiry
 * warning for that whole week. A warning or an error keeps the slot, because
 * it describes something live that the user cannot act on by hiding it.
 */
class OperatorNoticeNotificationUseCase(
    private val state: WarrenNoticeState,
    private val userPreferencesRepository: UserPreferencesRepository,
) : InAppNotificationUseCase {

    override operator fun invoke(): Flow<InAppNotification?> =
        combine(state.notices, userPreferencesRepository.dismissedNotices()) { notices, dismissed ->
            notices
                .firstOrNull { it.level != WarrenNoticeLevel.INFO || it.dismissalKey !in dismissed }
                ?.let(InAppNotification::OperatorNotice)
        }
            .distinctUntilChanged()
}
