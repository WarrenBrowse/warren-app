package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

private const val FULL_TURN = 360f
private const val PERCENT = 100f

/** Degrees of arc a load ring draws for [percent], clamped to a full turn. */
fun loadRingSweepDegrees(percent: Float): Float =
    percent.coerceIn(0f, PERCENT) / PERCENT * FULL_TURN
