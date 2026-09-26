package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.model.ExitDisplayMode
import com.warrenbrowse.vpn.lib.model.LoadLevel
import com.warrenbrowse.vpn.lib.model.NetworkStatsFormat
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.model.fleetPeople
import com.warrenbrowse.vpn.lib.model.peopleOn
import com.warrenbrowse.vpn.lib.repository.NetworkStatsAvailability
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkStatsProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.networkstats.NETWORK_STATS_MUTED_ALPHA
import com.warrenbrowse.vpn.lib.ui.component.networkstats.SnapshotFreshness
import com.warrenbrowse.vpn.lib.ui.component.networkstats.ThroughputFigures
import com.warrenbrowse.vpn.lib.ui.component.networkstats.figureLocale
import com.warrenbrowse.vpn.lib.ui.component.networkstats.loadDriverLabel
import com.warrenbrowse.vpn.lib.ui.component.networkstats.loadLevelLabel
import com.warrenbrowse.vpn.lib.ui.component.networkstats.peopleText
import com.warrenbrowse.vpn.lib.ui.component.networkstats.rememberSnapshotStale
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorMedium
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRing
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.LoadRingSize
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.PeoplePill
import com.warrenbrowse.vpn.lib.ui.designsystem.networkstats.Sparkline
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80
import java.text.NumberFormat
import org.koin.compose.koinInject

private const val FADE_MILLIS = 300

/**
 * The Warren network page: the fleet's live figures, one card per exit, and how the numbers are
 * made. Collecting the feed here is what makes it poll, so it asks the API once per window while
 * this page is on screen and the app in the foreground, and never otherwise.
 *
 * Until the API serves the snapshot (404), the page says so calmly and shows nothing else; the last
 * good snapshot stays on screen, greyed, through later failures.
 */
@Composable
fun WarrenNetwork(navigator: Navigator) {
    val state by koinInject<WarrenNetworkStatsProvider>().state.collectAsStateWithLifecycle()
    val relays by koinInject<WarrenRelayProvider>().catalogue.collectAsStateWithLifecycle()

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.network_stats_title),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
    ) { modifier ->
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .then(modifier)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding)
                    .testTag(WARREN_NETWORK_TEST_TAG),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            val snapshot = state.snapshot
            when {
                snapshot != null -> NetworkContent(snapshot, networkExitCards(snapshot, relays))
                state.availability == NetworkStatsAvailability.LOADING ->
                    Box(Modifier.fillMaxWidth().padding(Dimens.largePadding)) {
                        WarrenCircularProgressIndicatorMedium(Modifier.align(Alignment.Center))
                    }
                else ->
                    Text(
                        text = stringResource(R.string.network_stats_unavailable),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(vertical = Dimens.largePadding),
                    )
            }
        }
    }
}

const val WARREN_NETWORK_TEST_TAG = "warren_network_screen"

@Composable
private fun ColumnScope.NetworkContent(stats: WarrenNetworkStats, cards: List<NetworkExitCard>) {
    val stale = rememberSnapshotStale(stats)
    val dim by
        animateFloatAsState(
            if (stale) NETWORK_STATS_MUTED_ALPHA else 1f,
            animationSpec = tween(FADE_MILLIS),
            label = "network_stats_stale",
        )
    Column(
        modifier = Modifier.alpha(dim),
        verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
    ) {
        FleetCard(stats)
        SectionHeading(stringResource(R.string.network_stats_exits))
        cards.forEach { ExitCard(it, stats) }
    }
    Methodology(stats)
}

@Composable
private fun StatsCard(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                .background(
                    color = MaterialTheme.colorScheme.surfaceContainer,
                    shape = RoundedCornerShape(12.dp),
                )
                .padding(Dimens.mediumPadding),
        verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
        content = content,
    )
}

@Composable
private fun Label(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
        modifier = modifier,
    )
}

@Composable
private fun Value(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium.copy(fontFeatureSettings = "tnum"),
        color = MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun SectionHeading(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.padding(top = Dimens.smallPadding).semantics { heading() },
    )
}

@Composable
private fun RingCenter(value: String, caption: String?) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            text = value,
            style =
                MaterialTheme.typography.titleLarge.copy(
                    fontSize = if (caption == null) 13.sp else 22.sp,
                    lineHeight = if (caption == null) 16.sp else 26.sp,
                    fontWeight = FontWeight.SemiBold,
                    fontFeatureSettings = "tnum",
                ),
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
        )
        if (caption != null) {
            Text(
                text = caption,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
            )
        }
    }
}

@Composable
private fun FleetCard(stats: WarrenNetworkStats) {
    val locale = figureLocale()
    val count = { value: Long -> NumberFormat.getIntegerInstance(locale).format(value) }
    val fleetLoad = NetworkStatsFormat.percent(stats.fleet.loadPercent, locale)
    StatsCard(Modifier.testTag(WARREN_NETWORK_FLEET_TEST_TAG)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Label(stringResource(R.string.network_stats_people_now), Modifier.weight(1f))
            SnapshotFreshness(stats)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = peopleText(stats.fleetPeople()),
                style =
                    MaterialTheme.typography.displaySmall.copy(
                        fontWeight = FontWeight.SemiBold,
                        fontFeatureSettings = "tnum",
                    ),
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f),
            )
            LoadRing(
                level = stats.fleet.loadLevel,
                percent = stats.fleet.loadPercent,
                size = LoadRingSize.LARGE,
                contentDescription =
                    stringResource(R.string.network_stats_network_load_description, fleetLoad),
            ) {
                RingCenter(fleetLoad, stringResource(R.string.network_stats_caption_network_load))
            }
        }
        Figure(stringResource(R.string.network_stats_accounts)) {
            Value(count(stats.users.accountsTotal))
        }
        Figure(stringResource(R.string.network_stats_subscribers)) {
            Value(count(stats.users.subscribersActive))
        }
        Figure(stringResource(R.string.network_stats_exits_online)) {
            Value(
                stringResource(
                    R.string.network_stats_online_of_total,
                    stats.fleet.exitsOnline,
                    stats.fleet.exitsTotal,
                )
            )
        }
        Figure(stringResource(R.string.network_stats_throughput)) {
            ThroughputFigures(stats.fleet.downloadBps, stats.fleet.uploadBps)
        }
        Figure(stringResource(R.string.network_stats_data_24h)) {
            Value(NetworkStatsFormat.bytes(stats.fleet.transferred24hBytes, locale))
        }
        if (stats.history.isNotEmpty()) {
            Row(horizontalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
                Chart(
                    stringResource(R.string.network_stats_people_24h),
                    stats.history.map { it.connected.toLong() },
                    Modifier.weight(1f),
                )
                Chart(
                    stringResource(R.string.network_stats_throughput_24h),
                    stats.history.map { it.throughputBps },
                    Modifier.weight(1f),
                )
            }
        }
    }
}

const val WARREN_NETWORK_FLEET_TEST_TAG = "warren_network_fleet"

@Composable
private fun Figure(label: String, value: @Composable () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Label(label, Modifier.weight(1f))
        value()
    }
}

@Composable
private fun Chart(label: String, values: List<Long>, modifier: Modifier = Modifier) {
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Label(label)
        Sparkline(values = values, modifier = Modifier.fillMaxWidth().height(36.dp))
    }
}

@Composable
private fun ExitCard(card: NetworkExitCard, stats: WarrenNetworkStats) {
    val exit = card.exit
    val mode = exit.displayMode
    val locale = figureLocale()
    val country = countryDisplayName(card.country, locale).ifBlank { card.country }
    val title =
        if (card.city.isBlank()) country
        else stringResource(R.string.country_comma_city, country, card.city)
    StatsCard(
        Modifier.alpha(if (mode == ExitDisplayMode.OFFLINE) NETWORK_STATS_MUTED_ALPHA else 1f)
            .testTag(WARREN_NETWORK_EXIT_TEST_TAG)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f),
            )
            if (mode == ExitDisplayMode.OFFLINE)
                Label(stringResource(R.string.network_stats_offline))
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            ExitRing(card, mode)
            if (mode != ExitDisplayMode.OFFLINE) {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    PeoplePill(peopleText(stats.peopleOn(exit)))
                    if (mode == ExitDisplayMode.LIVE) {
                        ThroughputFigures(exit.downloadBps, exit.uploadBps)
                        loadDriverLabel(exit.loadDriver)?.let {
                            Text(
                                text = it,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha80),
                            )
                        }
                        val details =
                            listOfNotNull(
                                exit.capacityBps?.let {
                                    stringResource(
                                        R.string.network_stats_link_capacity,
                                        NetworkStatsFormat.bitsPerSecond(it, locale),
                                    )
                                },
                                exit.cpuPercent?.let {
                                    stringResource(
                                        R.string.network_stats_cpu,
                                        NetworkStatsFormat.percent(it, locale),
                                    )
                                },
                            )
                        if (details.isNotEmpty()) Label(details.joinToString(" · "))
                    } else {
                        Label(stringResource(R.string.network_stats_band_last_hour))
                    }
                }
            }
        }
        if (mode == ExitDisplayMode.LIVE && exit.history.isNotEmpty()) {
            Sparkline(
                values = exit.history.map { it.throughputBps },
                modifier = Modifier.fillMaxWidth().height(32.dp),
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
            )
        }
        if (mode == ExitDisplayMode.BAND) ThresholdNote(stats.exitLiveThreshold)
    }
}

const val WARREN_NETWORK_EXIT_TEST_TAG = "warren_network_exit"

@Composable
private fun ExitRing(card: NetworkExitCard, mode: ExitDisplayMode) {
    val exit = card.exit
    when (mode) {
        ExitDisplayMode.OFFLINE ->
            LoadRing(
                level = LoadLevel.UNKNOWN,
                percent = null,
                size = LoadRingSize.LARGE,
                muted = true,
                modifier = Modifier.clearAndSetSemantics {},
            )
        ExitDisplayMode.BAND -> {
            val band = loadLevelLabel(exit.loadLevel)
            LoadRing(
                level = exit.loadLevel,
                percent = null,
                size = LoadRingSize.LARGE,
                contentDescription = band,
            ) {
                RingCenter(band, caption = null)
            }
        }
        ExitDisplayMode.LIVE -> {
            val percent = exit.loadPercent
            val value =
                percent?.let { NetworkStatsFormat.percent(it, figureLocale()) }
                    ?: loadLevelLabel(exit.loadLevel)
            LoadRing(
                level = exit.loadLevel,
                percent = percent,
                size = LoadRingSize.LARGE,
                contentDescription = value,
            ) {
                RingCenter(
                    value,
                    if (percent != null) stringResource(R.string.network_stats_caption_load)
                    else null,
                )
            }
        }
    }
}

/** Why a quiet exit shows its band alone. */
@Composable
private fun ThresholdNote(threshold: Int) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Icon(
            imageVector = Icons.Rounded.Info,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
            modifier = Modifier.size(14.dp).padding(top = 1.dp),
        )
        Text(
            text = stringResource(R.string.network_stats_live_threshold_note, threshold),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
        )
    }
}

@Composable
private fun Methodology(stats: WarrenNetworkStats) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.smallPadding),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        SectionHeading(stringResource(R.string.network_stats_methodology_title))
        listOf(
                stringResource(R.string.network_stats_methodology_window, stats.windowSecs),
                stringResource(
                    R.string.network_stats_methodology_threshold,
                    stats.exitLiveThreshold,
                ),
                stringResource(
                    R.string.network_stats_methodology_rounding,
                    stats.exitUsersRounding,
                ),
                stringResource(R.string.network_stats_methodology_load),
            )
            .forEach { line ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = "•",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                    )
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha80),
                    )
                }
            }
    }
}
