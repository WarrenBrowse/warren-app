package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.DrawableRes
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R

/**
 * The five presentation phases of the connection, mirroring the desktop
 * `connection-phase.ts`. One source of truth drives the scenery hero, the
 * status pill, its eye and the status copy together, so they can never
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

// BEGIN GENERATED phase colour table: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.

/** The colour tokens scenery.json paints a phase with; the theme maps each one once. */
internal enum class SceneryTone {
    Green,
    GreenText,
    Orange,
    OrangeText,
    Red,
    RedText,
    White,
}

/** The saturated accent a phase fills its eye well, rail and buttons with. */
internal fun ConnectionPhase.accentTone(): SceneryTone =
    when (this) {
        ConnectionPhase.Exposed -> SceneryTone.Red
        ConnectionPhase.Connecting -> SceneryTone.Orange
        ConnectionPhase.Protected -> SceneryTone.Green
        ConnectionPhase.Interrupted -> SceneryTone.Orange
        ConnectionPhase.Blocked -> SceneryTone.White
    }

/** The lifted tint a phase writes its title with. */
internal fun ConnectionPhase.titleTone(): SceneryTone =
    when (this) {
        ConnectionPhase.Exposed -> SceneryTone.RedText
        ConnectionPhase.Connecting -> SceneryTone.OrangeText
        ConnectionPhase.Protected -> SceneryTone.GreenText
        ConnectionPhase.Interrupted -> SceneryTone.OrangeText
        ConnectionPhase.Blocked -> SceneryTone.White
    }

// END GENERATED phase colour table

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
    /** Frames the skyline down to the burrow (sky) rather than the cameras and Bula (ground). */
    val skyFraming: Boolean,
)

/** How a landscape is framed in the home hero (fixtures/client-rules/scenery_framing.json). */
internal data class SceneryLandscape(
    val skylineTopRow: Int,
    /** ARGB of the canvas top rows, for the strip above a canvas that had to drop. */
    val topFill: Long,
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
            "fr" to R.drawable.scenery_france,
            "france" to R.drawable.scenery_france,
            "nl" to R.drawable.scenery_netherlands,
            "netherlands" to R.drawable.scenery_netherlands,
            "ro" to R.drawable.scenery_romania,
            "romania" to R.drawable.scenery_romania,
            "sg" to R.drawable.scenery_singapore,
            "singapore" to R.drawable.scenery_singapore,
        )

    /** Keyed by the drawable of the landscape. */
    val landscapes: Map<Int, SceneryLandscape> =
        mapOf(
            R.drawable.scenery_plaine to SceneryLandscape(320, 0xFFD5DEE7),
            R.drawable.scenery_germany to SceneryLandscape(140, 0xFFF4F3F2),
            R.drawable.scenery_finland to SceneryLandscape(235, 0xFFF9F5F0),
            R.drawable.scenery_france to SceneryLandscape(160, 0xFFF1F2F3),
            R.drawable.scenery_netherlands to SceneryLandscape(100, 0xFFEFF2F3),
            R.drawable.scenery_romania to SceneryLandscape(300, 0xFFF8F7F6),
            R.drawable.scenery_singapore to SceneryLandscape(360, 0xFFFCF7F1),
        )

    fun row(phase: ConnectionPhase): SceneryRow =
        when (phase) {
            ConnectionPhase.Exposed ->
                SceneryRow(countryLandscape = false, showBula = true, blurred = false, skyFraming = false)
            ConnectionPhase.Connecting ->
                SceneryRow(countryLandscape = true, showBula = true, blurred = true, skyFraming = false)
            ConnectionPhase.Protected ->
                SceneryRow(countryLandscape = true, showBula = false, blurred = false, skyFraming = true)
            ConnectionPhase.Interrupted ->
                SceneryRow(countryLandscape = true, showBula = false, blurred = true, skyFraming = true)
            ConnectionPhase.Blocked ->
                SceneryRow(countryLandscape = false, showBula = false, blurred = true, skyFraming = false)
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
