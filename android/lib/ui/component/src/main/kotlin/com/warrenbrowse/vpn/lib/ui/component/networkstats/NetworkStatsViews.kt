package com.warrenbrowse.vpn.lib.ui.component.networkstats

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.intl.Locale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.model.ExitDisplayMode
import com.warrenbrowse.vpn.lib.model.ExitStats
import com.warrenbrowse.vpn.lib.model.LoadDriver
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.model.NetworkStatsClock
import com.warrenbrowse.vpn.lib.model.NetworkStatsFormat
import com.warrenbrowse.vpn.lib.model.PeopleCount
import com.warrenbrowse.vpn.lib.model.SnapshotAge
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.model.peopleOn
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LiveDot
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRing
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRingSize
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.PeoplePill
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.ThroughputPair
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80
import java.util.Locale as JavaLocale
import kotlinx.coroutines.delay

/** How much a stale snapshot, or an offline exit, is dimmed. */
const val NETWORK_STATS_MUTED_ALPHA = 0.5f

private const val TICK_MILLIS = 1000L
private const val FADE_MILLIS = 300

/** The locale figures are formatted in: the one the UI is displayed in. */
@Composable fun figureLocale(): JavaLocale = Locale.current.platformLocale

/** The wall clock, ticking once a second while in composition, for "updated N s ago". */
@Composable
fun rememberNowMillis(): Long {
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(TICK_MILLIS)
            now = System.currentTimeMillis()
        }
    }
    return now
}

@Composable
fun loadLevelLabel(level: LoadLevel): String =
    stringResource(
        when (level) {
            LoadLevel.LOW -> R.string.network_stats_load_low
            LoadLevel.MODERATE -> R.string.network_stats_load_moderate
            LoadLevel.HIGH -> R.string.network_stats_load_high
            LoadLevel.SATURATED -> R.string.network_stats_load_saturated
            LoadLevel.UNKNOWN -> R.string.network_stats_load_unknown
        }
    )

@Composable
fun loadDriverLabel(driver: LoadDriver?): String? =
    when (driver) {
        LoadDriver.BANDWIDTH -> stringResource(R.string.network_stats_limited_by_bandwidth)
        LoadDriver.CPU -> stringResource(R.string.network_stats_limited_by_cpu)
        LoadDriver.UNKNOWN,
        null -> null
    }

@Composable
fun peopleText(count: PeopleCount): String = NetworkStatsFormat.people(count, figureLocale())

@Composable
fun snapshotAgeText(age: SnapshotAge): String =
    when (age) {
        is SnapshotAge.Seconds ->
            stringResource(R.string.network_stats_updated_seconds, age.value.toInt())
        is SnapshotAge.Minutes ->
            stringResource(R.string.network_stats_updated_minutes, age.value.toInt())
        is SnapshotAge.Hours ->
            stringResource(R.string.network_stats_updated_hours, age.value.toInt())
    }

/** The live dot and "updated N s ago", from the moment the snapshot's window closed. */
@Composable
fun SnapshotFreshness(stats: WarrenNetworkStats, modifier: Modifier = Modifier) {
    val now = rememberNowMillis()
    val stale = NetworkStatsClock.isStale(stats, now)
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        LiveDot(fresh = !stale)
        Text(
            text = snapshotAgeText(NetworkStatsClock.age(NetworkStatsClock.ageSecs(stats, now))),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
            maxLines = 1,
        )
    }
}

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
 * The load of one exit at a glance: the ring, the percentage or the band name (colour is never the
 * only signal), and how many people share it. An exit below the live threshold shows its band and
 * `< 20`; an offline one a grey ring and "Offline". Read out as one sentence.
 */
@Composable
fun ExitLoadSummary(
    exit: ExitStats,
    stats: WarrenNetworkStats,
    ringSize: LoadRingSize,
    modifier: Modifier = Modifier,
    stale: Boolean = false,
    showPeople: Boolean = true,
) {
    val mode = exit.displayMode
    val dim by
        animateFloatAsState(
            if (stale || mode == ExitDisplayMode.OFFLINE) NETWORK_STATS_MUTED_ALPHA else 1f,
            animationSpec = tween(FADE_MILLIS),
            label = "exit_load_summary_alpha",
        )
    if (mode == ExitDisplayMode.OFFLINE) {
        val offline = stringResource(R.string.network_stats_offline)
        Row(
            modifier = modifier.alpha(dim).clearAndSetSemantics { contentDescription = offline },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            LoadRing(level = LoadLevel.UNKNOWN, percent = null, size = ringSize, muted = true)
            SummaryValue(offline)
        }
        return
    }
    val percent = if (mode == ExitDisplayMode.LIVE) exit.loadPercent else null
    val value =
        percent?.let { NetworkStatsFormat.percent(it, figureLocale()) }
            ?: loadLevelLabel(exit.loadLevel)
    val people = peopleText(stats.peopleOn(exit))
    val description = stringResource(R.string.network_stats_load_people, value, people)
    Row(
        modifier = modifier.alpha(dim).clearAndSetSemantics { contentDescription = description },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        LoadRing(level = exit.loadLevel, percent = percent, size = ringSize)
        SummaryValue(value)
        if (showPeople) PeoplePill(text = people)
    }
}

@Composable
private fun SummaryValue(text: String) {
    Text(
        text = text,
        style =
            MaterialTheme.typography.labelMedium.copy(
                fontSize = 12.sp,
                lineHeight = 16.sp,
                fontWeight = FontWeight.SemiBold,
                fontFeatureSettings = "tnum",
            ),
        color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha80),
        maxLines = 1,
    )
}

/** Download and upload of an exit or of the network, formatted for the UI locale. */
@Composable
fun ThroughputFigures(downloadBps: Long, uploadBps: Long, modifier: Modifier = Modifier) {
    val locale = figureLocale()
    ThroughputPair(
        download = NetworkStatsFormat.bitsPerSecond(downloadBps, locale),
        upload = NetworkStatsFormat.bitsPerSecond(uploadBps, locale),
        downloadDescription = stringResource(R.string.network_stats_download),
        uploadDescription = stringResource(R.string.network_stats_upload),
        modifier = modifier,
    )
}
