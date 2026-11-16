package com.warrenbrowse.vpn.app.connect

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Kotlin counterpart of the JSON object returned by
 * [com.warrenbrowse.vpn.jni.WarrenJni.listRelays]. Shape lives here in
 * the `app` module rather than in `lib/model` because the consumer (the
 * location-picker UI) is itself in `app`/feature land; promoting it
 * to a lib module makes sense once a second consumer appears.
 *
 * Field names mirror the Rust-side JSON keys exactly (snake_case) so we
 * avoid hand-rolling a SerialName mapping for every field.
 */
@Serializable
data class RelayInfo(
    @SerialName("exit_id") val exitId: String,
    @SerialName("exit_pubkey_hex") val exitPubkeyHex: String,
    val endpoint: String,
    val country: String,
    val city: String,
    val active: Boolean,
    val weight: Long,
    // The fleet name the operator knows the exit by (`fi-hel-bveh1`), null for
    // a node that has not reported every naming component and for rows cached
    // before the v11 list carried it.
    val name: String? = null,
)
