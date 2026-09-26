package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

import androidx.compose.ui.geometry.Offset

private const val FULL_TURN = 360f
private const val PERCENT = 100f

/** Degrees of arc a load ring draws for [percent], clamped to a full turn. */
fun loadRingSweepDegrees(percent: Float): Float =
    percent.coerceIn(0f, PERCENT) / PERCENT * FULL_TURN

/**
 * The points of a sparkline of [values] in a [width] x [height] box, inset by [inset] on every side
 * so the stroke and the emphasised last point stay inside.
 *
 * The scale runs from zero to the largest value, so a flat chart means a steady figure rather than
 * a zoomed-in wobble. A single value draws a flat line across the box.
 */
fun sparklinePoints(values: List<Long>, width: Float, height: Float, inset: Float): List<Offset> {
    if (values.isEmpty()) return emptyList()
    val series = if (values.size == 1) values + values else values
    val max = series.max().coerceAtLeast(0)
    val left = inset
    val span = width - 2 * inset
    val bottom = height - inset
    val rise = height - 2 * inset
    return series.mapIndexed { index, value ->
        val x = left + span * index / series.lastIndex
        val y = if (max == 0L) bottom else bottom - rise * value.coerceAtLeast(0) / max
        Offset(x, y)
    }
}
