package com.warrenbrowse.vpn.app.notices

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.WarrenNotice
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeState
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * When the next notices fetch runs, from what the last one brought back: the
 * daemon's `warren_notices_updater` cadence, kept here because Kotlin owns the
 * loop on Android.
 *
 * Five minutes between checks, which is the delay an operator waits for a
 * publication or an erasure to reach a running client. Not shorter: the poll
 * is traffic the API can see, and erasing within minutes is what the feature
 * needs, not within seconds. After a transport failure the retry doubles from
 * 20 s up to 240 s, so a client that just regained a network does not sit a
 * full interval showing nothing (or showing a notice whose envelope is about
 * to lapse); a server that answered, whatever it said, clears the fast retry.
 */
object WarrenNoticeCadence {
    val CHECK_INTERVAL: Duration = 5.minutes
    val RETRY_MIN: Duration = 20.seconds
    val RETRY_MAX: Duration = 240.seconds

    /** The delay before the next fetch and the retry state to carry over. */
    fun next(fetch: String, retry: Duration?): Pair<Duration, Duration?> =
        when (fetch) {
            FETCH_TRANSPORT,
            FETCH_DEFERRED -> {
                val armed = retry?.let { (it * 2).coerceAtMost(RETRY_MAX) } ?: RETRY_MIN
                armed to armed
            }
            else -> CHECK_INTERVAL to null
        }

    const val FETCH_TRANSPORT = "transport"

    /** The tunnel is between states: nothing was fetched, so it counts as unreachable. */
    const val FETCH_DEFERRED = "deferred"
}

/**
 * The foreground poll of the operator broadcast notices: one fetch on every
 * resume, then one every five minutes while the app is visible.
 *
 * No WorkManager and no service wake-up carries it. A background cadence would
 * make the app a periodic beacon for a banner nobody is looking at, and the
 * fetch on resume already catches up on whatever the operator published
 * meanwhile. That is the one place this differs from the desktop, where the
 * daemon polls beside its own relay-list refresh on a channel its API traffic
 * already uses.
 *
 * The fetch rides the shared API transport, so it leaves through the tunnel
 * whenever one is up. The host name still goes through the system resolver, so
 * a tunnel between states ([ForumPreflight]) defers the fetch instead of
 * hanging it for the transport's full timeout.
 */
class WarrenNoticePoller(
    private val jni: WarrenJniBridge,
    private val state: WarrenNoticeState,
    private val tunnelState: WarrenTunnelStateProvider,
    private val clientVersion: String,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) {
    /** Runs until cancelled: the caller scopes it to the visible lifecycle. */
    suspend fun run() {
        var retry: Duration? = null
        while (true) {
            val (wait, next) = WarrenNoticeCadence.next(fetchOnce(), retry)
            retry = next
            delay(wait)
        }
    }

    /**
     * [run] while [wanted] is true and nothing while it is false. The gate is
     * the privacy disclosure: nothing leaves this device before the user has
     * accepted it, and a notice is worth no exception. Accepting fetches at
     * once, so the first banner is never five minutes late.
     */
    suspend fun runWhile(wanted: Flow<Boolean>) {
        wanted.distinctUntilChanged().collectLatest { if (it) run() }
    }

    /** One fetch published to [WarrenNoticeState]; returns the fetch class. */
    suspend fun fetchOnce(): String {
        if (ForumPreflight.of(tunnelState.connectedInfo.value) is ForumPreflight.Defer) {
            // Nothing was fetched, so nothing is published: a deferred cycle
            // must not take down a banner the operator has not erased.
            return WarrenNoticeCadence.FETCH_DEFERRED
        }
        val raw = withContext(io) { fetchRaw() }
        return if (raw == null) {
            WarrenNoticeCadence.FETCH_TRANSPORT
        } else {
            val (notices, fetch) = parseNoticesEnvelope(raw)
            // Published on every readable cycle, the empty list included: that
            // is what takes the banner down when the notice is erased or
            // lapses. An unreadable envelope publishes nothing at all, so a
            // parsing bug can never erase a live operator message.
            notices?.let(state::setNotices)
            fetch
        }
    }

    // The JNI call is a system boundary: whatever crosses it as a throwable is
    // one failed fetch, retried on the fast cadence, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun fetchRaw(): String? =
        try {
            jni.noticesFetch(clientVersion)
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.noticesFetch threw" }
            null
        }
}

/**
 * The `{"notices":[..],"fetch":..}` JNI envelope: the notices to display and
 * the fetch class. Pure, unit-tested.
 *
 * A null list means the envelope could not be read at all, which is a failed
 * fetch rather than an empty set: taking the banner down because the boundary
 * answered nonsense would let a parsing bug erase a live operator message. A
 * single unreadable ROW is dropped and the rest of the set still shows, for
 * the same reason in the other direction.
 */
internal fun parseNoticesEnvelope(rawJson: String): Pair<List<WarrenNotice>?, String> =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val fetch =
            root["fetch"]?.jsonPrimitive?.contentOrNull ?: WarrenNoticeCadence.FETCH_TRANSPORT
        val notices =
            root["notices"]?.jsonArray.orEmpty().mapNotNull { element ->
                val row = element.jsonObject
                val id = row["id"]?.jsonPrimitive?.contentOrNull
                val message = row["message"]?.jsonPrimitive?.contentOrNull
                if (id.isNullOrEmpty() || message.isNullOrEmpty()) {
                    null
                } else {
                    WarrenNotice(
                        id = id,
                        message = message,
                        level =
                            WarrenNoticeLevel.of(
                                row["level"]?.jsonPrimitive?.contentOrNull.orEmpty()
                            ),
                    )
                }
            }
        notices to fetch
    } catch (e: IllegalArgumentException) {
        Logger.w(throwable = e) { "noticesFetch answered a malformed envelope" }
        null to WarrenNoticeCadence.FETCH_TRANSPORT
    }
