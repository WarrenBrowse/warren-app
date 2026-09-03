package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * When the next digest fetch runs, from what the last one brought back: the
 * daemon's `warren_forum_digest_updater` cadence, kept here because Kotlin
 * owns the loop on Android.
 *
 * A minute between checks: how long a reply takes to raise a badge, and how
 * long a badge cleared elsewhere takes to drop here. Bounded by design: the
 * request is a conditional GET on a document identical for every client, so
 * a quiet forum answers it with a 304. After a transport failure the retry
 * doubles from 20 s up to 45 s, so a client that just regained a network does
 * not sit a full interval with a badge it can no longer justify; a server
 * that answered, whatever it said, clears the fast retry.
 */
object ForumDigestCadence {
    val CHECK_INTERVAL: Duration = 60.seconds
    val RETRY_MIN: Duration = 20.seconds
    val RETRY_MAX: Duration = 45.seconds

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
 * The foreground poll of the broadcast digest: one fetch on every resume,
 * then one a minute while the app is visible. No WorkManager and no service
 * wake-up carries it: a background cadence would make the app a periodic
 * presence signal for a badge nobody is looking at, and the fetch on resume
 * already catches up on whatever happened meanwhile (docs/warren-forum-login.md).
 *
 * The fetch rides the VpnService-protected transport, but the API host name
 * still goes through the system resolver, so a tunnel between states
 * ([ForumPreflight]) defers the fetch instead of hanging it for 15 s.
 */
class ForumDigestPoller(
    private val jni: WarrenJniBridge,
    private val activity: ForumActivityState,
    private val tunnelState: WarrenTunnelStateProvider,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) {
    /** Runs until cancelled: the caller scopes it to the visible lifecycle. */
    suspend fun run() {
        var retry: Duration? = null
        while (true) {
            val (wait, next) = ForumDigestCadence.next(fetchOnce(), retry)
            retry = next
            delay(wait)
        }
    }

    /**
     * [run] while [wanted] is true and nothing while it is false, so the
     * digest is fetched only while something on this installation reads it
     * ([forumDigestWanted]). Turning it back on fetches at once: a badge is
     * never a minute late after the feature comes back.
     */
    suspend fun runWhile(wanted: Flow<Boolean>) {
        wanted.distinctUntilChanged().collectLatest { if (it) run() }
    }

    /** One fetch fed into the activity state; returns the fetch class. */
    suspend fun fetchOnce(): String {
        if (ForumPreflight.of(tunnelState.connectedInfo.value) is ForumPreflight.Defer) {
            return ForumDigestCadence.FETCH_DEFERRED
        }
        val raw = withContext(io) { fetchRaw() }
        return if (raw == null) {
            ForumDigestCadence.FETCH_TRANSPORT
        } else {
            val (counts, fetch) = parseDigestEnvelope(raw)
            activity.setDigest(counts)
            fetch
        }
    }

    // The JNI call is a system boundary: whatever crosses it as a throwable
    // is one failed fetch, retried on the fast cadence, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun fetchRaw(): String? =
        try {
            jni.forumDigestFetch()
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.forumDigestFetch threw" }
            null
        }
}

/**
 * Whether the broadcast digest has a reader on this installation: the privacy
 * disclosure accepted (nothing leaves the device before it), the forum
 * notifications setting on (off hides the bell, so no surface would show the
 * count) and a forum account, whose slot is what indexes the digest. Without
 * all three the fetch is a periodic handshake with the API host, from the
 * physical network since the forum flows ride the VpnService-protected
 * transport, for a number nobody displays.
 *
 * The desktop daemon polls the digest unconditionally, beside its relay-list
 * and notices refreshes on the channel its own API traffic already uses, and
 * the GUI setting does not reach it; here the loop is the app's own, so the
 * gate costs nothing and is applied.
 */
fun forumDigestWanted(
    disclosureAccepted: Flow<Boolean>,
    notificationsEnabled: Flow<Boolean>,
    identity: Flow<ForumIdentity?>,
): Flow<Boolean> =
    combine(disclosureAccepted, notificationsEnabled, identity) { accepted, enabled, forumIdentity ->
        accepted && enabled && forumIdentity != null
    }

/**
 * The `{"counts":..,"fetch":..}` JNI envelope: the verified counts (null while
 * no fresh document is held) and the fetch class. Pure, unit-tested.
 */
internal fun parseDigestEnvelope(rawJson: String): Pair<String?, String> =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val counts = root["counts"]?.jsonPrimitive?.contentOrNull?.takeIf { it != "null" }
        val fetch = root["fetch"]?.jsonPrimitive?.contentOrNull ?: ForumDigestCadence.FETCH_TRANSPORT
        counts to fetch
    } catch (e: IllegalArgumentException) {
        null to ForumDigestCadence.FETCH_TRANSPORT
    }
