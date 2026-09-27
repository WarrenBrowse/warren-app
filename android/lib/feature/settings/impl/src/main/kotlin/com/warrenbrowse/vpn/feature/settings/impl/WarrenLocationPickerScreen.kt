package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.ConnectAfterLocationPick
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkStatsProvider
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.networkstats.rememberSnapshotStale
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenAlertDialog
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/** What the catalogue is doing, so a fetch in flight never reads as "nothing here". */
private enum class CatalogueState {
    Loading,
    Empty,
    Content,
}

private val ExpandedKeySaver: Saver<Set<String>, Any> =
    listSaver(save = { it.toList() }, restore = { it.toSet() })

/**
 * Warren exit picker. Lists the available Warren exits read from
 * [WarrenRelayProvider] as a three-level accordion (country > city > relay,
 * matching the desktop SelectLocation). Every level is selectable: the label
 * area pins that scope, the trailing chevron expands, and an explicit
 * Automatic row heads the list, so a tap is always a selection and never a
 * hidden toggle back to auto-pick.
 *
 * A pick is terminal (it pops back to whichever screen pushed the picker) and
 * it applies immediately: it reconnects a live tunnel, and when [connectOnPick]
 * is set it hands a [ConnectAfterLocationPick] result back so the caller starts
 * the tunnel through its own VPN-consent gate.
 *
 * With multi-hop on, the scope bar chooses which hop the list is picking.
 * The entry hop is a country constraint, so its tab lists countries only and
 * a pick auto-advances to the exit tab rather than popping.
 *
 * Rows are labelled geographically only. The catalogue carries no hostname, so
 * several exits in one city are told apart by an ordinal derived from the
 * sorted exit id; the endpoint address is never rendered and never searched.
 *
 * Every location row has one anatomy wherever it appears (recents, custom
 * lists, the tree): its round flag (ringed in green while it holds the
 * selection), its name over an optional muted subtitle, the load badge when it
 * stands for one exit, then a quiet "more" button, also reached by a long
 * press, opening the lists menu: every list, checked when it holds the
 * location, a tap toggling membership, and "New list…" to start a list around
 * it. Recents can be disabled from the top-bar overflow menu.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
@Suppress("LongMethod", "CyclomaticComplexMethod")
fun WarrenLocationPicker(navigator: Navigator, connectOnPick: Boolean = false) {
    val relayProvider = koinInject<WarrenRelayProvider>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    // Only the in-flight bit of the tunnel state reaches this composition: a
    // dial is already running, so every row is inert until it settles. The
    // whole state used to be collected, and every Connecting, Connected or
    // Disconnecting edge recomposed the list; the pick reads the live state
    // at tap time instead.
    val inFlight by
        remember(tunnelStateProvider) {
                tunnelStateProvider.connectedInfo.map { transitionInFlight(it) }.distinctUntilChanged()
            }
            .collectAsStateWithLifecycle(
                initialValue = transitionInFlight(tunnelStateProvider.connectedInfo.value)
            )
    val exitPin by settings.exitPin.collectAsStateWithLifecycle()
    val recentPins by settings.recentPins.collectAsStateWithLifecycle()
    val recentsEnabled by settings.recentsEnabled.collectAsStateWithLifecycle()
    val customLists by settings.customLists.collectAsStateWithLifecycle()
    val multiHopEnabled by settings.multiHopEnabled.collectAsStateWithLifecycle()
    val entryCountry by settings.entryCountry.collectAsStateWithLifecycle()

    // Dialog and menu holders keep only saveable identifiers, so a rotation
    // mid-decision reopens on the same list or location instead of dropping it.
    var newListFor by rememberSaveable(stateSaver = LocationSaver) { mutableStateOf<ExitPin?>(null) }
    var renameListFor by rememberSaveable { mutableStateOf<String?>(null) }
    var deleteListFor by rememberSaveable { mutableStateOf<String?>(null) }
    var confirmClearRecents by rememberSaveable { mutableStateOf(false) }
    var confirmDisableRecents by rememberSaveable { mutableStateOf(false) }
    var rowMenuFor by rememberSaveable { mutableStateOf<String?>(null) }
    var overflowMenuOpen by rememberSaveable { mutableStateOf(false) }

    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val keyboard = LocalSoftwareKeyboardController.current
    val context = LocalContext.current
    val undoLabel = stringResource(R.string.undo)

    // Which hop the list is picking. Only reachable while multi-hop is on, so
    // turning it off from another screen collapses the picker back to the exit.
    var pickerScope by rememberSaveable { mutableStateOf(PickerScope.Exit) }
    val activeScope = if (multiHopEnabled) pickerScope else PickerScope.Exit

    var expandedCountries by
        rememberSaveable(stateSaver = ExpandedKeySaver) { mutableStateOf(emptySet<String>()) }
    var expandedCities by
        rememberSaveable(stateSaver = ExpandedKeySaver) { mutableStateOf(emptySet<String>()) }
    var expandedLists by
        rememberSaveable(stateSaver = ExpandedKeySaver) { mutableStateOf(emptySet<String>()) }
    var query by rememberSaveable { mutableStateOf("") }
    var seeded by rememberSaveable { mutableStateOf(false) }
    var didScroll by rememberSaveable { mutableStateOf(false) }
    var revealCountry by rememberSaveable { mutableStateOf<String?>(null) }
    val listState = rememberLazyListState()

    // The catalogue is a stream so a refresh landing after the screen opened
    // replaces the cold snapshot, and the fetch is tracked separately: an empty
    // list during the signed round trip is "not loaded yet", never "no relays".
    // Opening the picker takes the snapshot while it is fresh (the daemon's
    // hourly cadence); only the user's Retry forces a fetch.
    val relays by relayProvider.catalogue.collectAsStateWithLifecycle()
    // Collected with the lifecycle: the feed polls only while this list is on screen and the app
    // is in the foreground, and shows nothing at all while no snapshot is held (the endpoint not
    // deployed yet included).
    val networkStats by
        koinInject<WarrenNetworkStatsProvider>().state.collectAsStateWithLifecycle()
    val statsSnapshot = networkStats.snapshot
    val statsStale = statsSnapshot?.let { rememberSnapshotStale(it) } ?: false
    var refreshTick by rememberSaveable { mutableStateOf(0) }
    var refreshing by remember { mutableStateOf(true) }
    LaunchedEffect(refreshTick) {
        refreshing = true
        if (refreshTick == 0) relayProvider.refreshIfStale() else relayProvider.refresh()
        refreshing = false
    }

    // Expand the branch holding the current selection once the catalogue loads,
    // so scroll-to-selected has a row to land on.
    LaunchedEffect(relays, exitPin) {
        if (!seeded && relays.isNotEmpty()) {
            val branches = expandedKeysFor(exitPin, relays)
            expandedCountries = expandedCountries + branches.countries
            expandedCities = expandedCities + branches.cities
            seeded = true
        }
    }

    // A pick is terminal: persist it, put the tunnel change in flight, then pop
    // back to whichever screen pushed the picker (Connect or port forwarding).
    val applyPin: (ExitPin) -> Unit = { pin ->
        settings.setExitPin(pin)
        val followUp = pickFollowUp(tunnelStateProvider.connectedInfo.value)
        if (followUp == PickFollowUp.Reconnect) reconnectInvoker.reconnect()
        if (followUp == PickFollowUp.Connect && connectOnPick) {
            // The caller owns the VPN-consent gate and the biometric host, so
            // the connect is handed back rather than dispatched from here.
            navigator.goBack(ConnectAfterLocationPick)
        } else {
            navigator.goBack()
        }
    }

    val applied = appliedQuery(query)
    val searching = applied.isNotEmpty()

    // The row list is a pure function of its inputs, computed when one of them
    // changes and never on a recomposition caused by anything else.
    val pickerInputs =
        PickerInputs(
            relays = relays,
            query = applied,
            scope = activeScope,
            entryCountry = entryCountry,
            recentsEnabled = recentsEnabled,
            recentPins = recentPins,
            customLists = customLists,
            exitPin = exitPin,
            expanded = ExpandedKeys(expandedCountries, expandedCities, expandedLists),
        )
    val rows = remember(pickerInputs) { pickerRows(pickerInputs) }

    val noSearchResult = searching &&
        rows.none { it is PickerRow.ExitRow || it is PickerRow.EntryCountryRow }

    // Applying a search restarts the list at the top: results below the old
    // scroll offset would otherwise open off screen.
    LaunchedEffect(applied) {
        if (applied.isNotEmpty()) listState.scrollToItem(0)
    }

    // Scrolling is the gesture that says "let me read the list", so the IME
    // gets out of the way without a dismiss tap.
    LaunchedEffect(listState.isScrollInProgress) {
        if (listState.isScrollInProgress) keyboard?.hide()
    }

    // Scroll to the current selection once, targeting its country header so the
    // parent context stays on screen. Saved across rotation so it never fires
    // twice and yanks the user back.
    val scrollTarget = scrollTargetIndex(rows)
    LaunchedEffect(scrollTarget) {
        if (didScroll || searching || scrollTarget < 0) return@LaunchedEffect
        // Wait for the first layout: before it there is no viewport to compare
        // the target against, and every row would read as off screen.
        val layout = snapshotFlow { listState.layoutInfo }
            .first { it.visibleItemsInfo.isNotEmpty() }
        val visible = layout.visibleItemsInfo
        if (shouldScrollTo(scrollTarget, visible.first().index, visible.last().index)) {
            listState.animateScrollToItem(scrollTarget)
        }
        didScroll = true
    }

    // A country expanded near the bottom would reveal its children off screen.
    LaunchedEffect(revealCountry, rows.size) {
        val country = revealCountry ?: return@LaunchedEffect
        val header = rows.indexOfFirst { it is PickerRow.CountryHeader && it.country == country }
        val lastVisible = listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index
        if (header >= 0 && lastVisible != null && countryBlockEnd(rows, header) > lastVisible) {
            listState.animateScrollToItem(header)
        }
        revealCountry = null
    }

    // Membership changes answer with a short snackbar naming the list, and a
    // removal offers its undo, so a toggle made from a folded list is never silent.
    val toggleList: (String, ExitPin) -> Unit = { listName, location ->
        val outcome = toggleListMembership(
            name = listName,
            location = location,
            customLists = customLists,
            add = settings::addLocationToCustomList,
            remove = settings::removeLocationFromCustomList,
        )
        scope.launch {
            snackbarHostState.currentSnackbarData?.dismiss()
            if (outcome == ListToggle.Added) {
                snackbarHostState.showSnackbar(
                    message = context.getString(R.string.location_added_to_list, listName),
                    duration = SnackbarDuration.Short,
                )
            } else {
                showRemovalUndo(
                    snackbarHostState = snackbarHostState,
                    message = context.getString(R.string.location_removed_from_named_list, listName),
                    undoLabel = undoLabel,
                    onUndo = { settings.addLocationToCustomList(listName, location) },
                )
            }
        }
    }
    val actions = PickerActions(
        onApplyPin = applyPin,
        onEntryPick = { country ->
            pickerScope = applyEntryPick(country, settings::setEntryCountry)
        },
        onClearRecents = { confirmClearRecents = true },
        onRowMenu = { rowMenuFor = it },
        expand = ExpandActions(
            onToggleCountry = { country ->
                val open = country in expandedCountries
                expandedCountries = if (open) {
                    expandedCountries - country
                } else {
                    revealCountry = country
                    expandedCountries + country
                }
            },
            onToggleCity = { key ->
                expandedCities = if (key in expandedCities) expandedCities - key else expandedCities + key
            },
            onToggleList = { name ->
                expandedLists = if (name in expandedLists) expandedLists - name else expandedLists + name
            },
        ),
        lists = ListActions(
            memberships = { location -> listMemberships(location, customLists) },
            onToggleMembership = { listName, location ->
                rowMenuFor = null
                toggleList(listName, location)
            },
            onNewList = { location ->
                rowMenuFor = null
                newListFor = location
            },
            onRenameList = { renameListFor = it },
            onDeleteList = { deleteListFor = it },
        ),
    )

    val catalogueState = when {
        relays.isNotEmpty() -> CatalogueState.Content
        refreshing -> CatalogueState.Loading
        else -> CatalogueState.Empty
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.location_exit_relay_title),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
        actions = {
            Box {
                IconButton(onClick = { overflowMenuOpen = true }) {
                    Icon(
                        imageVector = Icons.Rounded.MoreVert,
                        contentDescription = stringResource(R.string.location_more_options),
                    )
                }
                DropdownMenu(
                    expanded = overflowMenuOpen,
                    onDismissRequest = { overflowMenuOpen = false },
                ) {
                    DropdownMenuItem(
                        text = {
                            Text(
                                stringResource(
                                    if (recentsEnabled) {
                                        R.string.location_disable_recents
                                    } else {
                                        R.string.location_enable_recents
                                    }
                                )
                            )
                        },
                        onClick = {
                            overflowMenuOpen = false
                            // Disabling also wipes the history, so it asks first.
                            if (recentsEnabled) {
                                confirmDisableRecents = true
                            } else {
                                settings.setRecentsEnabled(true)
                            }
                        },
                    )
                }
            }
        },
        snackbarHostState = snackbarHostState,
    ) { modifier ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(modifier)
                .padding(horizontal = Dimens.sideMargin),
        ) {
            AnimatedContent(
                targetState = catalogueState,
                transitionSpec = {
                    fadeIn(tween(PICKER_FADE_MS)) togetherWith fadeOut(tween(PICKER_FADE_MS))
                },
                label = "location-catalogue",
            ) { state ->
                when (state) {
                    CatalogueState.Loading ->
                        Box(
                            modifier = Modifier.fillMaxSize(),
                            contentAlignment = Alignment.Center,
                        ) { WarrenCircularProgressIndicatorLarge() }

                    CatalogueState.Empty -> EmptyCatalogue(onRetry = { refreshTick++ })

                    CatalogueState.Content ->
                        Column(modifier = Modifier.fillMaxSize()) {
                            if (multiHopEnabled) {
                                HopScopeBar(
                                    scope = activeScope,
                                    onScopeChange = { pickerScope = it },
                                )
                            }

                            SearchField(
                                query = query,
                                onQueryChange = { query = it },
                                onSearch = { keyboard?.hide() },
                            )

                            if (noSearchResult) {
                                NoSearchResult(term = applied, onClear = { query = "" })
                            } else {
                                LazyColumn(
                                    state = listState,
                                    modifier = Modifier.fillMaxWidth(),
                                    verticalArrangement =
                                        Arrangement.spacedBy(Dimens.listItemDivider),
                                ) {
                                    itemsIndexed(rows, key = { _, row -> row.key }) { _, row ->
                                        PickerRowContent(
                                            row = row,
                                            inFlight = inFlight,
                                            networkStats = statsSnapshot,
                                            networkStatsStale = statsStale,
                                            menuOpen = rowMenuFor == row.key,
                                            actions = actions,
                                        )
                                    }
                                }
                            }
                        }
                }
            }
        }
    }

    newListFor?.let { location ->
        NewListDialog(
            takenNames = customLists.keys,
            onDismiss = { newListFor = null },
            onCreate = { listName ->
                newListFor = null
                toggleList(listName, location)
            },
        )
    }

    renameListFor?.let { oldName ->
        RenameListDialog(
            currentName = oldName,
            onDismiss = { renameListFor = null },
            onRename = { newName ->
                settings.renameCustomList(oldName, newName)
                if (oldName in expandedLists) expandedLists = expandedLists - oldName + newName
                renameListFor = null
            },
        )
    }

    deleteListFor?.let { name ->
        NegativeConfirmationDialog(
            message = stringResource(R.string.location_delete_list_confirm, name),
            confirmationText = stringResource(R.string.location_delete_list),
            onConfirm = {
                settings.deleteCustomList(name)
                deleteListFor = null
            },
            onBack = { deleteListFor = null },
        )
    }

    if (confirmClearRecents) {
        NegativeConfirmationDialog(
            message = stringResource(R.string.location_clear_recents_confirm),
            confirmationText = stringResource(R.string.location_clear),
            onConfirm = {
                settings.clearRecents()
                confirmClearRecents = false
            },
            onBack = { confirmClearRecents = false },
        )
    }

    if (confirmDisableRecents) {
        NegativeConfirmationDialog(
            message = stringResource(R.string.location_disable_recents_confirm),
            confirmationText = stringResource(R.string.location_disable_recents),
            onConfirm = {
                settings.setRecentsEnabled(false)
                confirmDisableRecents = false
            },
            onBack = { confirmDisableRecents = false },
        )
    }
}

private suspend fun showRemovalUndo(
    snackbarHostState: SnackbarHostState,
    message: String,
    undoLabel: String,
    onUndo: () -> Unit,
) {
    val result = snackbarHostState.showSnackbar(
        message = message,
        actionLabel = undoLabel,
        duration = SnackbarDuration.Short,
    )
    if (result == SnackbarResult.ActionPerformed) onUndo()
}

/** The catalogue came back empty: say so plainly and offer the retry. */
@Composable
private fun EmptyCatalogue(onRetry: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        Text(
            text = stringResource(R.string.location_no_relays_available),
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
        )
        WarrenTextButton(onClick = onRetry) { Text(stringResource(R.string.retry)) }
    }
}

/** Nothing matched: the desktop's two lines plus a one-tap way back to the list. */
@Composable
private fun NoSearchResult(term: String, onClear: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        Text(
            text = stringResource(R.string.location_no_exits_match, term),
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
        )
        Text(
            text = stringResource(R.string.location_no_exits_match_hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        WarrenTextButton(onClick = onClear) {
            Text(stringResource(R.string.location_clear_search))
        }
    }
}

/**
 * Search field. No autofocus on purpose: raising the IME on entry would cover
 * the list the user came to browse.
 */
@Composable
private fun SearchField(query: String, onQueryChange: (String) -> Unit, onSearch: () -> Unit) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = Modifier.fillMaxWidth().padding(vertical = Dimens.smallPadding),
        placeholder = { Text(stringResource(R.string.location_search_hint)) },
        leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
        trailingIcon = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) {
                    Icon(
                        imageVector = Icons.Rounded.Close,
                        contentDescription = stringResource(R.string.location_clear_search),
                    )
                }
            }
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSearch() }),
    )
}

/**
 * Name a new list, which is created holding the location its menu was opened
 * on: a list exists to hold locations, so it is never created empty. A name
 * already in use is refused rather than merged.
 */
@Composable
private fun NewListDialog(
    takenNames: Set<String>,
    onDismiss: () -> Unit,
    onCreate: (String) -> Unit,
) {
    var name by rememberSaveable { mutableStateOf("") }
    val trimmed = name.trim()
    WarrenAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.location_new_list_title)) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                modifier = Modifier.fillMaxWidth(),
                label = { Text(stringResource(R.string.location_new_list_name)) },
                singleLine = true,
            )
        },
        confirmButton = {
            WarrenTextButton(
                enabled = trimmed.isNotEmpty() && trimmed !in takenNames,
                onClick = { onCreate(trimmed) },
            ) { Text(stringResource(R.string.location_create_and_add)) }
        },
        dismissButton = {
            WarrenTextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
        },
    )
}

/** Dialog to rename a custom list. */
@Composable
private fun RenameListDialog(
    currentName: String,
    onDismiss: () -> Unit,
    onRename: (String) -> Unit,
) {
    var name by rememberSaveable { mutableStateOf(currentName) }
    WarrenAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.location_rename_list_title)) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                modifier = Modifier.fillMaxWidth(),
                label = { Text(stringResource(R.string.location_new_list_name)) },
                singleLine = true,
            )
        },
        confirmButton = {
            WarrenTextButton(
                enabled = name.isNotBlank() && name.trim() != currentName,
                onClick = { onRename(name.trim()) },
            ) { Text(stringResource(R.string.location_rename_save)) }
        },
        dismissButton = {
            WarrenTextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
        },
    )
}
