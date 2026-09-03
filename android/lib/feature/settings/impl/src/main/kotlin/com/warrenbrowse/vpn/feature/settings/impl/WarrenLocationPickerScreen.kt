package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
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
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.ConnectAfterLocationPick
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.relaylist.InactiveRelayIndicator
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenAlertDialog
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.ListHeader
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemClickArea
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemDefaults
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/** Fade between the loading, empty and populated catalogue, and between rows. */
private const val PICKER_FADE_MS = 250

/** Depth of a row in the accordion, expressed as the design-system hierarchy. */
private fun hierarchyOf(depth: Int): Hierarchy = when (depth) {
    0 -> Hierarchy.Parent
    1 -> Hierarchy.Child1
    else -> Hierarchy.Child2
}

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
 * Warren-specific extras are kept: recents (behind the top-bar overflow menu),
 * and custom lists, reachable from each row's own menu.
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
    // mid-decision reopens on the same list or exit instead of dropping it.
    var addToListForExitId by rememberSaveable { mutableStateOf<String?>(null) }
    var renameListFor by rememberSaveable { mutableStateOf<String?>(null) }
    var deleteListFor by rememberSaveable { mutableStateOf<String?>(null) }
    var confirmClearRecents by rememberSaveable { mutableStateOf(false) }
    var confirmDisableRecents by rememberSaveable { mutableStateOf(false) }
    var rowMenuFor by rememberSaveable { mutableStateOf<String?>(null) }
    var overflowMenuOpen by rememberSaveable { mutableStateOf(false) }

    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val keyboard = LocalSoftwareKeyboardController.current
    val removedMessage = stringResource(R.string.location_removed_from_list)
    val undoLabel = stringResource(R.string.undo)

    // Which hop the list is picking. Only reachable while multi-hop is on, so
    // turning it off from another screen collapses the picker back to the exit.
    var pickerScope by rememberSaveable { mutableStateOf(PickerScope.Exit) }
    val activeScope = if (multiHopEnabled) pickerScope else PickerScope.Exit

    var expandedCountries by
        rememberSaveable(stateSaver = ExpandedKeySaver) { mutableStateOf(emptySet<String>()) }
    var expandedCities by
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
            expanded = ExpandedKeys(expandedCountries, expandedCities),
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
                                            onApplyPin = applyPin,
                                            onEntryPick = { country ->
                                                pickerScope = applyEntryPick(
                                                    country,
                                                    settings::setEntryCountry,
                                                )
                                            },
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
                                                expandedCities = if (key in expandedCities) {
                                                    expandedCities - key
                                                } else {
                                                    expandedCities + key
                                                }
                                            },
                                            onClearRecents = { confirmClearRecents = true },
                                            onRenameList = { renameListFor = it },
                                            onDeleteList = { deleteListFor = it },
                                            rowMenuFor = rowMenuFor,
                                            onRowMenu = { rowMenuFor = it },
                                            onAddToList = { addToListForExitId = it },
                                            onRemoveFromList = { listName, exitId ->
                                                settings.removeExitFromCustomList(
                                                    listName,
                                                    exitId,
                                                )
                                                scope.launch {
                                                    showRemovalUndo(
                                                        snackbarHostState = snackbarHostState,
                                                        message = removedMessage,
                                                        undoLabel = undoLabel,
                                                        onUndo = {
                                                            settings.addExitToCustomList(
                                                                listName,
                                                                exitId,
                                                            )
                                                        },
                                                    )
                                                }
                                            },
                                        )
                                    }
                                }
                            }
                        }
                }
            }
        }
    }

    addToListForExitId?.let { exitId ->
        val relay = relays.firstOrNull { it.exitId == exitId }
        if (relay == null) {
            addToListForExitId = null
        } else {
            AddToListDialog(
                listNames = customLists.keys.toList(),
                onDismiss = { addToListForExitId = null },
                onPick = { listName ->
                    settings.addExitToCustomList(listName, relay.exitId)
                    addToListForExitId = null
                },
            )
        }
    }

    renameListFor?.let { oldName ->
        RenameListDialog(
            currentName = oldName,
            onDismiss = { renameListFor = null },
            onRename = { newName ->
                settings.renameCustomList(oldName, newName)
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

@Composable
@Suppress("LongParameterList", "LongMethod")
private fun LazyItemScope.PickerRowContent(
    row: PickerRow,
    inFlight: Boolean,
    onApplyPin: (ExitPin) -> Unit,
    onEntryPick: (String?) -> Unit,
    onToggleCountry: (String) -> Unit,
    onToggleCity: (String) -> Unit,
    onClearRecents: () -> Unit,
    onRenameList: (String) -> Unit,
    onDeleteList: (String) -> Unit,
    rowMenuFor: String?,
    onRowMenu: (String?) -> Unit,
    onAddToList: (String) -> Unit,
    onRemoveFromList: (String, String) -> Unit,
) {
    val itemModifier = Modifier.animateItem(
        fadeInSpec = tween(PICKER_FADE_MS),
        placementSpec = tween(PICKER_FADE_MS),
        fadeOutSpec = tween(PICKER_FADE_MS),
    )
    when (row) {
        is PickerRow.Gap -> Spacer(modifier = itemModifier.height(Dimens.mediumPadding))

        is PickerRow.RecentsHeader ->
            SectionHeader(modifier = itemModifier) {
                ListHeader(
                    content = { Text(stringResource(R.string.location_recents)) },
                    actions = {
                        WarrenTextButton(onClick = onClearRecents) {
                            Text(stringResource(R.string.location_clear))
                        }
                    },
                )
            }

        is PickerRow.CustomListsHeader ->
            SectionHeader(modifier = itemModifier) {
                ListHeader(content = { Text(stringResource(R.string.location_custom_lists)) })
            }

        is PickerRow.CustomListsEmptyHint ->
            Text(
                text = stringResource(R.string.location_custom_lists_empty_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = itemModifier.padding(bottom = Dimens.smallPadding),
            )

        is PickerRow.AllLocationsHeader ->
            SectionHeader(modifier = itemModifier) {
                ListHeader(content = { Text(stringResource(R.string.location_all_locations)) })
            }

        is PickerRow.CustomListHeader ->
            SectionHeader(modifier = itemModifier) {
                ListHeader(
                    content = { Text(row.name) },
                    actions = {
                        WarrenTextButton(onClick = { onRenameList(row.name) }) {
                            Text(stringResource(R.string.location_rename_list))
                        }
                        WarrenTextButton(onClick = { onDeleteList(row.name) }) {
                            Text(stringResource(R.string.location_delete_list))
                        }
                    },
                )
            }

        is PickerRow.ExitAutomaticRow ->
            AutomaticCell(
                modifier = itemModifier,
                subtitle = stringResource(R.string.location_automatic_description),
                position = row.position,
                selected = row.isPinned,
                isEnabled = !inFlight,
                onClick = dropUnlessResumed { onApplyPin(ExitPin.Automatic) },
            )

        is PickerRow.EntryAutomaticRow ->
            AutomaticCell(
                modifier = itemModifier,
                subtitle = stringResource(R.string.location_entry_automatic_description),
                position = row.position,
                selected = row.isPinned,
                isEnabled = !inFlight,
                onClick = { onEntryPick(null) },
            )

        is PickerRow.EntryCountryRow ->
            ExitCell(
                modifier = itemModifier,
                title = row.display,
                selected = row.isPinned,
                isEnabled = !inFlight,
                position = row.position,
                hierarchy = Hierarchy.Parent,
                onClick = { onEntryPick(row.country) },
            )

        is PickerRow.CountryHeader ->
            AccordionHeader(
                modifier = itemModifier,
                title = row.display,
                expanded = row.expanded,
                selected = row.isPinned,
                isEnabled = row.hasActive && !inFlight,
                position = row.position,
                hierarchy = Hierarchy.Parent,
                onSelect = dropUnlessResumed { onApplyPin(ExitPin.Country(row.country)) },
                onToggleExpand = { onToggleCountry(row.country) },
            )

        is PickerRow.CityHeader ->
            AccordionHeader(
                modifier = itemModifier,
                title = row.city,
                expanded = row.expanded,
                selected = row.isPinned,
                isEnabled = row.hasActive && !inFlight,
                position = row.position,
                hierarchy = hierarchyOf(row.depth),
                onSelect = dropUnlessResumed { onApplyPin(ExitPin.City(row.country, row.city)) },
                onToggleExpand = { onToggleCity(cityKey(row.country, row.city)) },
            )

        is PickerRow.RecentScopeRow ->
            ExitCell(
                modifier = itemModifier,
                title = row.title,
                selected = row.isPinned,
                active = row.hasActive,
                isEnabled = row.hasActive && !inFlight,
                position = row.position,
                hierarchy = Hierarchy.Parent,
                onClick = dropUnlessResumed { onApplyPin(row.pin) },
            )

        is PickerRow.ExitRow -> {
            val label = if (row.ordinal == null) {
                row.title
            } else {
                stringResource(R.string.location_exit_ordinal, row.title, row.ordinal)
            }
            val section = row.section
            ExitCell(
                modifier = itemModifier,
                title = label,
                selected = row.isPinned,
                active = row.relay.active,
                isEnabled = row.relay.active && !inFlight,
                position = row.position,
                hierarchy = hierarchyOf(row.depth),
                onClick = dropUnlessResumed { onApplyPin(ExitPin.Exit(row.relay.exitId)) },
                onLongClick = {
                    if (section is ExitSection.Custom) {
                        onRemoveFromList(section.name, row.relay.exitId)
                    } else {
                        onAddToList(row.relay.exitId)
                    }
                },
                menu = {
                    RowMenu(
                        expanded = rowMenuFor == row.key,
                        inList = section as? ExitSection.Custom,
                        onOpen = { onRowMenu(row.key) },
                        onDismiss = { onRowMenu(null) },
                        onAddToList = {
                            onRowMenu(null)
                            onAddToList(row.relay.exitId)
                        },
                        onRemoveFromList = { listName ->
                            onRowMenu(null)
                            onRemoveFromList(listName, row.relay.exitId)
                        },
                    )
                },
            )
        }
    }
}

/**
 * Item wrapper for a section header. [ListHeader] sizes itself from its own
 * intrinsics, so the item animation is carried by this box instead of being
 * appended to that chain.
 */
@Composable
private fun SectionHeader(modifier: Modifier, content: @Composable () -> Unit) {
    Box(modifier = modifier.fillMaxWidth()) { content() }
}

/**
 * Per-row custom-list menu. The visible affordance the picker used to lack:
 * the same actions long-press already carried, discoverable without knowing
 * they exist.
 */
@Composable
@Suppress("LongParameterList")
private fun RowMenu(
    expanded: Boolean,
    inList: ExitSection.Custom?,
    onOpen: () -> Unit,
    onDismiss: () -> Unit,
    onAddToList: () -> Unit,
    onRemoveFromList: (String) -> Unit,
) {
    Box {
        IconButton(onClick = onOpen) {
            Icon(
                imageVector = Icons.Rounded.MoreVert,
                contentDescription = stringResource(R.string.location_row_options),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.location_add_to_list)) },
                onClick = onAddToList,
            )
            if (inList != null) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.location_remove_from_list)) },
                    onClick = { onRemoveFromList(inList.name) },
                )
            }
        }
    }
}

/** Entry / Exit hop selector, mirroring the desktop scope bar. */
@Composable
private fun HopScopeBar(scope: PickerScope, onScopeChange: (PickerScope) -> Unit) {
    SingleChoiceSegmentedButtonRow(
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.smallPadding),
    ) {
        SegmentedButton(
            selected = scope == PickerScope.Entry,
            onClick = { onScopeChange(PickerScope.Entry) },
            shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
        ) { Text(stringResource(R.string.location_scope_entry)) }
        SegmentedButton(
            selected = scope == PickerScope.Exit,
            onClick = { onScopeChange(PickerScope.Exit) },
            shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
        ) { Text(stringResource(R.string.location_scope_exit)) }
    }
}

/**
 * Country / city accordion header. Two hit zones, like the desktop: the label
 * area pins that scope, the trailing chevron is the only expand target. Depth
 * is the design-system [Hierarchy], so the row stays full width instead of
 * shrinking its card with every level.
 */
@Composable
@Suppress("LongParameterList")
private fun AccordionHeader(
    title: String,
    expanded: Boolean,
    selected: Boolean,
    isEnabled: Boolean,
    position: Position?,
    hierarchy: Hierarchy,
    onSelect: () -> Unit,
    onToggleExpand: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = ListItemDefaults.colors()
    WarrenListItem(
        modifier = modifier,
        position = position ?: Position.Single,
        hierarchy = hierarchy,
        isSelected = selected,
        isEnabled = isEnabled,
        mainClickArea = ListItemClickArea.LeadingAndMain,
        onClick = if (isEnabled) onSelect else null,
        colors = colors,
        leadingContent = if (selected) {
            {
                Icon(
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(end = Dimens.smallPadding),
                    imageVector = Icons.Rounded.Check,
                    contentDescription = null,
                    tint = LocalContentColor.current,
                )
            }
        } else {
            null
        },
        content = {
            Text(
                text = title.ifBlank { stringResource(R.string.location_unknown_country) },
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.align(Alignment.CenterStart),
            )
        },
        trailingContent = {
            IconButton(onClick = onToggleExpand, modifier = Modifier.align(Alignment.Center)) {
                ExpandChevron(isExpanded = expanded)
            }
        },
    )
}

/** The explicit "let Warren choose" row heading the list (desktop parity). */
@Composable
private fun AutomaticCell(
    subtitle: String,
    position: Position?,
    selected: Boolean,
    isEnabled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = ListItemDefaults.colors()
    WarrenListItem(
        modifier = modifier,
        position = position ?: Position.Single,
        isSelected = selected,
        isEnabled = isEnabled,
        onClick = if (isEnabled) onClick else null,
        colors = colors,
        leadingContent = if (selected) {
            {
                Icon(
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(end = Dimens.smallPadding),
                    imageVector = Icons.Rounded.Check,
                    contentDescription = null,
                    tint = LocalContentColor.current,
                )
            }
        } else {
            null
        },
        content = {
            Column(modifier = Modifier.align(Alignment.CenterStart)) {
                Text(
                    text = stringResource(R.string.automatic),
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
    )
}

/**
 * A single exit row: a selected check (or an inactive indicator) leading a
 * geographical label, and a trailing menu when the row can join a custom list.
 * Depth comes from the design-system [Hierarchy], so every row keeps the same
 * left and right edges whatever its level.
 *
 * An inactive exit is a disabled row: it keeps the indicator that explains why
 * but takes neither tap nor long press, so a node that is down cannot become
 * the selection.
 */
@Composable
@Suppress("LongParameterList")
private fun ExitCell(
    title: String,
    selected: Boolean,
    isEnabled: Boolean,
    position: Position?,
    hierarchy: Hierarchy,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    menu: @Composable (() -> Unit)? = null,
    active: Boolean = true,
) {
    val colors = ListItemDefaults.colors()
    val trailing: @Composable (BoxScope.() -> Unit)? = if (menu == null) {
        null
    } else {
        { Box(modifier = Modifier.align(Alignment.Center)) { menu() } }
    }
    WarrenListItem(
        modifier = modifier,
        position = position ?: Position.Single,
        hierarchy = hierarchy,
        isSelected = selected,
        isEnabled = isEnabled,
        mainClickArea = if (menu == null) {
            ListItemClickArea.All
        } else {
            ListItemClickArea.LeadingAndMain
        },
        onClick = if (isEnabled) onClick else null,
        onLongClick = if (isEnabled) onLongClick else null,
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
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = colors.headlineColor(enabled = isEnabled, selected = selected),
                modifier = Modifier.align(Alignment.CenterStart),
            )
        },
        trailingContent = trailing,
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
    var newName by rememberSaveable { mutableStateOf("") }
    WarrenAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.location_add_to_list)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                listNames.forEach { name ->
                    WarrenTextButton(
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
            WarrenTextButton(
                enabled = newName.isNotBlank(),
                onClick = { onPick(newName.trim()) },
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
