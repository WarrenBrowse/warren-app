package com.warrenbrowse.vpn.app.connect

import java.util.concurrent.atomic.AtomicInteger
import kotlin.time.Duration.Companion.minutes
import kotlin.time.TestTimeSource
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The catalogue fetch is a blocking, signed `GET /v1/exits` behind the JNI
 * boundary, so the contract these tests pin is a threading one: `list()` is
 * the snapshot composition reads and must never reach the network, while
 * `refresh()` is the only path that does and must leave the caller's thread.
 * The fetch is injected so the real JNI symbol (absent on the JVM) is never
 * loaded.
 *
 * The second contract is the daemon's refresh cadence: the desktop relay-list
 * updater refetches a list only once it is an hour old
 * (`mullvad-daemon/src/warren_relay_list_updater.rs`, `UPDATE_INTERVAL`), and
 * every Android reader (home screen, picker, the dial itself) follows the same
 * rule instead of refetching on every navigation.
 */
class RelayCatalogTest {

    private val sample = RelayInfo(
        exitId = "2921abad869e94064b56cf48c8da3631",
        exitPubkeyHex = "2921abad869e94064b56cf48c8da3631",
        endpoint = "warren-exit-1.warren.brown:443",
        country = "DE",
        city = "Falkenstein",
        active = true,
        weight = 100,
    )

    private val sampleJson = """
        [{"exit_id":"2921abad869e94064b56cf48c8da3631",
          "exit_pubkey_hex":"2921abad869e94064b56cf48c8da3631",
          "endpoint":"warren-exit-1.warren.brown:443",
          "country":"DE","city":"Falkenstein","active":true,"weight":100}]
    """.trimIndent()

    @Test
    fun `ensure list does not fetch before a refresh`() {
        val calls = AtomicInteger(0)
        val catalog = RelayCatalog { calls.incrementAndGet(); sampleJson }

        val snapshot = catalog.list()

        assertTrue(snapshot.isEmpty(), "cold cache must read empty, not fetch")
        assertEquals(0, calls.get(), "list() must never trigger the network fetch")
    }

    @Test
    fun `ensure refresh publishes the fetched catalogue to the cache`() = runTest {
        val catalog = RelayCatalog { sampleJson }

        val refreshed = catalog.refresh()

        assertEquals(1, refreshed.size)
        assertEquals(sample.exitId, refreshed.first().exitId)
        assertEquals("Falkenstein", refreshed.first().city)
        // The same snapshot is now readable without any further fetch.
        assertEquals(refreshed, catalog.list())
    }

    @Test
    fun `ensure list still does not fetch once the cache is warm`() = runTest {
        val calls = AtomicInteger(0)
        val catalog = RelayCatalog { calls.incrementAndGet(); sampleJson }

        catalog.refresh()
        repeat(5) { catalog.list() }

        assertEquals(1, calls.get(), "only refresh() may reach the fetch")
    }

    @Test
    fun `ensure refresh runs the blocking fetch off the calling thread`() = runTest {
        val callerThread = Thread.currentThread().name
        var fetchThread: String? = null
        val catalog = RelayCatalog {
            fetchThread = Thread.currentThread().name
            sampleJson
        }

        catalog.refresh()

        assertNotEquals(
            callerThread,
            fetchThread,
            "the blocking fetch must not run on the caller's thread (this is the UI freeze)",
        )
    }

    @Test
    fun `ensure refresh yields an empty catalogue when the fetch throws`() = runTest {
        val catalog = RelayCatalog { throw IllegalStateException("JNI unavailable") }

        assertTrue(catalog.refresh().isEmpty())
        assertTrue(catalog.list().isEmpty())
    }

    @Test
    fun `ensure refresh yields an empty catalogue on malformed JSON`() = runTest {
        val catalog = RelayCatalog { "{not-a-relay-array}" }

        assertTrue(catalog.refresh().isEmpty())
    }

    @Test
    fun `ensure RelayInfo round-trips a fully-populated relay`() {
        // The data class must stay wide enough to hold every field the JNI
        // side ships; a regression here silently drops picker attributes.
        assertEquals("2921abad869e94064b56cf48c8da3631", sample.exitId)
        assertEquals("warren-exit-1.warren.brown:443", sample.endpoint)
        assertEquals("DE", sample.country)
        assertEquals(100L, sample.weight)
        assertTrue(sample.active)
    }

    @Test
    fun `ensure refreshIfStale serves the snapshot while it is younger than the daemon bound`() =
        runTest {
            val calls = AtomicInteger(0)
            val clock = TestTimeSource()
            val catalog =
                RelayCatalog(
                    fetchRelaysJson = {
                        calls.incrementAndGet()
                        sampleJson
                    },
                    timeSource = clock,
                )

            catalog.refreshIfStale()
            clock += 59.minutes
            val served = catalog.refreshIfStale()

            assertEquals(1, calls.get(), "a snapshot under an hour old must not be refetched")
            assertEquals(sample.exitId, served.single().exitId)
        }

    @Test
    fun `ensure refreshIfStale refetches once the snapshot is an hour old`() = runTest {
        val calls = AtomicInteger(0)
        val clock = TestTimeSource()
        val catalog =
            RelayCatalog(
                fetchRelaysJson = {
                    calls.incrementAndGet()
                    sampleJson
                },
                timeSource = clock,
            )

        catalog.refreshIfStale()
        clock += 60.minutes
        catalog.refreshIfStale()

        assertEquals(2, calls.get(), "an hour-old snapshot is stale, as it is for the daemon")
    }

    @Test
    fun `ensure an empty fetch never counts as fresh`() = runTest {
        // The native side answers `[]` while nothing was ever fetched (offline
        // first launch); treating that as a fresh snapshot would leave the app
        // with no exits for an hour.
        val calls = AtomicInteger(0)
        val catalog = RelayCatalog(fetchRelaysJson = { calls.incrementAndGet(); "[]" })

        catalog.refreshIfStale()
        catalog.refreshIfStale()

        assertEquals(2, calls.get(), "an empty catalogue must be retried on the next read")
    }

    @Test
    fun `ensure the dial takes a fresh snapshot without a fetch`() = runTest {
        val calls = AtomicInteger(0)
        val catalog = RelayCatalog(fetchRelaysJson = { calls.incrementAndGet(); sampleJson })

        val shown = catalog.refresh()
        val dialled = catalog.relaysForDial()

        assertEquals(1, calls.get(), "an exit switch must not refetch the list it was picked from")
        assertEquals(shown, dialled)
    }

    @Test
    fun `ensure the dial refetches a stale snapshot and publishes it`() = runTest {
        val calls = AtomicInteger(0)
        val clock = TestTimeSource()
        val catalog =
            RelayCatalog(
                fetchRelaysJson = {
                    calls.incrementAndGet()
                    sampleJson
                },
                timeSource = clock,
            )

        catalog.refresh()
        clock += 60.minutes
        val dialled = catalog.relaysForDial()

        assertEquals(2, calls.get(), "a stale snapshot is refetched before a dial")
        assertEquals(dialled, catalog.list(), "the dial's fetch feeds every other reader")
    }
}
