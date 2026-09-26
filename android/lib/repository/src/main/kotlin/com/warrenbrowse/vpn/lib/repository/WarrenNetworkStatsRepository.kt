package com.warrenbrowse.vpn.lib.repository

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.NetworkStatsClock
import com.warrenbrowse.vpn.lib.model.NetworkStatsFetch
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStatsParser
import kotlin.time.ComparableTimeMark
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * When the next network stats fetch runs, from what the last one brought back.
 *
 * A snapshot is asked for once per window: the server changes it only when the next window closes,
 * so asking faster returns the same bytes. An API that does not serve it (404, the state of every
 * deployment until the endpoint ships) is asked again after ten minutes, then less often: it will
 * not grow the route within seconds. A transient failure retries from 15 s, doubling.
 */
object NetworkStatsCadence {
    val RETRY_MIN: Duration = 15.seconds
    val RETRY_MAX: Duration = 5.minutes
    val UNAVAILABLE_MIN: Duration = 10.minutes
    val UNAVAILABLE_MAX: Duration = 30.minutes

    /** The delay before the next fetch and the backoff to carry over. */
    fun next(fetch: NetworkStatsFetch, backoff: Duration?): Pair<Duration, Duration?> =
        when (fetch) {
            is NetworkStatsFetch.Snapshot ->
                NetworkStatsClock.pollInterval(fetch.stats.windowSecs) to null
            NetworkStatsFetch.Unavailable -> grow(backoff, UNAVAILABLE_MIN, UNAVAILABLE_MAX)
            NetworkStatsFetch.Failed -> grow(backoff, RETRY_MIN, RETRY_MAX)
        }

    private fun grow(backoff: Duration?, min: Duration, max: Duration): Pair<Duration, Duration> {
        val wait = backoff?.let { (it * 2).coerceIn(min, max) } ?: min
        return wait to wait
    }
}

/**
 * The network stats feed behind [WarrenNetworkStatsProvider]: polls only while [state] is
 * collected, keeps the last good snapshot, and never fetches on a schedule of its own.
 *
 * The time of the next fetch survives the gap between two collectors, so moving from one surface to
 * another (the picker, the network screen) costs no extra request inside a window.
 */
class WarrenNetworkStatsRepository(
    private val bridge: WarrenJniBridge,
    scope: CoroutineScope,
    private val io: CoroutineDispatcher = Dispatchers.IO,
    private val timeSource: TimeSource.WithComparableMarks = TimeSource.Monotonic,
    /**
     * True while the tunnel is between states: the host name still goes through the system
     * resolver, which then points at nothing, so the fetch is skipped rather than left to time out.
     */
    private val deferred: () -> Boolean = { false },
) : WarrenNetworkStatsProvider {
    private val _state = MutableStateFlow(WarrenNetworkStatsState.INITIAL)
    override val state: StateFlow<WarrenNetworkStatsState> = _state.asStateFlow()

    // Only the polling loop touches these, and it never runs twice at once (collectLatest).
    private var nextFetchAt: ComparableTimeMark? = null
    private var backoff: Duration? = null

    init {
        scope.launch {
            _state.subscriptionCount
                .map { it > 0 }
                .distinctUntilChanged()
                .collectLatest { watched -> if (watched) pollWhileWatched() }
        }
    }

    private suspend fun pollWhileWatched() {
        while (true) {
            nextFetchAt?.let { at -> delay(at - timeSource.markNow()) }
            val fetch = fetchOnce()
            val (wait, nextBackoff) = NetworkStatsCadence.next(fetch, backoff)
            backoff = nextBackoff
            nextFetchAt = timeSource.markNow() + wait
        }
    }

    private suspend fun fetchOnce(): NetworkStatsFetch {
        val fetch =
            if (deferred()) {
                NetworkStatsFetch.Failed
            } else {
                withContext(io) { fetchRaw() }?.let(WarrenNetworkStatsParser::parseEnvelope)
                    ?: NetworkStatsFetch.Failed
            }
        _state.update { it.after(fetch) }
        return fetch
    }

    // The JNI call is a system boundary: whatever crosses it as a throwable is one failed fetch,
    // retried on the fast cadence, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun fetchRaw(): String? =
        try {
            bridge.fetchNetworkStats()
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.fetchNetworkStats threw" }
            null
        }
}

/** The state after [fetch]: a snapshot replaces an older one only, and a failure keeps it. */
internal fun WarrenNetworkStatsState.after(fetch: NetworkStatsFetch): WarrenNetworkStatsState =
    when (fetch) {
        is NetworkStatsFetch.Snapshot ->
            WarrenNetworkStatsState(
                snapshot =
                    snapshot?.takeIf { it.generatedAt > fetch.stats.generatedAt } ?: fetch.stats,
                availability = NetworkStatsAvailability.AVAILABLE,
            )
        NetworkStatsFetch.Unavailable -> copy(availability = NetworkStatsAvailability.UNAVAILABLE)
        NetworkStatsFetch.Failed -> copy(availability = NetworkStatsAvailability.FAILING)
    }
