package com.warrenbrowse.vpn.lib.repository

private const val KEY_LABEL_HEX_CHARS = 16

/**
 * How an exit is named on screen: the fleet name the operator knows it by (`fi-hel-bveh1`), or,
 * for a node that has not reported every naming component, `warren-` and the first 16 hex
 * characters of its key. The desktop daemon's `exit_label`
 * (`mullvad-daemon/src/warren_relay_list_view.rs`) applies the same rule, so both clients name one
 * exit the same way.
 */
val WarrenRelaySummary.label: String
    get() = name ?: "warren-${exitPubkeyHex.take(KEY_LABEL_HEX_CHARS)}"
