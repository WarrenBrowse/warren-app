package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.runtime.Immutable
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.TunnelState

/**
 * Declared immutable so the home screen can skip.
 *
 * Every field is deeply immutable in practice, but the compiler cannot see it:
 * [TunnelState.Connecting] / [TunnelState.Connected] hold a
 * `kotlin.collections.List` of feature indicators, which is unstable by
 * inference, and that alone makes the whole screen non-skippable. Under strong
 * skipping an unstable parameter is compared by identity, so `Connect` would
 * drag `SceneryBackdrop` (three painter lookups, two full-screen brushes)
 * through a full recomposition on every expiry tick and every network-info
 * emission. The promise this annotation makes is real: nothing ever mutates a
 * published state or the list inside it; a new state is always a new instance.
 */
@Immutable
data class ConnectUiState(
    val location: GeoIpLocation?,
    val selectedRelayItemTitle: String?,
    val tunnelState: TunnelState,
    val inAppNotification: InAppNotification?,
    val isPlayBuild: Boolean,
    // Debounced host-offline verdict; degrades the Connected presentation
    // ("Connection interrupted") because the tunnel can hold Connected
    // through its transparent redial window while nothing flows.
    val hostOffline: Boolean = false,
    // Automatic recoveries since process start (native redials + retry-loop
    // successes); shown as the "Reconnections" connection-details row.
    val autoRecoveryCount: Int = 0,
) {

    companion object {
        val INITIAL =
            ConnectUiState(
                location = null,
                selectedRelayItemTitle = null,
                tunnelState = TunnelState.Disconnected(),
                inAppNotification = null,
                isPlayBuild = false,
                hostOffline = false,
                autoRecoveryCount = 0,
            )
    }
}
