package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.DrawableRes
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.color.pending
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.positiveText
import com.warrenbrowse.vpn.lib.ui.theme.color.pendingText
import com.warrenbrowse.vpn.lib.ui.theme.color.errorText

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

/**
 * The colour of the status TITLE, as opposed to the fills: the lifted tints
 * built for 4.5:1 on the card at title size (desktop
 * `getPhaseTitleColorName`), white for the neutral blocked phase.
 */
@Composable
fun ConnectionPhase.titleColor(): Color =
    when (this) {
        ConnectionPhase.Protected -> MaterialTheme.colorScheme.positiveText
        ConnectionPhase.Connecting,
        ConnectionPhase.Interrupted -> MaterialTheme.colorScheme.pendingText
        ConnectionPhase.Exposed -> MaterialTheme.colorScheme.errorText
        ConnectionPhase.Blocked -> MaterialTheme.colorScheme.onSurface
    }

/** Which layers the scenery backdrop shows for a phase (desktop `resolveScenery`). */
data class SceneryState(
    @param:DrawableRes val landscape: Int,
    val showBula: Boolean,
    val blurred: Boolean,
)

// BEGIN GENERATED scenery table: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.

/** Which landscape a phase shows (the exit country, or the plain), and the other two layers. */
internal data class SceneryRow(
    val countryLandscape: Boolean,
    val showBula: Boolean,
    val blurred: Boolean,
)

internal object SceneryTable {
    @DrawableRes val plain: Int = R.drawable.scenery_plaine
    @DrawableRes val burrow: Int = R.drawable.scenery_terrier
    @DrawableRes val bula: Int = R.drawable.scenery_bula

    /** Keyed by the lower-case ISO code and the lower-case English relay-list name. */
    val countries: Map<String, Int> =
        mapOf(
            "de" to R.drawable.scenery_germany,
            "germany" to R.drawable.scenery_germany,
            "fi" to R.drawable.scenery_finland,
            "finland" to R.drawable.scenery_finland,
            "nl" to R.drawable.scenery_netherlands,
            "netherlands" to R.drawable.scenery_netherlands,
            "sg" to R.drawable.scenery_singapore,
            "singapore" to R.drawable.scenery_singapore,
        )

    fun row(phase: ConnectionPhase): SceneryRow =
        when (phase) {
            ConnectionPhase.Exposed ->
                SceneryRow(countryLandscape = false, showBula = true, blurred = false)
            ConnectionPhase.Connecting ->
                SceneryRow(countryLandscape = true, showBula = true, blurred = true)
            ConnectionPhase.Protected ->
                SceneryRow(countryLandscape = true, showBula = false, blurred = false)
            ConnectionPhase.Interrupted ->
                SceneryRow(countryLandscape = true, showBula = false, blurred = true)
            ConnectionPhase.Blocked ->
                SceneryRow(countryLandscape = false, showBula = false, blurred = true)
        }
}

// END GENERATED scenery table

// Only the countries in the table have bespoke art; every other one falls back
// to the plaine, same as desktop and iOS.
@DrawableRes
internal fun countryLandscape(exitCountry: String?): Int =
    SceneryTable.countries[exitCountry?.trim()?.lowercase()] ?: SceneryTable.plain

// Without a tunnel the backdrop is the watched plain, so an unprotected screen
// shows what unprotected means, and the country art is reserved for the states
// where traffic really goes there. The rows come from scenery.json.
fun resolveScenery(phase: ConnectionPhase, exitCountry: String?): SceneryState {
    val row = SceneryTable.row(phase)
    val landscape = if (row.countryLandscape) countryLandscape(exitCountry) else SceneryTable.plain
    return SceneryState(landscape, showBula = row.showBula, blurred = row.blurred)
}

/**
 * The masters the first home frame draws whatever the phase: the watched
 * plain behind an exposed screen, the burrow and Bula. Warmed before the
 * Connect screen is reached so that frame decodes nothing on the main thread.
 */
internal fun firstFrameMasters(): List<Int> =
    listOf(SceneryTable.plain, SceneryTable.burrow, SceneryTable.bula)
