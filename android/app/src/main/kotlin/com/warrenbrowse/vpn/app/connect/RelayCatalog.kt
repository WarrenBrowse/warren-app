package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import kotlin.time.ComparableTimeMark
import kotlin.time.Duration.Companion.minutes
import kotlin.time.TimeSource
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json

/**
 * Single point of access to the Warren relay catalogue.
 *
 * [WarrenJni.listRelays] performs a signed `GET /v1/exits` and blocks the
 * calling thread for the whole round trip, so the read path and the fetch
 * path are deliberately separate here: [list] serves an in-memory snapshot
 * and never touches the network, [refresh] and [refreshIfStale] are the
 * entry points that fetch and they move to [Dispatchers.IO] first. Calling
 * the fetch during composition froze the UI for over a second per navigation.
 *
 * The snapshot ages the way the desktop daemon's does: its relay-list
 * updater refetches only once the list is an hour old
 * (`mullvad-daemon/src/warren_relay_list_updater.rs`, `UPDATE_INTERVAL`) and
 * serves the last good copy in between. Every reader here follows that rule,
 * the dial included, so an exit switch no longer refetches the list it was
 * picked from.
 *
 * Held as a Koin `single` so every consumer (home screen, location picker,
 * multi-hop settings, connect flow) shares one cache.
 */
class RelayCatalog(
    // Injectable so a test can age the snapshot without waiting an hour.
    private val timeSource: TimeSource.WithComparableMarks = TimeSource.Monotonic,
    private val fetchRelaysJson: () -> String = { WarrenJni.listRelays() },
) : WarrenRelayProvider {
    private val json = Json { ignoreUnknownKeys = true }

    private val cache = MutableStateFlow<List<WarrenRelaySummary>>(emptyList())

    // When the current snapshot was fetched; null while nothing usable was
    // ever fetched, so an empty answer is retried on the next read.
    @Volatile private var fetchedAt: ComparableTimeMark? = null

    /** Cached catalogue, updated by every fetch. Empty until the first lands. */
    override val catalogue: StateFlow<List<WarrenRelaySummary>> = cache.asStateFlow()

    /**
     * Snapshot of the last fetched catalogue. Safe to read during composition
     * and from any thread; an empty list means "not loaded yet", not "no
     * relays". Never fetches.
     */
    override fun list(): List<WarrenRelaySummary> = cache.value

    /** True while the snapshot is non-empty and younger than [STALE_AFTER]. */
    val isFresh: Boolean
        get() = fetchedAt?.let { it.elapsedNow() < STALE_AFTER } == true

    /**
     * Fetch the catalogue off the main thread and publish it to the cache,
     * whatever its age (the user's explicit retry).
     *
     * The native side already falls back to its own last-good list on a
     * network or signature failure, so whatever comes back is published as
     * is rather than being merged with the previous snapshot.
     */
    override suspend fun refresh(): List<WarrenRelaySummary> =
        withContext(Dispatchers.IO) { fetchAndPublish() }

    /** The snapshot while it is fresh, otherwise [refresh]. */
    override suspend fun refreshIfStale(): List<WarrenRelaySummary> =
        if (isFresh) list() else refresh()

    /**
     * The catalogue a dial resolves its exit from: the fresh snapshot, or a
     * blocking fetch that also feeds every other reader. Blocks on a signed
     * round trip when it fetches, so call it off the main thread.
     */
    fun relaysForDial(): List<WarrenRelaySummary> = if (isFresh) list() else fetchAndPublish()

    private fun fetchAndPublish(): List<WarrenRelaySummary> {
        val fetched = listRelays().map { it.toSummary() }
        fetchedAt = if (fetched.isEmpty()) null else timeSource.markNow()
        cache.value = fetched
        return fetched
    }

    private fun listRelays(): List<RelayInfo> {
        val raw =
            try {
                fetchRelaysJson()
            } catch (e: Throwable) {
                Logger.e(throwable = e) { "WarrenJni.listRelays threw" }
                return emptyList()
            }
        return try {
            json.decodeFromString<List<RelayInfo>>(raw)
        } catch (e: Exception) {
            Logger.e(throwable = e) { "Failed to parse listRelays() JSON: $raw" }
            emptyList()
        }
    }

    private companion object {
        /** The daemon's `UPDATE_INTERVAL`: a list older than this is refetched. */
        val STALE_AFTER = 60.minutes
    }
}

internal fun RelayInfo.toSummary(): WarrenRelaySummary =
    WarrenRelaySummary(
        exitId = exitId,
        exitPubkeyHex = exitPubkeyHex,
        endpoint = endpoint,
        country = country,
        city = city,
        active = active,
        weight = weight,
    )
