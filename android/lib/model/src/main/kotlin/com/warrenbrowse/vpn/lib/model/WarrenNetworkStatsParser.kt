package com.warrenbrowse.vpn.lib.model

import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull

/** Tolerant reader of the network stats snapshot and of its JNI envelope. */
object WarrenNetworkStatsParser {
    private const val SUPPORTED_VERSION = 1L
    private const val MAX_PERCENT = 100
    private val EXIT_ID = Regex("^[0-9a-fA-F]{32}$")

    /**
     * The `{"ok":true,"stats":{..}}` / `{"ok":false,"reason":".."}` envelope of
     * `WarrenJni.fetchNetworkStats`.
     */
    fun parseEnvelope(raw: String): NetworkStatsFetch {
        val root = parseObject(raw) ?: return NetworkStatsFetch.Failed
        if (root.bool("ok") == true) {
            val stats = (root["stats"] as? JsonObject)?.let(::parseOrNull)
            return stats?.let(NetworkStatsFetch::Snapshot) ?: NetworkStatsFetch.Failed
        }
        return when (root.string("reason")) {
            REASON_UNAVAILABLE,
            REASON_VERSION -> NetworkStatsFetch.Unavailable
            else -> NetworkStatsFetch.Failed
        }
    }

    /** The snapshot in [json], or null when it is not one this app can read. */
    fun parse(json: String): WarrenNetworkStats? = parseObject(json)?.let(::parseOrNull)

    private const val REASON_UNAVAILABLE = "unavailable"
    private const val REASON_VERSION = "version"

    private fun parseObject(raw: String): JsonObject? =
        try {
            Json.parseToJsonElement(raw) as? JsonObject
        } catch (_: SerializationException) {
            null
        }

    private fun parseOrNull(root: JsonObject): WarrenNetworkStats? =
        try {
            parseSnapshot(root)
        } catch (_: Malformed) {
            null
        }

    /** A required figure is missing or out of range. */
    private class Malformed : Exception()

    private fun parseSnapshot(root: JsonObject): WarrenNetworkStats {
        if (root.long("version") != SUPPORTED_VERSION) throw Malformed()
        val windowSecs = root.requiredInt("window_secs")
        if (windowSecs == 0) throw Malformed()
        val users = root.requiredObject("users")
        val fleet = root.requiredObject("fleet")
        return WarrenNetworkStats(
            environment = root.string("environment").orEmpty(),
            generatedAt = root.requiredLong("generated_at"),
            windowSecs = windowSecs,
            exitUsersRounding = root.requiredInt("exit_users_rounding").coerceAtLeast(1),
            exitLiveThreshold = root.requiredInt("exit_live_threshold"),
            users =
                NetworkUsers(
                    accountsTotal = users.requiredLong("accounts_total"),
                    subscribersActive = users.requiredLong("subscribers_active"),
                    connected = users.requiredInt("connected"),
                ),
            fleet =
                FleetStats(
                    exitsOnline = fleet.requiredInt("exits_online"),
                    exitsTotal = fleet.requiredInt("exits_total"),
                    downloadBps = fleet.requiredLong("download_bps"),
                    uploadBps = fleet.requiredLong("upload_bps"),
                    capacityBps = fleet.requiredLong("capacity_bps"),
                    loadPercent = fleet.requiredInt("load_percent").coerceAtMost(MAX_PERCENT),
                    loadLevel = LoadLevel.of(fleet.string("load_level")),
                    transferred24hBytes = fleet.requiredLong("transferred_24h_bytes"),
                    peakConnected24h = fleet.requiredInt("peak_connected_24h"),
                    peakThroughput24hBps = fleet.requiredLong("peak_throughput_24h_bps"),
                ),
            exits = root.requiredArray("exits").entries(::parseExit),
            history =
                root.requiredArray("history").entries { point ->
                    FleetHistoryPoint(
                        t = point.requiredLong("t"),
                        connected = point.requiredInt("connected"),
                        throughputBps = point.requiredLong("throughput_bps"),
                    )
                },
        )
    }

    private fun parseExit(exit: JsonObject): ExitStats {
        val exitId = exit.string("exit_id")?.takeIf(EXIT_ID::matches) ?: throw Malformed()
        return ExitStats(
            exitId = exitId.lowercase(),
            name = exit.string("name")?.takeIf { it.isNotEmpty() },
            country = exit.string("country") ?: throw Malformed(),
            city = exit.string("city") ?: throw Malformed(),
            online = exit.bool("online") == true,
            live = exit.bool("live") == true,
            connected = exit.int("connected") ?: 0,
            downloadBps = exit.long("download_bps") ?: 0,
            uploadBps = exit.long("upload_bps") ?: 0,
            capacityBps = exit.long("capacity_bps"),
            loadPercent = exit.percent("load_percent"),
            loadLevel = LoadLevel.of(exit.string("load_level")),
            loadDriver = LoadDriver.of(exit.string("load_driver")),
            cpuPercent = exit.percent("cpu_percent"),
            history =
                (exit["history"] as? JsonArray)?.entries { point ->
                    ExitHistoryPoint(
                        t = point.requiredLong("t"),
                        connected = point.requiredInt("connected"),
                        throughputBps = point.requiredLong("throughput_bps"),
                        loadPercent = point.percent("load_percent"),
                    )
                } ?: emptyList(),
        )
    }

    /** One bad entry costs that entry, never the whole list. */
    private fun <T> JsonArray.entries(parse: (JsonObject) -> T): List<T> = mapNotNull { entry ->
        (entry as? JsonObject)?.let {
            try {
                parse(it)
            } catch (_: Malformed) {
                null
            }
        }
    }

    private fun JsonObject.primitive(key: String): JsonPrimitive? = get(key) as? JsonPrimitive

    private fun JsonObject.string(key: String): String? =
        primitive(key)?.takeIf { it.isString }?.content

    private fun JsonObject.bool(key: String): Boolean? =
        primitive(key)?.takeIf { !it.isString }?.booleanOrNull

    /** A non-negative whole number; a fraction is truncated, anything else is absent. */
    private fun JsonObject.long(key: String): Long? {
        val value = primitive(key)?.takeIf { !it.isString } ?: return null
        val number = value.longOrNull ?: value.doubleOrNull?.takeIf { it.isFinite() }?.toLong()
        return number?.takeIf { it >= 0 }
    }

    private fun JsonObject.int(key: String): Int? =
        long(key)?.coerceAtMost(Int.MAX_VALUE.toLong())?.toInt()

    private fun JsonObject.percent(key: String): Int? = int(key)?.coerceAtMost(MAX_PERCENT)

    private fun JsonObject.requiredLong(key: String): Long = long(key) ?: throw Malformed()

    private fun JsonObject.requiredInt(key: String): Int = int(key) ?: throw Malformed()

    private fun JsonObject.requiredObject(key: String): JsonObject =
        get(key) as? JsonObject ?: throw Malformed()

    private fun JsonObject.requiredArray(key: String): JsonArray =
        get(key) as? JsonArray ?: throw Malformed()
}
