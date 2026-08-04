package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

/**
 * Subscription expiry banner, fed by the cached expiry the subscription fetch
 * and the purchase poll both write.
 *
 * It is an ordinary in-app notification rather than a strip of its own, so it
 * competes for the single banner slot and loses to a tunnel error or an
 * unsupported version, exactly as on desktop. Before this it was drawn above
 * the banner, which let the connect screen show two cards at once.
 */
class AccountExpiryNotificationUseCase(
    private val localSettings: WarrenLocalSettingsRepository,
    private val now: () -> Long = { System.currentTimeMillis() / 1000 },
) : InAppNotificationUseCase {
    override operator fun invoke(): Flow<InAppNotification?> =
        localSettings.cachedSubscriptionExpiry
            .map { expiry -> closeToExpiryNotification(expiry, now()) }
            .distinctUntilChanged()

    companion object {
        private const val DAY_SECS = 86_400L

        // Desktop closeToExpiry window. The home header hides "Time left"
        // inside it, so the banner is what carries the remaining time.
        private const val WARNING_WINDOW_SECS = 3L * DAY_SECS

        fun closeToExpiryNotification(expiryUnixSecs: Long, nowSecs: Long): InAppNotification? =
            when {
                expiryUnixSecs <= 0L -> null
                expiryUnixSecs <= nowSecs -> InAppNotification.CloseToExpiry(daysLeft = 0)
                expiryUnixSecs - nowSecs <= WARNING_WINDOW_SECS ->
                    InAppNotification.CloseToExpiry(
                        // Rounded up: the last hours of a subscription still
                        // read as a day left rather than as zero.
                        daysLeft = (expiryUnixSecs - nowSecs + DAY_SECS - 1) / DAY_SECS
                    )
                else -> null
            }
    }
}
