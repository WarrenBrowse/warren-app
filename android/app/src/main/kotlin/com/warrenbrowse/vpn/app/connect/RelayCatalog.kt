package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
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
 * and never touches the network, [refresh] is the only entry point that
 * fetches and it moves to [Dispatchers.IO] first. Calling the fetch during
 * composition froze the UI for over a second per navigation.
 *
 * Held as a Koin `single` so every consumer (home screen, location picker,
 * multi-hop settings, connect flow) shares one cache.
 */
class RelayCatalog(
    private val fetchRelaysJson: () -> String = { WarrenJni.listRelays() },
) : WarrenRelayProvider {
    private val json = Json { ignoreUnknownKeys = true }

    private val cache = MutableStateFlow<List<WarrenRelaySummary>>(emptyList())

    /** Cached catalogue, updated by [refresh]. Empty until the first fetch lands. */
    override val catalogue: StateFlow<List<WarrenRelaySummary>> = cache.asStateFlow()

    /**
     * Snapshot of the last fetched catalogue. Safe to read during composition
     * and from any thread; an empty list means "not loaded yet", not "no
     * relays". Never fetches.
     */
    override fun list(): List<WarrenRelaySummary> = cache.value

    /**
     * Fetch the catalogue off the main thread and publish it to the cache.
     *
     * The native side already falls back to its own last-good list on a
     * network or signature failure, so whatever comes back is published as
     * is rather than being merged with the previous snapshot.
     */
    override suspend fun refresh(): List<WarrenRelaySummary> =
        withContext(Dispatchers.IO) { listRelays().map { it.toSummary() } }
            .also { cache.value = it }

    /**
     * Blocking fetch + parse of the relay catalogue. Blocks on a signed
     * network round trip, so call it off the main thread. The connect flow
     * uses it directly because it needs a fresh catalogue even when the
     * cache is still cold.
     */
    fun listRelays(): List<RelayInfo> {
        val raw = try {
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
}

internal fun RelayInfo.toSummary(): WarrenRelaySummary = WarrenRelaySummary(
    exitId = exitId,
    exitPubkeyHex = exitPubkeyHex,
    endpoint = endpoint,
    country = country,
    city = city,
    active = active,
    weight = weight,
)
