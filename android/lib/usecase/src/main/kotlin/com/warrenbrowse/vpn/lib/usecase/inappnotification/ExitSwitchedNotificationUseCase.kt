package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenFailoverProvider
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * "EXIT SWITCHED": an automatic retry landed on another exit than the one that
 * dropped (desktop WarrenFailoverNotificationProvider). The banner shows while
 * the failover count is ahead of what the user acknowledged, so dismissing it
 * keeps it down until the next switch, and a switch that happened while the
 * screen was away is still read.
 */
class ExitSwitchedNotificationUseCase(private val failoverProvider: WarrenFailoverProvider) :
    InAppNotificationUseCase {
    private val acknowledged = MutableStateFlow(0)

    override operator fun invoke(): Flow<InAppNotification?> =
        combine(failoverProvider.failoverCount, acknowledged) { count, seen ->
                if (count > seen) InAppNotification.ExitSwitched else null
            }
            .distinctUntilChanged()

    /** The user closed the banner: every switch so far is read. */
    fun acknowledge() {
        acknowledged.value = failoverProvider.failoverCount.value
    }
}
