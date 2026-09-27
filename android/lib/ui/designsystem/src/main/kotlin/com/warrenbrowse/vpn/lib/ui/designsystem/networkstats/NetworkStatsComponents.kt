package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.pending
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

// The snapshot changes once per window; the ring glides to the new value over this long.
private const val TWEEN_MILLIS = 800

// The arc starts at 12 o'clock.
private const val TOP_ANGLE = -90f
private const val FULL_TURN = 360f

private val RING_DIAMETER = 12.dp
private val RING_STROKE = 2.dp

/**
 * The colour of a load band. The band is decided server-side so every client paints the same exit
 * the same way; this only maps it onto the palette, and anything unknown stays neutral.
 */
@Composable
fun loadLevelColor(level: LoadLevel, muted: Boolean = false): Color =
    when {
        muted -> MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)
        else ->
            when (level) {
                LoadLevel.LOW -> MaterialTheme.colorScheme.positive
                LoadLevel.MODERATE -> MaterialTheme.colorScheme.warning
                LoadLevel.HIGH -> MaterialTheme.colorScheme.pending
                LoadLevel.SATURATED -> MaterialTheme.colorScheme.error
                LoadLevel.UNKNOWN -> MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)
            }
    }

/**
 * A 12 dp load gauge: an arc of [arcFraction] of a turn in the band colour over a neutral track,
 * starting at 12 o'clock and gliding to each new value. Decorative: the badge around it carries
 * the text and the spoken description.
 */
@Composable
fun LoadRing(
    level: LoadLevel,
    arcFraction: Float,
    modifier: Modifier = Modifier,
    muted: Boolean = false,
) {
    val color by
        animateColorAsState(
            loadLevelColor(level, muted),
            animationSpec = tween(TWEEN_MILLIS / 2),
            label = "load_ring_color",
        )
    val sweep by
        animateFloatAsState(
            targetValue = arcFraction.coerceIn(0f, 1f) * FULL_TURN,
            animationSpec = tween(TWEEN_MILLIS, easing = LinearOutSlowInEasing),
            label = "load_ring_sweep",
        )
    val track = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20)
    Canvas(modifier = modifier.size(RING_DIAMETER)) {
        val strokePx = RING_STROKE.toPx()
        val topLeft = Offset(strokePx / 2, strokePx / 2)
        val arcSize = Size(size.width - strokePx, size.height - strokePx)
        drawArc(
            color = track,
            startAngle = 0f,
            sweepAngle = FULL_TURN,
            useCenter = false,
            topLeft = topLeft,
            size = arcSize,
            style = Stroke(width = strokePx),
        )
        if (sweep > 0f) {
            drawArc(
                color = color,
                startAngle = TOP_ANGLE,
                sweepAngle = sweep,
                useCenter = false,
                topLeft = topLeft,
                size = arcSize,
                style = Stroke(width = strokePx, cap = StrokeCap.Round),
            )
        }
    }
}
