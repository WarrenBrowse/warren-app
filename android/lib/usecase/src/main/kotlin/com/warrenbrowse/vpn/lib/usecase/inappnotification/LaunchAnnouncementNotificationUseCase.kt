package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAnnouncementState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * The launch-announcement card (desktop `WarrenAnnouncementNotificationProvider`).
 *
 * The single banner slot shows one announcement at a time, and it is the first
 * the operator published: the set arrives in publication order, and reordering
 * it here would make which card a user sees depend on a rule nobody publishing
 * one can see.
 *
 * Nothing is filtered on this side beyond the dismissal. Whether an
 * announcement may be displayed at all was settled in Rust against the pinned
 * server key, so an empty list means exactly "there is nothing to show",
 * whether it was withdrawn, lapsed, or never verified.
 *
 * A dismissal is by announcement id and it is permanent: the card is an event
 * rather than a live statement, and it may already have handed over a code, so
 * raising it again on every launch would be nagging about something the reader
 * has dealt with.
 */
class LaunchAnnouncementNotificationUseCase(
    private val state: WarrenAnnouncementState,
    private val userPreferencesRepository: UserPreferencesRepository,
) : InAppNotificationUseCase {

    override operator fun invoke(): Flow<InAppNotification?> =
        combine(state.announcements, userPreferencesRepository.dismissedAnnouncements()) {
                announcements,
                dismissed ->
                announcements
                    .firstOrNull { it.id !in dismissed }
                    ?.let(InAppNotification::LaunchAnnouncement)
            }
            .distinctUntilChanged()
}
