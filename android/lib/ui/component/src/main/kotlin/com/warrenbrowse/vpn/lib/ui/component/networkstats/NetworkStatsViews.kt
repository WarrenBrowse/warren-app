package com.warrenbrowse.vpn.lib.ui.component.networkstats

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.intl.Locale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.model.ExitLoadBadge
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.model.NetworkStatsClock
import com.warrenbrowse.vpn.lib.model.NetworkStatsFormat
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRing
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import java.util.Locale as JavaLocale
import kotlinx.coroutines.delay

/** How much a stale snapshot is dimmed. */
private const val STALE_ALPHA = 0.5f
private const val FADE_MILLIS = 300

private val FIGURE_GAP = 4.dp
private val GROUP_GAP = 8.dp
private val GLYPH_SIZE = 12.dp

/** The locale figures are formatted in: the one the UI is displayed in. */
@Composable private fun figureLocale(): JavaLocale = Locale.current.platformLocale

@Composable
private fun loadLevelLabel(level: LoadLevel): String =
    stringResource(
        when (level) {
            LoadLevel.LOW -> R.string.network_stats_load_low
            LoadLevel.MODERATE -> R.string.network_stats_load_moderate
            LoadLevel.HIGH -> R.string.network_stats_load_high
            LoadLevel.SATURATED -> R.string.network_stats_load_saturated
            LoadLevel.UNKNOWN -> R.string.network_stats_load_unknown
        }
    )

/**
 * Whether [stats] is older than three windows. Flips once, when that moment comes, rather than
 * ticking: a whole location list reads it.
 */
@Composable
fun rememberSnapshotStale(stats: WarrenNetworkStats): Boolean {
    var stale by
        remember(stats) {
            mutableStateOf(NetworkStatsClock.isStale(stats, System.currentTimeMillis()))
        }
    LaunchedEffect(stats) {
        if (!stale) {
            delay(NetworkStatsClock.millisUntilStale(stats, System.currentTimeMillis()))
            stale = true
        }
    }
    return stale
}

/**
 * One exit's load as a single line of muted text: `[ring] 37%  [person] 40+  [down] 300 Mbit/s`.
 * The percentage and the rate appear only while the exit is live; a quiet exit shows its band as
 * the ring's shape and names it only to a screen reader. Read out as one sentence.
 */
@Composable
fun ExitLoadBadge(
    badge: ExitLoadBadge,
    modifier: Modifier = Modifier,
    showThroughput: Boolean = true,
    stale: Boolean = false,
) {
    val dim by
        animateFloatAsState(
            if (stale) STALE_ALPHA else 1f,
            animationSpec = tween(FADE_MILLIS),
            label = "exit_load_badge_alpha",
        )
    val locale = figureLocale()
    val muted = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60)
    val style = badgeTextStyle()
    when (badge) {
        ExitLoadBadge.Offline -> {
            val offline = stringResource(R.string.network_stats_offline)
            Row(
                modifier =
                    modifier.alpha(dim).clearAndSetSemantics { contentDescription = offline },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP),
            ) {
                LoadRing(level = LoadLevel.UNKNOWN, arcFraction = 0f, muted = true)
                Text(text = offline, style = style, color = muted, maxLines = 1)
            }
        }
        is ExitLoadBadge.Shown -> {
            val percent = badge.percent?.let { NetworkStatsFormat.percent(it, locale) }
            val people = NetworkStatsFormat.people(badge.people, locale)
            val rate =
                badge.downloadBps
                    ?.takeIf { showThroughput }
                    ?.let { NetworkStatsFormat.bitsPerSecond(it, locale) }
            val description =
                listOfNotNull(
                        if (percent != null) {
                            stringResource(R.string.network_stats_load_people, percent, people)
                        } else {
                            stringResource(
                                R.string.network_stats_band_people,
                                loadLevelLabel(badge.level),
                                people,
                            )
                        },
                        rate?.let { "${stringResource(R.string.network_stats_download)} $it" },
                    )
                    .joinToString(", ")
            Row(
                modifier =
                    modifier.alpha(dim).clearAndSetSemantics { contentDescription = description },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                LoadRing(level = badge.level, arcFraction = badge.arcFraction)
                if (percent != null) {
                    Spacer(Modifier.width(FIGURE_GAP))
                    Text(text = percent, style = style, color = muted, maxLines = 1)
                }
                Spacer(Modifier.width(GROUP_GAP))
                Figure(Icons.Rounded.Person, people, style, muted)
                if (rate != null) {
                    Spacer(Modifier.width(GROUP_GAP))
                    Figure(Icons.Rounded.ArrowDownward, rate, style, muted)
                }
            }
        }
    }
}

@Composable
private fun Figure(glyph: ImageVector, text: String, style: TextStyle, color: Color) {
    Icon(
        imageVector = glyph,
        contentDescription = null,
        tint = color,
        modifier = Modifier.size(GLYPH_SIZE),
    )
    Spacer(Modifier.width(2.dp))
    Text(text = text, style = style, color = color, maxLines = 1)
}

@Composable
private fun badgeTextStyle(): TextStyle =
    MaterialTheme.typography.labelMedium.copy(
        fontSize = 12.sp,
        lineHeight = 16.sp,
        fontWeight = FontWeight.Normal,
        fontFeatureSettings = "tnum",
    )
