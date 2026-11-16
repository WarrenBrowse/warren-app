package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRingPalette
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces

/** The fill of the status pill and the colour of its label and eye. */
data class HomePillColors(val background: Color, val text: Color)

/**
 * The pill carries the phase in its fill (desktop `HomeStatus` pillColors): an interrupted tunnel
 * is pending like one coming up, since nothing flows in either, and the kill switch stays neutral.
 */
fun ConnectionPhase.pillColors(surfaces: WarrenSurfaces): HomePillColors =
    when (this) {
        ConnectionPhase.Exposed ->
            HomePillColors(surfaces.homePillExposed, surfaces.homePillExposedText)
        ConnectionPhase.Connecting,
        ConnectionPhase.Interrupted ->
            HomePillColors(surfaces.homePillPending, surfaces.homePillPendingText)
        ConnectionPhase.Protected ->
            HomePillColors(surfaces.homePillProtected, surfaces.homePillProtectedText)
        ConnectionPhase.Blocked ->
            HomePillColors(surfaces.homePillBlocked, surfaces.homePillBlockedText)
    }

/** The exit row's load ring, in the home load colours (desktop HomeStatus). */
fun homeLoadRingPalette(surfaces: WarrenSurfaces): LoadRingPalette =
    LoadRingPalette(
        low = surfaces.homeLoad,
        moderate = surfaces.homeBadge,
        high = surfaces.homePillPending,
        saturated = surfaces.homePillExposed,
        track = surfaces.homeLoadTrack,
    )
