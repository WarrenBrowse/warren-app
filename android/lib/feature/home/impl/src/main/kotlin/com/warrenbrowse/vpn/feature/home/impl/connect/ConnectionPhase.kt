package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.DrawableRes
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.color.pending
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

/**
 * The five presentation phases of the connection, mirroring the desktop
 * `connection-phase.ts`. One source of truth drives the scenery backdrop, the
 * status eye, the status copy and the accent color together, so they can never
 * drift apart across states.
 */
enum class ConnectionPhase {
    Exposed,
    Connecting,
    Protected,
    Interrupted,
    Blocked,
}

fun TunnelState.connectionPhase(hostOffline: Boolean = false): ConnectionPhase =
    when (this) {
        // Connected with the host offline is a real window (the native session
        // holds Connected through its transparent redial): presenting a calm
        // green "protected" there would be a lie.
        is TunnelState.Connected ->
            if (hostOffline) ConnectionPhase.Interrupted else ConnectionPhase.Protected
        is TunnelState.Connecting -> ConnectionPhase.Connecting
        is TunnelState.Disconnected -> ConnectionPhase.Exposed
        // Desktop treats a tearing-down tunnel as transitional connecting-orange
        // for every sub-state, including a kill-switch-held teardown: a distinct
        // Blocked wash mid-disconnect (the old Android behavior) is a flash the
        // desktop never shows. Android's TunnelState.Disconnected carries no
        // lockedDown flag, so the desktop "disconnected + lockedDown -> blocked"
        // state cannot be represented here and always reads as Exposed.
        is TunnelState.Disconnecting -> ConnectionPhase.Connecting
        is TunnelState.Error ->
            if (errorState.isBlocking) ConnectionPhase.Blocked else ConnectionPhase.Exposed
    }

/** Whether the status eye is open (the user is visible to the network). */
fun ConnectionPhase.isEyeOpen(): Boolean =
    this == ConnectionPhase.Exposed || this == ConnectionPhase.Connecting

@Composable
fun ConnectionPhase.accentColor(): Color =
    when (this) {
        ConnectionPhase.Protected -> MaterialTheme.colorScheme.positive
        ConnectionPhase.Connecting,
        ConnectionPhase.Interrupted -> MaterialTheme.colorScheme.pending
        ConnectionPhase.Exposed -> MaterialTheme.colorScheme.error
        ConnectionPhase.Blocked -> MaterialTheme.colorScheme.onSurface
    }

/** Which layers the scenery backdrop shows for a phase (desktop `resolveScenery`). */
data class SceneryState(
    @param:DrawableRes val landscape: Int,
    val showBula: Boolean,
    val blurred: Boolean,
)

// Only these exits have bespoke art; every other country falls back to the
// plaine, same as desktop and iOS.
@DrawableRes
private fun countryLandscape(exitCountry: String?): Int =
    when (exitCountry?.trim()?.lowercase()) {
        "de", "germany" -> R.drawable.scenery_germany
        "fi", "finland" -> R.drawable.scenery_finland
        "nl", "netherlands" -> R.drawable.scenery_netherlands
        "sg", "singapore" -> R.drawable.scenery_singapore
        else -> R.drawable.scenery_plaine
    }

// Without a tunnel the backdrop is the watched plain, so an unprotected screen
// shows what unprotected means, and the country art is reserved for the states
// where traffic really goes there.
fun resolveScenery(phase: ConnectionPhase, exitCountry: String?): SceneryState =
    when (phase) {
        ConnectionPhase.Exposed ->
            SceneryState(R.drawable.scenery_plaine, showBula = true, blurred = false)
        ConnectionPhase.Connecting ->
            SceneryState(countryLandscape(exitCountry), showBula = true, blurred = true)
        ConnectionPhase.Protected ->
            SceneryState(countryLandscape(exitCountry), showBula = false, blurred = false)
        ConnectionPhase.Interrupted ->
            SceneryState(countryLandscape(exitCountry), showBula = false, blurred = true)
        // Kill switch: the rabbit is tucked in, so the watched world outside is
        // only seen through the blur, never sharp like the exposed state.
        ConnectionPhase.Blocked ->
            SceneryState(R.drawable.scenery_plaine, showBula = false, blurred = true)
    }
