package com.warrenbrowse.vpn.feature.splittunneling.impl

import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode

data class Loading(val isModal: Boolean = false)

data class SplitTunnelingUiState(
    val splitMode: SplitTunnelMode = SplitTunnelMode.Off,
    val tab: SplitTunnelingTab = SplitTunnelingTab.Bypass,
    /** The apps on the list of [tab]. */
    val selectedApps: List<AppData> = emptyList(),
    val otherApps: List<AppData> = emptyList(),
    val showSystemApps: Boolean = false,
    val isModal: Boolean = false,
    /** A mode change waiting for the user's answer. */
    val confirmation: ModeChangeConfirmation? = null,
) {
    /** Whether the mode of the shown tab is the one in force. */
    val tabModeOn: Boolean
        get() = splitMode == tab.mode

    /**
     * Include-only is on and none of its apps is on this device, so the tunnel
     * carries every app until one is chosen.
     */
    val includeOnlyWithoutApps: Boolean
        get() =
            splitMode == SplitTunnelMode.IncludeOnly &&
                tab == SplitTunnelingTab.IncludeOnly &&
                selectedApps.isEmpty()
}
