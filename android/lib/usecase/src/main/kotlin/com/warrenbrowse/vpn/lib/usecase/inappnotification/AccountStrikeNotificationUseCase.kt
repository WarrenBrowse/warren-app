package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * The port-forward strike banner (warren-core doc 105, desktop
 * `WarrenAccountStrikeNotificationProvider`): the newest live strike, until
 * the reader puts it away. The dismissal is keyed on that strike, so the next
 * one raises the banner again, and it rides the notice dismissals: a digest of
 * the case reference, never the reference itself.
 */
class AccountStrikeNotificationUseCase(
    private val state: WarrenAccountStandingState,
    private val userPreferencesRepository: UserPreferencesRepository,
) : InAppNotificationUseCase {

    override operator fun invoke(): Flow<InAppNotification?> =
        combine(state.standing, userPreferencesRepository.dismissedNotices()) { standing, dismissed ->
            standing
                ?.latestStrike()
                ?.takeIf { it.strike.dismissalKey !in dismissed }
                ?.let(InAppNotification::AccountStrike)
        }
            .distinctUntilChanged()
}
