package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * Dedicated Multihop settings page (desktop `WarrenMultiHopSettingsView`
 * parity). Multi-hop is a real on/off toggle: ON (the default) is a 2-hop
 * circuit through a distinct entry relay so no single node sees both the user
 * IP and the destination, with the entry/exit country pickers; OFF is the
 * single-hop 1-hop circuit collapsed onto one node (faster, but that node sees
 * both). Both ride the same unified `:443` multi-hop wire (see
 * `WarrenTunnelConfigBuilder` / warren-jni `run_multi_hop_session`); the toggle
 * only chooses whether warren-jni picks a distinct entry or collapses onto the
 * exit. The country pickers apply only to the 2-hop circuit, so they show only
 * while the toggle is ON.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenMultihopSettings(navigator: Navigator) {
    val repo = koinInject<WarrenLocalSettingsRepository>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()
    val relayProvider = koinInject<WarrenRelayProvider>()

    // Distinct relay countries for the entry/exit pickers, sourced from the same
    // signed relay catalogue the location picker uses. Seeded from the cached
    // snapshot then refreshed off the main thread: the fetch behind the
    // catalogue blocks on a network round trip.
    val relays by produceState(initialValue = relayProvider.list()) {
        value = relayProvider.refresh()
    }
    // Code kept as the stored value, localized name shown and sorted on: the
    // rest of the app names countries, so a raw ISO code here reads as a
    // different catalogue.
    val countryOptions = remember(relays) {
        relays
            .map { it.country }
            .filter { it.isNotBlank() }
            .distinct()
            .map { it to countryDisplayName(it) }
            .sortedBy { it.second }
    }
    val multiHopEnabled by repo.multiHopEnabled.collectAsStateWithLifecycle()
    val entryCountry by repo.entryCountry.collectAsStateWithLifecycle()
    val exitCountry by repo.exitCountry.collectAsStateWithLifecycle()
    val connectedInfo by tunnelStateProvider.connectedInfo.collectAsStateWithLifecycle()

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.multihop),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
    ) { modifier ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(modifier)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            TunnelChangesBanner(connectedInfo) { reconnectInvoker.reconnect() }

            ToggleCell(
                title = stringResource(R.string.tunnel_multihop_title),
                subtitle = "",
                value = multiHopEnabled,
                onValueChange = repo::setMultiHopEnabled,
                position = Position.Single,
            )

            // Privacy/throughput tradeoff, shown in both states so the choice
            // is informed before the toggle is flipped.
            Text(
                text = stringResource(R.string.tunnel_multihop_tradeoff),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            // Entry/exit country pickers apply only to the 2-hop circuit; the
            // single-hop 1-hop circuit rides one node, so they are hidden when
            // multi-hop is off.
            if (multiHopEnabled) {
                Text(
                    text = stringResource(R.string.tunnel_multihop_desc),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                CountryDropdown(
                    label = stringResource(R.string.tunnel_entry_country_label),
                    automaticLabel = stringResource(R.string.automatic),
                    options = countryOptions,
                    selected = entryCountry,
                    onSelect = repo::setEntryCountry,
                )
                CountryDropdown(
                    label = stringResource(R.string.tunnel_exit_country_picker_label),
                    automaticLabel = stringResource(R.string.automatic),
                    options = countryOptions,
                    selected = exitCountry,
                    onSelect = repo::setExitCountry,
                )
            }
        }
    }
}
