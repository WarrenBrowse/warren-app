package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80
import com.warrenbrowse.vpn.lib.ui.theme.color.pending
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

// The snapshot changes once per window; the ring glides to the new value over this long.
private const val TWEEN_MILLIS = 800

// Tint of the disc a band-only ring encloses: a hint of the band's colour, so the closed ring
// reads as a state rather than as a full gauge.
private const val BAND_FILL_ALPHA = 0.07f

// The arc starts at 12 o'clock.
private const val TOP_ANGLE = -90f

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
 * A circular load gauge. With a [percent], an arc of that share of a turn over a neutral track,
 * starting at 12 o'clock and gliding to each new value. Without one (an exit below the live
 * threshold), the band alone: a full thin circle in the band colour, no arc, no figure.
 *
 * Colour is never the only signal: the caller always shows the percentage or the band name as text,
 * and passes [contentDescription] when the ring stands alone.
 */
@Composable
fun LoadRing(
    level: LoadLevel,
    percent: Int?,
    size: LoadRingSize,
    modifier: Modifier = Modifier,
    muted: Boolean = false,
    contentDescription: String? = null,
) {
    val color by
        animateColorAsState(
            loadLevelColor(level, muted),
            animationSpec = tween(TWEEN_MILLIS / 2),
            label = "load_ring_color",
        )
    val sweep by
        animateFloatAsState(
            targetValue = loadRingSweepDegrees(percent?.toFloat() ?: 0f),
            animationSpec = tween(TWEEN_MILLIS, easing = LinearOutSlowInEasing),
            label = "load_ring_sweep",
        )
    val track = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20)
    val semantics =
        if (contentDescription != null) {
            Modifier.semantics { this.contentDescription = contentDescription }
        } else {
            Modifier
        }
    Box(
        modifier = modifier.size(size.diameter).then(semantics),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(modifier = Modifier.size(size.diameter)) {
            val bandOnly = percent == null
            val strokePx = (if (bandOnly) size.bandStroke else size.stroke).toPx()
            val inset = strokePx / 2
            val topLeft = Offset(inset, inset)
            val arcSize = Size(this.size.width - strokePx, this.size.height - strokePx)
            if (bandOnly) {
                drawCircle(color = color.copy(alpha = BAND_FILL_ALPHA), radius = arcSize.width / 2)
                drawCircle(
                    color = color,
                    radius = arcSize.width / 2,
                    style = Stroke(width = strokePx),
                )
            } else {
                drawArc(
                    color = track,
                    startAngle = 0f,
                    sweepAngle = 360f,
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
    }
}

/**
 * How many people share an exit (`< 5`, `40+`, `< 20`) or the whole network (exact), behind a
 * person glyph.
 */
@Composable
fun PeoplePill(text: String, modifier: Modifier = Modifier) {
    val content = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha80)
    Row(
        modifier =
            modifier
                .height(PILL_HEIGHT)
                .border(
                    width = 1.dp,
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20),
                    shape = RoundedCornerShape(percent = 50),
                )
                .padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Icon(
            imageVector = Icons.Rounded.Person,
            contentDescription = null,
            tint = content,
            modifier = Modifier.size(12.dp),
        )
        Text(text = text, style = figureStyle(), color = content, maxLines = 1)
    }
}

@Composable
private fun figureStyle() =
    MaterialTheme.typography.labelMedium.copy(
        fontSize = 12.sp,
        lineHeight = 16.sp,
        fontWeight = FontWeight.SemiBold,
        fontFeatureSettings = "tnum",
    )

private val PILL_HEIGHT = 18.dp
