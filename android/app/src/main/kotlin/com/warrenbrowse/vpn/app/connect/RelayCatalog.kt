package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import kotlinx.serialization.json.Json

/**
 * Single point of access to the Warren relay catalogue.
 *
 * Today the catalogue is sourced from
 * [com.warrenbrowse.vpn.jni.WarrenJni.listRelays], which currently
 * returns a hardcoded list (one entry: warren-exit-1 prod). D.6 will
 * swap the JNI implementation for a real signed-relay-list fetch via
 * warren-api-client; the Kotlin shape stays unchanged.
 *
 * Held as a Koin `single` so future consumers (location picker,
 * relay selector, connect button) can share the same instance and
 * eventually a single in-memory cache + refresh strategy.
 */
class RelayCatalog {
    private val json = Json { ignoreUnknownKeys = true }

    /**
     * Fetch the current relay list. Synchronous because the underlying
     * JNI call is cheap (in-memory) today; once the signed-fetch path
     * lands the API can move to suspend without callers having to
     * change.
     */
    fun listRelays(): List<RelayInfo> {
        val raw = try {
            WarrenJni.listRelays()
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
