package com.warrenbrowse.vpn.feature.splittunneling.impl

import com.warrenbrowse.vpn.lib.model.SplitTunnelMode

/**
 * The tabs of the screen, in the desktop's order. Bypass and IncludeOnly are each the switch and
 * the list of one split mode; CountryPerApp composes with both and has no [mode], its switch being
 * the one of the per-app countries.
 */
enum class SplitTunnelingTab(val mode: SplitTunnelMode?) {
    Bypass(SplitTunnelMode.Exclude),
    CountryPerApp(null),
    IncludeOnly(SplitTunnelMode.IncludeOnly),
}

/**
 * What the user accepts before a split mode change: that the rest of the
 * device leaves the VPN, that the other mode stops, or both.
 */
data class ModeChangeConfirmation(
    val leavesDeviceUnprotected: Boolean,
    val replaces: SplitTunnelMode?,
)

/**
 * The confirmation a move from [current] to [next] needs, or null when it
 * applies at once. Turning a mode off never asks: it only ever puts more of the
 * device back in the VPN.
 */
fun modeChangeConfirmation(
    current: SplitTunnelMode,
    next: SplitTunnelMode,
): ModeChangeConfirmation? {
    val replaces = current.takeUnless { it == SplitTunnelMode.Off }
    val leavesDeviceUnprotected = next == SplitTunnelMode.IncludeOnly
    return when {
        next == SplitTunnelMode.Off || next == current -> null
        !leavesDeviceUnprotected && replaces == null -> null
        else -> ModeChangeConfirmation(leavesDeviceUnprotected, replaces)
    }
}
