package com.warrenbrowse.vpn.lib.model

/**
 * What the compact load badge of one exit shows: a small ring, the percentage, the people and the
 * download rate. Pure, so every rule of what appears when is decided here and tested off-device.
 */
sealed interface ExitLoadBadge {
    /** A grey empty ring and the word "Offline". */
    data object Offline : ExitLoadBadge

    data class Shown(
        /** The server's band, which colours the ring. */
        val level: LoadLevel,
        /** Share of a full turn the ring's arc covers, in `0..1`. */
        val arcFraction: Float,
        /** The percentage text, present only while the exit is live. */
        val percent: Int?,
        val people: PeopleCount,
        /** Download bits per second, present only while the exit is live. */
        val downloadBps: Long?,
    ) : ExitLoadBadge
}

private const val PERCENT = 100f
private const val LOW_ARC = 0.25f
private const val MODERATE_ARC = 0.6f
private const val HIGH_ARC = 0.85f
private const val SATURATED_ARC = 1f

/**
 * The arc a band draws when no percentage may be shown. Fixed per band, so the ring's shape carries
 * the level and colour is never its only signal.
 */
fun LoadLevel.bandArcFraction(): Float =
    when (this) {
        LoadLevel.LOW -> LOW_ARC
        LoadLevel.MODERATE -> MODERATE_ARC
        LoadLevel.HIGH -> HIGH_ARC
        LoadLevel.SATURATED -> SATURATED_ARC
        LoadLevel.UNKNOWN -> 0f
    }

/**
 * The badge of [exit]. Live: the load percentage as the arc and as text, a people floor, the
 * download rate. Quiet (under the live threshold): the band's fixed arc and "fewer than the
 * threshold" people, nothing else, whatever figures the exit happens to carry. Offline: nothing but
 * that.
 */
fun WarrenNetworkStats.loadBadgeOf(exit: ExitStats): ExitLoadBadge =
    when (exit.displayMode) {
        ExitDisplayMode.OFFLINE -> ExitLoadBadge.Offline
        ExitDisplayMode.BAND ->
            ExitLoadBadge.Shown(
                level = exit.loadLevel,
                arcFraction = exit.loadLevel.bandArcFraction(),
                percent = null,
                people = peopleOn(exit),
                downloadBps = null,
            )
        ExitDisplayMode.LIVE ->
            ExitLoadBadge.Shown(
                level = exit.loadLevel,
                arcFraction =
                    exit.loadPercent?.let { (it / PERCENT).coerceIn(0f, 1f) }
                        ?: exit.loadLevel.bandArcFraction(),
                percent = exit.loadPercent,
                people = peopleOn(exit),
                downloadBps = exit.downloadBps,
            )
    }
