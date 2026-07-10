package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.relaylist.InactiveRelayIndicator
import com.warrenbrowse.vpn.lib.ui.designsystem.ListHeader
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemDefaults
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * Warren exit picker. Lists the available Warren exits read from
 * [WarrenRelayProvider]; tapping selects (a second tap on the active row
 * clears the override, back to auto-pick), persisting to
 * [WarrenLocalSettingsRepository.selectedExitId].
 *
 * The UX mirrors the upstream Mullvad SelectLocation screen: search field,
 * grouped rounded cells via the shared [WarrenListItem]/[ListHeader] design
 * system, a selected check and inactive dimming. Warren-specific features are
 * preserved: a "remember recent exits" toggle, recents, and custom lists. As
 * on desktop, long-pressing an exit adds it to a custom list (or removes it
 * from one inside a list section).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenLocationPicker(navigator: Navigator) {
    val relayProvider = koinInject<WarrenRelayProvider>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val tunnelState by tunnelStateProvider.state.collectAsStateWithLifecycle()
    val selectedExitId by settings.selectedExitId.collectAsStateWithLifecycle()
    val recentExitIds by settings.recentExitIds.collectAsStateWithLifecycle()
    val recentsEnabled by settings.recentsEnabled.collectAsStateWithLifecycle()
    val customLists by settings.customLists.collectAsStateWithLifecycle()
    var addToListFor by remember { mutableStateOf<WarrenRelaySummary?>(null) }

    // RelayProvider.list() is synchronous (in-memory today); produceState
    // resolves a fresh list so a future async fetch only needs to change the
    // body without touching consumers.
    val relays by produceState(initialValue = emptyList<WarrenRelaySummary>()) {
        value = relayProvider.list()
    }

    val onSelect: (WarrenRelaySummary) -> Unit = { relay ->
        settings.setSelectedExitId(if (relay.exitId == selectedExitId) null else relay.exitId)
        // Apply the new exit immediately when a tunnel is up: the service
        // rebuilds the config from the just-updated selection and reconnects
        // through the new exit (reusing the cached mnemonic, no biometric
        // re-prompt). When disconnected this is a no-op and the change applies
        // on the next connect. Mirrors the desktop, which reconnects on a
        // location change.
        if (tunnelState.startsWith("Connected")) {
            reconnectInvoker.reconnect()
        }
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.location_exit_relay_title),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = { navigator.goBack() }) },
    ) { modifier ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(modifier)
                .padding(horizontal = Dimens.sideMargin),
        ) {
            if (relays.isEmpty()) {
                Text(
                    text = stringResource(R.string.location_no_relays_available),
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(top = Dimens.mediumPadding),
                )
                return@Column
            }

            var query by remember { mutableStateOf("") }
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier.fillMaxWidth().padding(vertical = Dimens.smallPadding),
                label = { Text(stringResource(R.string.location_search_hint)) },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
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
                    modifier = Modifier.padding(top = Dimens.mediumPadding),
                )
                return@Column
            }

            val notSearching = trimmed.isEmpty()
            val recentRelays = if (notSearching && recentsEnabled) {
                recentExitIds.mapNotNull { id -> relays.firstOrNull { it.exitId == id } }
            } else {
                emptyList()
            }
            val byCountry = filtered
                .sortedWith(compareBy({ it.country }, { it.city }))
                .groupBy { it.country }

            LazyColumn(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(Dimens.listItemDivider),
            ) {
                // Remember-recent-exits toggle (Warren feature), styled as a
                // single cell so it reads as part of the list.
                if (notSearching) {
                    item(key = "recents-toggle") {
                        WarrenListItem(
                            position = Position.Single,
                            onClick = { settings.setRecentsEnabled(!recentsEnabled) },
                            content = {
                                Text(
                                    text = stringResource(R.string.location_remember_recent_exits),
                                    modifier = Modifier.align(Alignment.CenterStart),
                                )
                            },
                            trailingContent = {
                                WarrenSwitch(
                                    checked = recentsEnabled,
                                    onCheckedChange = settings::setRecentsEnabled,
                                )
                            },
                        )
                    }
                    item(key = "recents-toggle-gap") { SectionGap() }
                }

                if (recentRelays.isNotEmpty()) {
                    item(key = "header-recents") {
                        ListHeader(
                            content = { Text(stringResource(R.string.location_recents)) },
                            actions = {
                                TextButton(onClick = { settings.clearRecentExits() }) {
                                    Text(stringResource(R.string.location_clear))
                                }
                            },
                        )
                    }
                    itemsIndexed(recentRelays, key = { _, r -> "recent-${r.exitId}" }) { i, relay ->
                        ExitCell(
                            title = "${relay.country}  ${relay.city}",
                            endpoint = relay.endpoint,
                            selected = relay.exitId == selectedExitId,
                            active = relay.active,
                            position = positionOf(i, recentRelays.size),
                            onClick = { onSelect(relay) },
                            onLongClick = { addToListFor = relay },
                        )
                    }
                    item(key = "recents-gap") { SectionGap() }
                }

                // Custom lists (Warren feature): each under its own header with
                // a delete action; long-press a member to remove it.
                if (notSearching) {
                    customLists.forEach { (listName, exitIds) ->
                        val listRelays =
                            exitIds.mapNotNull { id -> relays.firstOrNull { it.exitId == id } }
                        item(key = "customhdr-$listName") {
                            ListHeader(
                                content = { Text(listName) },
                                actions = {
                                    TextButton(onClick = { settings.deleteCustomList(listName) }) {
                                        Text(stringResource(R.string.location_delete_list))
                                    }
                                },
                            )
                        }
                        itemsIndexed(
                            listRelays,
                            key = { _, r -> "custom-$listName-${r.exitId}" },
                        ) { i, relay ->
                            ExitCell(
                                title = "${relay.country}  ${relay.city}",
                                endpoint = relay.endpoint,
                                selected = relay.exitId == selectedExitId,
                                active = relay.active,
                                position = positionOf(i, listRelays.size),
                                onClick = { onSelect(relay) },
                                onLongClick = {
                                    settings.removeExitFromCustomList(listName, relay.exitId)
                                },
                            )
                        }
                        item(key = "customgap-$listName") { SectionGap() }
                    }
                }

                // Exits grouped by country, each country under its own header.
                byCountry.forEach { (country, countryRelays) ->
                    item(key = "header-$country") {
                        ListHeader(
                            text = country.ifBlank {
                                stringResource(R.string.location_unknown_country)
                            },
                        )
                    }
                    itemsIndexed(countryRelays, key = { _, r -> r.exitId }) { i, relay ->
                        ExitCell(
                            title = relay.city.ifBlank { relay.country },
                            endpoint = relay.endpoint,
                            selected = relay.exitId == selectedExitId,
                            active = relay.active,
                            position = positionOf(i, countryRelays.size),
                            onClick = { onSelect(relay) },
                            onLongClick = { addToListFor = relay },
                        )
                    }
                    item(key = "countrygap-$country") { SectionGap() }
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

/** Position of an item within a section so its corners round into a block. */
private fun positionOf(index: Int, count: Int): Position = when {
    count <= 1 -> Position.Single
    index == 0 -> Position.Top
    index == count - 1 -> Position.Bottom
    else -> Position.Middle
}

@Composable
private fun SectionGap() {
    Spacer(modifier = Modifier.height(Dimens.mediumPadding))
}

/**
 * A single exit row, styled like the upstream Mullvad relay cell: a selected
 * check (or an inactive indicator) leading the name, with the Warren endpoint
 * as a dimmed subtitle.
 */
@Composable
private fun ExitCell(
    title: String,
    endpoint: String,
    selected: Boolean,
    active: Boolean,
    position: Position,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
) {
    val colors = ListItemDefaults.colors()
    WarrenListItem(
        position = position,
        isSelected = selected,
        isEnabled = true,
        onClick = onClick,
        onLongClick = onLongClick,
        colors = colors,
        leadingContent = {
            if (selected) {
                Icon(
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(end = Dimens.smallPadding),
                    imageVector = Icons.Rounded.Check,
                    contentDescription = null,
                    tint = if (!active) MaterialTheme.colorScheme.error else LocalContentColor.current,
                )
            } else if (!active) {
                InactiveRelayIndicator(
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(end = Dimens.smallPadding),
                    tint = MaterialTheme.colorScheme.error,
                )
            }
        },
        content = {
            Column(modifier = Modifier.align(Alignment.CenterStart)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color = colors.headlineColor(enabled = active, selected = selected),
                )
                Text(
                    text = endpoint,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
    )
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
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
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
