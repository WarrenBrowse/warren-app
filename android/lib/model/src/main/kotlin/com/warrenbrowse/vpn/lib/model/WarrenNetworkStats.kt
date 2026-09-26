package com.warrenbrowse.vpn.lib.model

/**
 * The public network transparency snapshot (`GET /v1/network/stats`, warren-core doc 106), as
 * the UI reads it.
 *
 * Rust passes the body through untouched after checking it is a version 1 JSON object, so the
 * parser in [WarrenNetworkStatsParser] is the only place that knows the wire shape. It follows the
 * contract's evolution rules: unknown fields are ignored, an unknown band or driver reads as
 * [LoadLevel.UNKNOWN] / [LoadDriver.UNKNOWN], and an optional figure the server left out stays
 * null instead of turning into a zero the UI would then display.
 *
 * `uptime_secs` is deliberately not modelled: the reference server withholds it (a drop to zero
 * dates a restart), and a client that read it would have a field to render.
 */
data class WarrenNetworkStats(
    val environment: String,
    /** Unix seconds at which the window this snapshot describes closed. */
    val generatedAt: Long,
    val windowSecs: Int,
    /** Per-exit people counts are floored to a multiple of this. At least 1. */
    val exitUsersRounding: Int,
    /** Fewest people an exit must carry for its live figures to be published. */
    val exitLiveThreshold: Int,
    val users: NetworkUsers,
    val fleet: FleetStats,
    val exits: List<ExitStats>,
    /** One point per clock hour over the last 24, oldest first. */
    val history: List<FleetHistoryPoint>,
) {
    /** The exit keyed by [exitId] (32 hex chars, any case). */
    fun exit(exitId: String): ExitStats? = exits.firstOrNull { it.exitId == exitId.lowercase() }

    /** Whether any exit publishes live figures, which floors the fleet people count. */
    val anyExitLive: Boolean
        get() = exits.any { it.live }
}

data class NetworkUsers(val accountsTotal: Long, val subscribersActive: Long, val connected: Int)

data class FleetStats(
    val exitsOnline: Int,
    val exitsTotal: Int,
    val downloadBps: Long,
    val uploadBps: Long,
    val capacityBps: Long,
    val loadPercent: Int,
    val loadLevel: LoadLevel,
    /** Bytes carried over the 24 clock hours ending on the last one. */
    val transferred24hBytes: Long,
    val peakConnected24h: Int,
    val peakThroughput24hBps: Long,
)

data class ExitStats(
    /** Lowercase hex, the join key with the signed relay list. */
    val exitId: String,
    val name: String?,
    val country: String,
    val city: String,
    val online: Boolean,
    /** When false only [loadLevel] (over the last clock hour) and [capacityBps] mean anything. */
    val live: Boolean,
    val connected: Int,
    val downloadBps: Long,
    val uploadBps: Long,
    val capacityBps: Long?,
    val loadPercent: Int?,
    val loadLevel: LoadLevel,
    val loadDriver: LoadDriver?,
    val cpuPercent: Int?,
    val history: List<ExitHistoryPoint>,
) {
    val displayMode: ExitDisplayMode
        get() =
            when {
                !online -> ExitDisplayMode.OFFLINE
                live -> ExitDisplayMode.LIVE
                else -> ExitDisplayMode.BAND
            }
}

data class ExitHistoryPoint(
    val t: Long,
    val connected: Int,
    val throughputBps: Long,
    val loadPercent: Int?,
)

data class FleetHistoryPoint(val t: Long, val connected: Int, val throughputBps: Long)

/** Load band, decided server-side so every client colours the same exit the same way. */
enum class LoadLevel {
    LOW,
    MODERATE,
    HIGH,
    SATURATED,
    UNKNOWN;

    companion object {
        fun of(token: String?): LoadLevel =
            when (token) {
                "low" -> LOW
                "moderate" -> MODERATE
                "high" -> HIGH
                "saturated" -> SATURATED
                else -> UNKNOWN
            }
    }
}

/** The resource that set an exit's load. */
enum class LoadDriver {
    BANDWIDTH,
    CPU,
    UNKNOWN;

    companion object {
        /** Null when absent, [UNKNOWN] for a driver this build does not know. */
        fun of(token: String?): LoadDriver? =
            when (token) {
                null -> null
                "bandwidth" -> BANDWIDTH
                "cpu" -> CPU
                else -> UNKNOWN
            }
    }
}

/**
 * What an exit may show. [BAND] is the common case on a young network: under the live threshold
 * the snapshot carries the exit's load band and nothing else.
 */
enum class ExitDisplayMode {
    OFFLINE,
    BAND,
    LIVE,
}

/** A people count as it may be displayed. Only the fleet total is ever [Exact]. */
sealed interface PeopleCount {
    data class Exact(val count: Int) : PeopleCount

    /** A floor: renders `40+`. */
    data class AtLeast(val count: Int) : PeopleCount

    /** Fewer than [bound]: renders `< 5`. */
    data class Below(val bound: Int) : PeopleCount
}

/**
 * The people on [exit], never exact: a floor to the rounding step while live, and "fewer than
 * the threshold" while not.
 */
fun WarrenNetworkStats.peopleOn(exit: ExitStats): PeopleCount =
    when {
        !exit.live -> PeopleCount.Below(exitLiveThreshold)
        else -> flooredCount(exit.connected)
    }

/**
 * The fleet people count: exact while no exit is live, and floored to the rounding step while one
 * is (the server floors it then, so rendering it plainly would claim a precision it lacks).
 */
fun WarrenNetworkStats.fleetPeople(): PeopleCount =
    if (anyExitLive) flooredCount(users.connected) else PeopleCount.Exact(users.connected)

private fun WarrenNetworkStats.flooredCount(count: Int): PeopleCount =
    if (count < exitUsersRounding) {
        PeopleCount.Below(exitUsersRounding)
    } else {
        PeopleCount.AtLeast(count)
    }

/** What one fetch of the snapshot brought back across the JNI boundary. */
sealed interface NetworkStatsFetch {
    data class Snapshot(val stats: WarrenNetworkStats) : NetworkStatsFetch

    /**
     * The API does not serve a snapshot this build can read (404, or another schema version).
     * Stable for minutes at least, so it is asked again slowly.
     */
    data object Unavailable : NetworkStatsFetch

    /** Anything transient: no answer, another status, a body that could not be read. */
    data object Failed : NetworkStatsFetch
}
