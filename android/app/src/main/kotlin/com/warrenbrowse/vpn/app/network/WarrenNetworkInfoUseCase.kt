package com.warrenbrowse.vpn.app.network

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfo
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

/**
 * Process-wide `GET /v1/network` feed: fetches the public environment
 * descriptor once at startup and then on a slow cadence, with a capped
 * failure backoff. The value rarely changes (an ops-side flip of the
 * beta cap), so a slow poll is enough; failures leave the last-good
 * value in place.
 */
class WarrenNetworkInfoUseCase(
    private val bridge: WarrenJniBridge,
    scope: CoroutineScope,
) : WarrenNetworkInfoProvider {

    private val _networkInfo = MutableStateFlow<WarrenNetworkInfo?>(null)
    override val networkInfo: StateFlow<WarrenNetworkInfo?> = _networkInfo

    init {
        scope.launch {
            var retry = INITIAL_RETRY
            while (true) {
                val info = withContext(Dispatchers.IO) { fetchOnce() }
                if (info != null) {
                    _networkInfo.value = info
                    retry = INITIAL_RETRY
                    delay(REFRESH_INTERVAL)
                } else {
                    delay(retry)
                    retry = (retry * 2).coerceAtMost(MAX_RETRY)
                }
            }
        }
    }

    private fun fetchOnce(): WarrenNetworkInfo? =
        try {
            val obj = Json.parseToJsonElement(bridge.fetchNetworkInfo()).jsonObject
            if (obj["ok"]?.jsonPrimitive?.boolean == true) {
                WarrenNetworkInfo(
                    environment = obj["environment"]?.jsonPrimitive?.content.orEmpty(),
                    degraded = obj["degraded"]?.jsonPrimitive?.boolean == true,
                    defaultRateBps = obj["default_rate_bps"]?.jsonPrimitive?.longOrNull,
                    paymentsEnabled = obj["payments_enabled"]?.jsonPrimitive?.boolean == true,
                )
            } else {
                null
            }
        } catch (e: Exception) {
            Logger.d(throwable = e) { "network-info fetch failed (will retry)" }
            null
        }

    private companion object {
        val REFRESH_INTERVAL = 30.minutes
        val INITIAL_RETRY = 15.seconds
        val MAX_RETRY = 10.minutes
    }
}
