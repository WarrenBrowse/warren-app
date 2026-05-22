package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import org.koin.compose.koinInject

/**
 * D.6 location picker screen. Lists the available Warren exits read
 * from [WarrenRelayProvider] (backed by `WarrenJni.listRelays`). Tap
 * to select; the selection is persisted to
 * [WarrenLocalSettingsRepository.selectedExitId] and the next connect
 * routes through the chosen relay.
 *
 * Selecting an already-selected relay clears the override (back to
 * "first active" auto-pick). This matches the Mullvad UX where a
 * second tap on the active row clears the manual selection.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenLocationPicker(navigator: Navigator) {
    val relayProvider = koinInject<WarrenRelayProvider>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val selectedExitId by settings.selectedExitId.collectAsStateWithLifecycle()

    // RelayProvider.list() is synchronous (in-memory today); produceState
    // resolves a fresh list on every recomposition so a future async
    // fetch only needs to change the body without touching consumers.
    val relays by produceState(initialValue = emptyList<WarrenRelaySummary>()) {
        value = relayProvider.list()
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = "Exit relay",
        navigationIcon = {
            NavigateBackIconButton(onNavigateBack = {
                navigator.goBackUntil(SettingsNavKey)
            })
        },
    ) { modifier ->
        Column(
            modifier = Modifier.fillMaxSize().then(modifier).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (relays.isEmpty()) {
                Text(
                    text = "No relays available. The Warren API is unreachable or the catalogue is empty.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            } else {
                Text(
                    text = "Tap to select. Tap again to clear (auto-pick the first active exit).",
                    style = MaterialTheme.typography.bodySmall,
                )
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(relays) { relay ->
                        RelayRow(
                            relay = relay,
                            selected = relay.exitId == selectedExitId,
                            onClick = {
                                settings.setSelectedExitId(
                                    if (relay.exitId == selectedExitId) null else relay.exitId
                                )
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun RelayRow(
    relay: WarrenRelaySummary,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val rowAlpha = if (relay.active) 1.0f else 0.5f
    Card(
        modifier = Modifier.fillMaxWidth(),
        onClick = onClick,
        colors = CardDefaults.cardColors(
            containerColor = if (selected) {
                MaterialTheme.colorScheme.primaryContainer
            } else {
                MaterialTheme.colorScheme.surface
            },
        ),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(
                text = "${relay.country} • ${relay.city}",
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = rowAlpha),
            )
            Text(
                text = relay.endpoint,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = rowAlpha),
            )
            if (selected) {
                Text(
                    text = "Selected",
                    style = MaterialTheme.typography.labelSmall,
                    color = Color(0xFF2E7D32),
                )
            }
            if (!relay.active) {
                Text(
                    text = "Inactive",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}
