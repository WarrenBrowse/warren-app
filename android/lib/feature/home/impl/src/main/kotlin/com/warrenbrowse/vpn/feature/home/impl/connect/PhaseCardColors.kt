package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRingPalette
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces

/** The colour a phase writes its status title and eye with, and the fill of the well behind it. */
data class PhaseCardColors(val title: Color, val well: Color)

/**
 * The card is a surface of its own, light or dark with the theme, so it writes the phase in the
 * surface palette rather than in the scenery accents: the title carries the hue and the well
 * behind the eye is a quiet fill of it (desktop `getPhaseCardColors`).
 */
fun ConnectionPhase.cardColors(surfaces: WarrenSurfaces): PhaseCardColors =
    when (this) {
        ConnectionPhase.Exposed -> PhaseCardColors(surfaces.exposed, surfaces.exposedWell)
        ConnectionPhase.Connecting,
        ConnectionPhase.Interrupted -> PhaseCardColors(surfaces.connecting, surfaces.connectingWell)
        ConnectionPhase.Protected -> PhaseCardColors(surfaces.protected, surfaces.protectedWell)
        ConnectionPhase.Blocked -> PhaseCardColors(surfaces.text, surfaces.button)
    }

private const val LOAD_RING_TRACK_ALPHA = 0.3f

/** The hostname line's load ring, in the phase colours of the card (desktop Hostname). */
fun cardLoadRingPalette(surfaces: WarrenSurfaces): LoadRingPalette =
    LoadRingPalette(
        low = surfaces.protected,
        moderate = surfaces.pill,
        high = surfaces.connecting,
        saturated = surfaces.exposed,
        track = surfaces.textMuted.copy(alpha = LOAD_RING_TRACK_ALPHA),
    )
