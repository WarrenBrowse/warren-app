package com.warrenbrowse.vpn.app.connect

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.repository.ExitChoice
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.ExitPinResolver
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/**
 * [ExitPinResolver] over `WarrenJni.resolveExitPin` / `resolveAutomaticExit` /
 * `resolveFailoverExit`: the pin and the relay snapshot cross the JNI as JSON,
 * the chosen position comes back. The relay list is re-encoded in the exact
 * `listRelays` schema ([RelayInfo]), so Rust reads back what it projected.
 *
 * The three native calls are injectable so the JVM tests can pin the request
 * bytes and script the answer without the native library; production leaves
 * the defaults, which need neither the logger nor the runtime (pure Rust).
 */
class JniExitPinResolver(
    private val resolveJson: (pinJson: String, relaysJson: String) -> String =
        { pin, relays -> WarrenJni.resolveExitPin(pin, relays) },
    private val automaticJson: (exitCountry: String, relaysJson: String) -> String =
        { exitCountry, relays -> WarrenJni.resolveAutomaticExit(exitCountry, relays) },
    private val failoverJson:
        (pinJson: String, exitCountry: String, relaysJson: String, failedExitPubkeyHex: String) -> String =
        { pin, exitCountry, relays, failed ->
            WarrenJni.resolveFailoverExit(pin, exitCountry, relays, failed)
        },
) : ExitPinResolver {
    override fun resolve(pin: ExitPin, relays: List<WarrenRelaySummary>): ExitChoice =
        answer(relays) { resolveJson(encodePin(pin), encodeRelays(relays)) }

    override fun automatic(exitCountry: String?, relays: List<WarrenRelaySummary>): ExitChoice =
        answer(relays) { automaticJson(exitCountry.orEmpty(), encodeRelays(relays)) }

    override fun failover(
        pin: ExitPin,
        exitCountry: String?,
        relays: List<WarrenRelaySummary>,
        failedExitPubkeyHex: String,
    ): ExitChoice =
        answer(relays) {
            failoverJson(encodePin(pin), exitCountry.orEmpty(), encodeRelays(relays), failedExitPubkeyHex)
        }

    /**
     * A throw and an off-contract answer are [ExitChoice.ResolverFailed], never
     * "nothing in scope": the caller must not widen a scope the rule never
     * evaluated. `{"index":null}` is the rule's own refusal and stays
     * [ExitChoice.NoneInScope].
     */
    private fun answer(relays: List<WarrenRelaySummary>, call: () -> String): ExitChoice {
        val json =
            try {
                call()
            } catch (e: Throwable) {
                Logger.e(throwable = e) { "WarrenJni exit choice threw" }
                return ExitChoice.ResolverFailed
            }
        return when (val index = decodeAnswer(json)) {
            is Answer.Index -> relays.getOrNull(index.value)?.let(ExitChoice::Picked) ?: run {
                Logger.e("exit choice named a position past the snapshot it was sent")
                ExitChoice.ResolverFailed
            }
            Answer.None -> ExitChoice.NoneInScope
            Answer.OffContract -> ExitChoice.ResolverFailed
        }
    }

    /** The three shapes the answer can take, before it is read against a snapshot. */
    private sealed interface Answer {
        data class Index(val value: Int) : Answer

        data object None : Answer

        data object OffContract : Answer
    }

    private fun decodeAnswer(json: String): Answer =
        try {
            when (val index = Json.parseToJsonElement(json).jsonObject["index"]) {
                null -> Answer.OffContract
                JsonNull -> Answer.None
                else -> index.jsonPrimitive.intOrNull?.let(Answer::Index) ?: Answer.OffContract
            }
        } catch (e: Exception) {
            Logger.e(throwable = e) { "exit choice answer is not the JNI contract" }
            Answer.OffContract
        }

    companion object {
        /** The pin as Rust's `ExitPinSpec` reads it: a `kind` tag plus its fields. */
        fun encodePin(pin: ExitPin): String =
            buildJsonObject {
                when (pin) {
                    ExitPin.Automatic -> put("kind", "automatic")
                    is ExitPin.Country -> {
                        put("kind", "country")
                        put("country", pin.country)
                    }
                    is ExitPin.City -> {
                        put("kind", "city")
                        put("country", pin.country)
                        put("city", pin.city)
                    }
                    is ExitPin.Exit -> {
                        put("kind", "exit")
                        put("exit_id", pin.exitId)
                    }
                }
            }.toString()

        /** The snapshot in the `listRelays` schema, the shape Rust projected it in. */
        fun encodeRelays(relays: List<WarrenRelaySummary>): String =
            Json.encodeToString(relays.map { it.toRelayInfo() })

        /**
         * The position Rust answered, or `null` for `{"index":null}` and for anything
         * that is not the contract (a native library and a decoder from two revisions).
         */
        fun decodeIndex(json: String): Int? =
            try {
                Json.parseToJsonElement(json).jsonObject["index"]?.jsonPrimitive?.intOrNull
            } catch (e: Exception) {
                Logger.e(throwable = e) { "exit choice answer is not the JNI contract" }
                null
            }
    }
}
