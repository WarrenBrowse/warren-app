package com.warrenbrowse.vpn.screen.outoftime

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

/**
 * Whether the subscription has lapsed, as one boolean the root composable
 * collects.
 *
 * The root used to collect the cached expiry, the whole tunnel state and a
 * 60 s clock itself, which recomposed the entire app shell (and rebuilt its
 * navigation entry provider) on every tunnel transition, right through the
 * connect animation. Everything the verdict depends on is combined here, so
 * the shell recomposes only when the verdict itself changes.
 *
 * The tunnel contributes one bit, the exit having refused the account, and
 * only its changes propagate. The clock re-evaluates on the exact expiry
 * boundary so the gate raises itself the moment the subscription lapses
 * while the app is open (iOS timer parity), and on a coarse tick between.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class OutOfTimeGateViewModel(
    localSettings: WarrenLocalSettingsRepository,
    tunnelStateProvider: WarrenTunnelStateProvider,
    private val nowSecs: () -> Long = { System.currentTimeMillis() / MILLIS_PER_SEC },
) : ViewModel() {

    private val tunnelExpired =
        tunnelStateProvider.connectedInfo.map { it.refusedAccount() }.distinctUntilChanged()

    val lapsed: StateFlow<Boolean> =
        combine(localSettings.cachedSubscriptionExpiry, tunnelExpired) { expiry, refused ->
                expiry to refused
            }
            .flatMapLatest { (expiry, refused) ->
                flow {
                    while (true) {
                        val now = nowSecs()
                        emit(subscriptionLapsed(expiry, now, refused))
                        delay(reevaluationDelaySecs(expiry, now) * MILLIS_PER_SEC)
                    }
                }
            }
            .distinctUntilChanged()
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS),
                subscriptionLapsed(
                    localSettings.cachedSubscriptionExpiry.value,
                    nowSecs(),
                    tunnelStateProvider.connectedInfo.value.refusedAccount(),
                ),
            )

    private companion object {
        const val MILLIS_PER_SEC = 1_000L
        const val STOP_TIMEOUT_MILLIS = 5_000L
    }
}

/** True when the exit refused the account (lapsed or revoked subscription). */
internal fun WarrenConnectedInfo.refusedAccount(): Boolean =
    when (this) {
        is WarrenConnectedInfo.Blocking -> expired
        is WarrenConnectedInfo.Failed -> expired
        else -> false
    }

/** Seconds until the verdict is worth re-evaluating: the expiry boundary when it is near, a coarse tick otherwise. */
internal fun reevaluationDelaySecs(expiryUnixSecs: Long, nowSecs: Long): Long {
    val untilExpiry = expiryUnixSecs - nowSecs
    return if (untilExpiry in 1..OUT_OF_TIME_TICK_SECS) untilExpiry else OUT_OF_TIME_TICK_SECS
}

// Coarse re-evaluation tick for the out-of-time verdict between expiry boundaries.
internal const val OUT_OF_TIME_TICK_SECS = 60L
