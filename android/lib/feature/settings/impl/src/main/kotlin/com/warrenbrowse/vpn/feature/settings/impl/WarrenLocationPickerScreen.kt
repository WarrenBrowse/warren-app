package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.resource.R
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
    val recentExitIds by settings.recentExitIds.collectAsStateWithLifecycle()
    val recentsEnabled by settings.recentsEnabled.collectAsStateWithLifecycle()
    val customLists by settings.customLists.collectAsStateWithLifecycle()
    // The exit the user is currently adding to a custom list, if any.
    var addToListFor by remember { mutableStateOf<WarrenRelaySummary?>(null) }

    // RelayProvider.list() is synchronous (in-memory today); produceState
    // resolves a fresh list on every recomposition so a future async
    // fetch only needs to change the body without touching consumers.
    val relays by produceState(initialValue = emptyList<WarrenRelaySummary>()) {
        value = relayProvider.list()
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.location_exit_relay_title),
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
                    text = stringResource(R.string.location_no_relays_available),
                    style = MaterialTheme.typography.bodyMedium,
                )
            } else {
                var query by remember { mutableStateOf("") }
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(stringResource(R.string.location_search_hint)) },
                    singleLine = true,
                )

                val trimmed = query.trim()
                val filtered = if (trimmed.isEmpty()) {
                    relays
                } else {
                    relays.filter {
                        it.country.contains(trimmed, ignoreCase = true) ||
                            it.city.contains(trimmed, ignoreCase = true) ||
                            it.endpoint.contains(trimmed, ignoreCase = true)
                    }
                }

                if (filtered.isEmpty()) {
                    Text(
                        text = stringResource(R.string.location_no_exits_match, trimmed),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                } else {
                    Text(
                        text = stringResource(R.string.location_tap_to_select_hint),
                        style = MaterialTheme.typography.bodySmall,
                    )

                    // Recents privacy toggle (desktop parity), only while not
                    // searching so the search view stays focused on results.
                    if (trimmed.isEmpty()) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Text(
                                text = stringResource(R.string.location_remember_recent_exits),
                                style = MaterialTheme.typography.bodyMedium,
                                modifier = Modifier.weight(1f),
                            )
                            if (recentsEnabled && recentExitIds.isNotEmpty()) {
                                TextButton(onClick = { settings.clearRecentExits() }) {
                                    Text(stringResource(R.string.location_clear))
                                }
                            }
                            Switch(
                                checked = recentsEnabled,
                                onCheckedChange = settings::setRecentsEnabled,
                            )
                        }
                    }

                    // Recents only when not searching and remembering is on:
                    // resolve most-recent-first, dropping stale ids.
                    val recentRelays = if (trimmed.isEmpty() && recentsEnabled) {
                        recentExitIds.mapNotNull { id -> relays.firstOrNull { it.exitId == id } }
                    } else {
                        emptyList()
                    }
                    val onSelect: (WarrenRelaySummary) -> Unit = { relay ->
                        settings.setSelectedExitId(
                            if (relay.exitId == selectedExitId) null else relay.exitId
                        )
                    }
                    // Country -> city hierarchy: group the (filtered) exits by
                    // country, alphabetically, each under a country header.
                    val byCountry = filtered
                        .sortedWith(compareBy({ it.country }, { it.city }))
                        .groupBy { it.country }
                    LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (recentRelays.isNotEmpty()) {
                            item(key = "header-recents") {
                                SectionHeader(stringResource(R.string.location_recents))
                            }
                            items(recentRelays, key = { "recent-${it.exitId}" }) { relay ->
                                RelayRow(
                                    relay = relay,
                                    selected = relay.exitId == selectedExitId,
                                    onClick = { onSelect(relay) },
                                    onAddToList = { addToListFor = relay },
                                )
                            }
                        }
                        // Custom lists (desktop parity): only while not searching,
                        // each under its own header with the member exits.
                        if (trimmed.isEmpty()) {
                            customLists.forEach { (listName, exitIds) ->
                                val listRelays =
                                    exitIds.mapNotNull { id -> relays.firstOrNull { it.exitId == id } }
                                item(key = "customhdr-$listName") {
                                    CustomListHeader(
                                        name = listName,
                                        onDelete = { settings.deleteCustomList(listName) },
                                    )
                                }
                                items(listRelays, key = { "custom-$listName-${it.exitId}" }) { relay ->
                                    RelayRow(
                                        relay = relay,
                                        selected = relay.exitId == selectedExitId,
                                        onClick = { onSelect(relay) },
                                        onRemoveFromList = {
                                            settings.removeExitFromCustomList(listName, relay.exitId)
                                        },
                                    )
                                }
                            }
                        }
                        byCountry.forEach { (country, countryRelays) ->
                            item(key = "header-$country") {
                                SectionHeader(country.ifBlank { stringResource(R.string.location_unknown_country) })
                            }
                            items(countryRelays, key = { it.exitId }) { relay ->
                                RelayRow(
                                    relay = relay,
                                    selected = relay.exitId == selectedExitId,
                                    onClick = { onSelect(relay) },
                                    onAddToList = { addToListFor = relay },
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    addToListFor?.let { relay ->
        AddToListDialog(
            listNames = customLists.keys.toList(),
            onDismiss = { addToListFor = null },
            onPick = { listName ->
                settings.addExitToCustomList(listName, relay.exitId)
                addToListFor = null
            },
        )
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(top = 4.dp),
    )
}

@Composable
private fun CustomListHeader(name: String, onDelete: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = name,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onDelete) { Text(stringResource(R.string.location_delete_list)) }
    }
}

@Composable
private fun RelayRow(
    relay: WarrenRelaySummary,
    selected: Boolean,
    onClick: () -> Unit,
    onAddToList: (() -> Unit)? = null,
    onRemoveFromList: (() -> Unit)? = null,
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
        Row(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
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
                        text = stringResource(R.string.location_selected),
                        style = MaterialTheme.typography.labelSmall,
                        color = Color(0xFF2E7D32),
                    )
                }
                if (!relay.active) {
                    Text(
                        text = stringResource(R.string.location_inactive),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
            onRemoveFromList?.let {
                TextButton(onClick = it) { Text(stringResource(R.string.remove_button)) }
            }
            onAddToList?.let {
                TextButton(onClick = it) { Text(stringResource(R.string.location_add_to_list)) }
            }
        }
    }
}

/**
 * Dialog to add an exit to a custom list: pick an existing list or type a new
 * name. Creating a list with an exit in one step mirrors the desktop flow.
 */
@Composable
private fun AddToListDialog(
    listNames: List<String>,
    onDismiss: () -> Unit,
    onPick: (String) -> Unit,
) {
    var newName by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.location_add_to_list)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                listNames.forEach { name ->
                    TextButton(
                        onClick = { onPick(name) },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(name) }
                }
                OutlinedTextField(
                    value = newName,
                    onValueChange = { newName = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(stringResource(R.string.location_new_list_name)) },
                    singleLine = true,
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = newName.isNotBlank(),
                onClick = { onPick(newName.trim()) },
            ) { Text(stringResource(R.string.location_create_and_add)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
        },
    )
}
