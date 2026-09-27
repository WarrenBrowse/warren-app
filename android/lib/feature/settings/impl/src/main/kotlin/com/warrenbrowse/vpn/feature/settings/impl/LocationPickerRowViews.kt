package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.List
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.lib.model.WarrenNetworkStats
import com.warrenbrowse.vpn.lib.model.loadBadgeOf
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.ui.component.CountryFlag
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.component.networkstats.ExitLoadBadge
import com.warrenbrowse.vpn.lib.ui.component.relaylist.InactiveRelayIndicator
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.ListHeader
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemClickArea
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemDefaults
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

/** Fade between the loading, empty and populated catalogue, and between rows. */
internal const val PICKER_FADE_MS = 250

/** The selection ring around a flag: thin, and clear of the flag's own hairline. */
private val SelectionRingWidth = 2.dp
private val SelectionRingGap = 2.dp

/** The "more" glyph is smaller than a standard icon so a column of them stays quiet. */
private val MoreIconSize = 20.dp

/** Depth of a row in the accordion, expressed as the design-system hierarchy. */
private fun hierarchyOf(depth: Int): Hierarchy = when (depth) {
    0 -> Hierarchy.Parent
    1 -> Hierarchy.Child1
    else -> Hierarchy.Child2
}

/**
 * Keeps the location a dialog was opened on across a rotation. Saved as its
 * kind and fields, the only form a bundle takes.
 */
internal val LocationSaver: Saver<ExitPin?, Any> =
    listSaver(
        save = { pin ->
            when (pin) {
                null, ExitPin.Automatic -> emptyList()
                is ExitPin.Country -> listOf("country", pin.country)
                is ExitPin.City -> listOf("city", pin.country, pin.city)
                is ExitPin.Exit -> listOf("exit", pin.exitId)
            }
        },
        restore = { saved ->
            when (saved.firstOrNull()) {
                "country" -> ExitPin.Country(saved[1])
                "city" -> ExitPin.City(saved[1], saved[2])
                "exit" -> ExitPin.Exit(saved[1])
                else -> null
            }
        },
    )

/** Everything a row can ask of the screen. */
internal class PickerActions(
    val onApplyPin: (ExitPin) -> Unit,
    val onEntryPick: (String?) -> Unit,
    val onClearRecents: () -> Unit,
    /** Opens the menu of the row with this key, or closes any with `null`. */
    val onRowMenu: (String?) -> Unit,
    val expand: ExpandActions,
    val lists: ListActions,
)

/** Folding and unfolding the accordion's countries, cities and lists. */
internal class ExpandActions(
    val onToggleCountry: (String) -> Unit,
    val onToggleCity: (String) -> Unit,
    val onToggleList: (String) -> Unit,
)

/** What the lists menus do: membership of a location, and a list's own rename and delete. */
internal class ListActions(
    val memberships: (ExitPin) -> List<ListMembership>,
    val onToggleMembership: (String, ExitPin) -> Unit,
    val onNewList: (ExitPin) -> Unit,
    val onRenameList: (String) -> Unit,
    val onDeleteList: (String) -> Unit,
)

@Composable
internal fun LazyItemScope.PickerRowContent(
    row: PickerRow,
    inFlight: Boolean,
    networkStats: WarrenNetworkStats?,
    networkStatsStale: Boolean,
    menuOpen: Boolean,
    actions: PickerActions,
) {
    // Rows enter, leave and slide on one clock, so expanding a country unfolds
    // its children instead of snapping them in.
    val itemModifier = Modifier.animateItem(
        fadeInSpec = tween(PICKER_FADE_MS),
        placementSpec = tween(PICKER_FADE_MS),
        fadeOutSpec = tween(PICKER_FADE_MS),
    )
    when (row) {
        is PickerRow.Gap -> Spacer(modifier = itemModifier.height(Dimens.mediumPadding))

        is PickerRow.RecentsHeader,
        is PickerRow.CustomListsHeader,
        is PickerRow.AllLocationsHeader ->
            PickerSectionHeader(row = row, modifier = itemModifier, onClearRecents = actions.onClearRecents)

        is PickerRow.ExitAutomaticRow,
        is PickerRow.EntryAutomaticRow,
        is PickerRow.EntryCountryRow -> ScopeRowCell(row, itemModifier, inFlight, actions)

        is PickerRow.CustomListRow -> CustomListCell(row, itemModifier, menuOpen, actions)

        is PickerRow.CountryHeader,
        is PickerRow.CityHeader,
        is PickerRow.ExitRow,
        is PickerRow.SavedRow ->
            LocationRowCell(
                row = row,
                modifier = itemModifier,
                inFlight = inFlight,
                loadBadge = LoadBadgeSource(networkStats, networkStatsStale),
                menuOpen = menuOpen,
                actions = actions,
            )
    }
}

/** The network figures a row reads its load badge from. */
private class LoadBadgeSource(val stats: WarrenNetworkStats?, val stale: Boolean) {
    /** The badge of [exitId], or `null` while there is no exit or no figure for it. */
    fun badgeFor(exitId: String?): (@Composable () -> Unit)? {
        val snapshot = stats
        val exitStats = if (snapshot != null && exitId != null) snapshot.exit(exitId) else null
        return if (snapshot != null && exitStats != null) {
            { ExitLoadBadge(badge = snapshot.loadBadgeOf(exitStats), stale = stale) }
        } else {
            null
        }
    }
}

/** The rows choosing a scope rather than a place: Automatic on either hop, and an entry country. */
@Composable
private fun ScopeRowCell(row: PickerRow, modifier: Modifier, inFlight: Boolean, actions: PickerActions) {
    when (row) {
        is PickerRow.ExitAutomaticRow ->
            LocationCell(
                modifier = modifier,
                title = stringResource(R.string.automatic),
                subtitle = stringResource(R.string.location_automatic_description),
                leading = LeadingSlot.Glyph { AutomaticGlyph() },
                position = row.position,
                selected = row.isPinned,
                isEnabled = !inFlight,
                onClick = dropUnlessResumed { actions.onApplyPin(ExitPin.Automatic) },
            )
        is PickerRow.EntryAutomaticRow ->
            LocationCell(
                modifier = modifier,
                title = stringResource(R.string.automatic),
                subtitle = stringResource(R.string.location_entry_automatic_description),
                leading = LeadingSlot.Glyph { AutomaticGlyph() },
                position = row.position,
                selected = row.isPinned,
                isEnabled = !inFlight,
                onClick = { actions.onEntryPick(null) },
            )
        is PickerRow.EntryCountryRow ->
            LocationCell(
                modifier = modifier,
                title = row.display,
                leading = LeadingSlot.Flag(row.country),
                selected = row.isPinned,
                isEnabled = !inFlight,
                position = row.position,
                onClick = { actions.onEntryPick(row.country) },
            )
        else -> Unit
    }
}

/**
 * Every geographical row, wherever it sits: tapping selects it, and its
 * "more" button or a long press opens its lists menu. Countries and cities
 * also carry their expand chevron.
 */
@Composable
@Suppress("LongParameterList")
private fun LocationRowCell(
    row: PickerRow,
    modifier: Modifier,
    inFlight: Boolean,
    loadBadge: LoadBadgeSource,
    menuOpen: Boolean,
    actions: PickerActions,
) {
    val location = locationOf(row) ?: return
    val openMenu = { actions.onRowMenu(row.key) }
    val listsMenu: @Composable () -> Unit = { RowListsMenu(location, row.key, menuOpen, actions) }
    val onClick = dropUnlessResumed { actions.onApplyPin(location) }
    when (row) {
        is PickerRow.CountryHeader ->
            LocationCell(
                modifier = modifier,
                title = row.display.ifBlank { stringResource(R.string.location_unknown_country) },
                leading = LeadingSlot.Flag(row.country),
                selected = row.isPinned,
                isEnabled = row.hasActive && !inFlight,
                position = row.position,
                onClick = onClick,
                onLongClick = openMenu,
                trailing = {
                    listsMenu()
                    ExpandButton(row.expanded) { actions.expand.onToggleCountry(row.country) }
                },
            )
        is PickerRow.CityHeader ->
            LocationCell(
                modifier = modifier,
                title = row.city,
                selected = row.isPinned,
                isEnabled = row.hasActive && !inFlight,
                position = row.position,
                hierarchy = hierarchyOf(row.depth),
                onClick = onClick,
                onLongClick = openMenu,
                trailing = {
                    listsMenu()
                    ExpandButton(row.expanded) {
                        actions.expand.onToggleCity(cityKey(row.country, row.city))
                    }
                },
            )
        is PickerRow.ExitRow ->
            LocationCell(
                modifier = modifier,
                title = exitLabel(row.title, row.ordinal),
                subtitle = row.subtitle,
                leading = row.flagCountry?.let { LeadingSlot.Flag(it) },
                badge = loadBadge.badgeFor(row.relay.exitId),
                selected = row.isPinned,
                inactive = !row.relay.active,
                isEnabled = row.relay.active && !inFlight,
                position = row.position,
                hierarchy = hierarchyOf(row.depth),
                onClick = onClick,
                onLongClick = openMenu,
                trailing = listsMenu,
            )
        is PickerRow.SavedRow ->
            LocationCell(
                modifier = modifier,
                title = exitLabel(row.place.title, row.place.ordinal),
                subtitle = row.place.subtitle,
                leading = row.place.flagCountry?.let { LeadingSlot.Flag(it) },
                badge = loadBadge.badgeFor(row.place.loadExitId),
                selected = row.isPinned,
                inactive = !row.place.hasActive,
                isEnabled = row.place.hasActive && !inFlight,
                position = row.position,
                hierarchy = hierarchyOf(row.depth),
                onClick = onClick,
                onLongClick = openMenu,
                trailing = listsMenu,
            )
        else -> Unit
    }
}

/** The lists menu of the row keyed [rowKey], standing for [location]. */
@Composable
private fun RowListsMenu(location: ExitPin, rowKey: String, menuOpen: Boolean, actions: PickerActions) {
    ListsMenuButton(
        expanded = menuOpen,
        memberships = if (menuOpen) actions.lists.memberships(location) else emptyList(),
        onOpen = { actions.onRowMenu(rowKey) },
        onDismiss = { actions.onRowMenu(null) },
        onToggle = { listName -> actions.lists.onToggleMembership(listName, location) },
        onNewList = { actions.lists.onNewList(location) },
    )
}

/** The location a geographical row stands for, which a tap selects and its menu files. */
private fun locationOf(row: PickerRow): ExitPin? =
    when (row) {
        is PickerRow.CountryHeader -> row.location
        is PickerRow.CityHeader -> row.location
        is PickerRow.ExitRow -> row.location
        is PickerRow.SavedRow -> row.place.location
        else -> null
    }

/** A custom list: tapping folds it, its own menu renames or deletes it. */
@Composable
private fun CustomListCell(
    row: PickerRow.CustomListRow,
    modifier: Modifier,
    menuOpen: Boolean,
    actions: PickerActions,
) {
    val closeMenu = { actions.onRowMenu(null) }
    LocationCell(
        modifier = modifier,
        title = row.name,
        subtitle = pluralStringResource(R.plurals.location_list_count, row.count, row.count),
        leading = LeadingSlot.Glyph { ListGlyph() },
        selected = false,
        isEnabled = true,
        position = row.position,
        onClick = { actions.expand.onToggleList(row.name) },
        onLongClick = { actions.onRowMenu(row.key) },
        trailing = {
            ListOptionsButton(
                expanded = menuOpen,
                onOpen = { actions.onRowMenu(row.key) },
                onDismiss = closeMenu,
                onRename = {
                    closeMenu()
                    actions.lists.onRenameList(row.name)
                },
                onDelete = {
                    closeMenu()
                    actions.lists.onDeleteList(row.name)
                },
            )
            ExpandButton(row.expanded) { actions.expand.onToggleList(row.name) }
        },
    )
}

/** An exit's label: its place, numbered when its city holds several exits. */
@Composable
private fun exitLabel(title: String, ordinal: Int?): String =
    if (ordinal == null) title else stringResource(R.string.location_exit_ordinal, title, ordinal)

@Composable
private fun PickerSectionHeader(row: PickerRow, modifier: Modifier, onClearRecents: () -> Unit) {
    // [ListHeader] sizes itself from its own intrinsics, so the item animation
    // is carried by this box instead of being appended to that chain.
    Box(modifier = modifier.fillMaxWidth()) {
        when (row) {
            is PickerRow.RecentsHeader ->
                ListHeader(
                    content = { Text(stringResource(R.string.location_recents)) },
                    actions = {
                        WarrenTextButton(onClick = onClearRecents) {
                            Text(stringResource(R.string.location_clear))
                        }
                    },
                )
            is PickerRow.CustomListsHeader ->
                ListHeader(content = { Text(stringResource(R.string.location_custom_lists)) })
            else ->
                ListHeader(content = { Text(stringResource(R.string.location_all_locations)) })
        }
    }
}

/** The quiet trigger every row menu hangs from: a full-size target with a small muted glyph. */
@Composable
private fun MoreButton(onClick: () -> Unit) {
    IconButton(onClick = onClick) {
        Icon(
            imageVector = Icons.Rounded.MoreVert,
            contentDescription = stringResource(R.string.location_row_options),
            tint = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40),
            modifier = Modifier.size(MoreIconSize),
        )
    }
}

/**
 * A location's lists menu: every list, checked when it holds the location, a
 * tap adding or removing it at once; then "New list…", which starts a list
 * around the location.
 */
@Composable
@Suppress("LongParameterList")
private fun ListsMenuButton(
    expanded: Boolean,
    memberships: List<ListMembership>,
    onOpen: () -> Unit,
    onDismiss: () -> Unit,
    onToggle: (String) -> Unit,
    onNewList: () -> Unit,
) {
    Box {
        MoreButton(onClick = onOpen)
        DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
            memberships.forEach { membership ->
                DropdownMenuItem(
                    text = {
                        Text(membership.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    },
                    leadingIcon = { MembershipCheck(membership.contains) },
                    onClick = { onToggle(membership.name) },
                )
            }
            if (memberships.isNotEmpty()) HorizontalDivider()
            DropdownMenuItem(
                text = { Text(stringResource(R.string.location_new_list)) },
                leadingIcon = { Icon(Icons.Rounded.Add, contentDescription = null) },
                onClick = onNewList,
            )
        }
    }
}

/** A list's membership mark: a check, or an empty slot of the same width so names line up. */
@Composable
private fun MembershipCheck(contains: Boolean) {
    if (contains) {
        Icon(
            imageVector = Icons.Rounded.Check,
            contentDescription = stringResource(R.string.location_in_list),
            tint = MaterialTheme.colorScheme.positive,
        )
    } else {
        Spacer(Modifier.size(Dimens.smallIconSize + Dimens.smallPadding))
    }
}

/** A custom list row's own menu: rename and delete. */
@Composable
private fun ListOptionsButton(
    expanded: Boolean,
    onOpen: () -> Unit,
    onDismiss: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
) {
    Box {
        MoreButton(onClick = onOpen)
        DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.location_rename_list)) },
                onClick = onRename,
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.location_delete_list)) },
                onClick = onDelete,
            )
        }
    }
}

/** Entry / Exit hop selector, mirroring the desktop scope bar. */
@Composable
internal fun HopScopeBar(scope: PickerScope, onScopeChange: (PickerScope) -> Unit) {
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

/** What leads a row, in the flag slot. */
private sealed interface LeadingSlot {
    /** A country flag, which carries the selection as a green ring. */
    data class Flag(val countryCode: String) : LeadingSlot

    /** A glyph (Automatic, a list): the selection stays a check after the name. */
    data class Glyph(val content: @Composable () -> Unit) : LeadingSlot
}

/**
 * The round flag leading a row. Selected, it is circled by a thin ring in the
 * accent, drawn outside its bounds so a selected row keeps every edge of an
 * unselected one.
 */
@Composable
private fun RowFlag(countryCode: String, selected: Boolean) {
    val ring = MaterialTheme.colorScheme.positive
    val modifier = if (selected) {
        Modifier.drawBehind {
            val stroke = SelectionRingWidth.toPx()
            drawCircle(
                color = ring,
                radius = size.minDimension / 2 + SelectionRingGap.toPx() + stroke / 2,
                style = Stroke(width = stroke),
            )
        }
    } else {
        Modifier
    }
    CountryFlag(countryCode = countryCode, size = Dimens.countryFlagSize, modifier = modifier)
}

/** The Automatic row's glyph, in the flag slot so every top-level label lines up. */
@Composable
private fun AutomaticGlyph() {
    Icon(
        imageVector = Icons.Rounded.Public,
        contentDescription = null,
        tint = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
        modifier = Modifier.size(Dimens.countryFlagSize),
    )
}

/** A custom list's glyph, in the flag slot. */
@Composable
private fun ListGlyph() {
    Icon(
        imageVector = Icons.AutoMirrored.Rounded.List,
        contentDescription = null,
        tint = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
        modifier = Modifier.size(Dimens.countryFlagSize),
    )
}

/** The only expand target of a country, city or list row besides a list's own label. */
@Composable
private fun ExpandButton(expanded: Boolean, onToggle: () -> Unit) {
    IconButton(onClick = onToggle) { ExpandChevron(isExpanded = expanded) }
}

/**
 * Every picker row: `[flag] name / muted subtitle ...... [load badge] [more]`.
 * The selection shows as a green name and, on a row led by a flag, a green
 * ring around it; a row without a flag gets a small check after its name
 * instead. Depth comes from the design-system [Hierarchy], so every row keeps
 * the same edges whatever its level. With a [trailing] control the label area
 * alone selects, so the control never doubles as a hidden selection.
 *
 * An [inactive] row keeps a red dot that explains why it takes no tap, so a
 * node that is down cannot become the selection.
 */
/** The flag slot, followed by the gap to the name when it holds anything. */
@Composable
private fun LeadingContent(leading: LeadingSlot?, selected: Boolean) {
    when (leading) {
        is LeadingSlot.Flag -> RowFlag(leading.countryCode, selected)
        is LeadingSlot.Glyph -> leading.content()
        null -> return
    }
    Spacer(Modifier.width(Dimens.mediumPadding))
}

/** The name, with the selection check when [check] gives its tint, over a muted subtitle. */
@Composable
private fun RowScope.LocationLabel(title: String, subtitle: String?, labelColor: Color, check: Color?) {
    Column(modifier = Modifier.weight(1f)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = labelColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            if (check != null) {
                Icon(
                    imageVector = Icons.Rounded.Check,
                    contentDescription = null,
                    tint = check,
                    modifier = Modifier.padding(start = Dimens.tinyPadding).size(Dimens.smallIconSize),
                )
            }
        }
        if (subtitle != null) {
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
@Suppress("LongParameterList", "LongMethod")
private fun LocationCell(
    title: String,
    selected: Boolean,
    isEnabled: Boolean,
    position: Position?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    hierarchy: Hierarchy = Hierarchy.Parent,
    subtitle: String? = null,
    leading: LeadingSlot? = null,
    badge: (@Composable () -> Unit)? = null,
    inactive: Boolean = false,
    onLongClick: (() -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    val colors = ListItemDefaults.colors()
    val labelColor = colors.headlineColor(enabled = isEnabled, selected = selected)
    WarrenListItem(
        modifier = modifier,
        position = position ?: Position.Single,
        hierarchy = hierarchy,
        isSelected = selected,
        isEnabled = isEnabled,
        mainClickArea = if (trailing == null) {
            ListItemClickArea.All
        } else {
            ListItemClickArea.LeadingAndMain
        },
        onClick = if (isEnabled) onClick else null,
        onLongClick = if (isEnabled) onLongClick else null,
        colors = colors,
        content = {
            Row(
                modifier = Modifier
                    .align(Alignment.CenterStart)
                    .fillMaxWidth()
                    .padding(vertical = Dimens.smallPadding),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                LeadingContent(leading, selected)
                LocationLabel(
                    title = title,
                    subtitle = subtitle,
                    labelColor = labelColor,
                    // A flag carries the selection as its ring; anything else gets the check.
                    check = when {
                        !selected || leading is LeadingSlot.Flag -> null
                        inactive -> MaterialTheme.colorScheme.error
                        else -> labelColor
                    },
                )
                if (inactive && !selected) {
                    InactiveRelayIndicator(
                        modifier = Modifier.padding(start = Dimens.smallPadding),
                        tint = MaterialTheme.colorScheme.error,
                    )
                }
                if (badge != null) {
                    Spacer(Modifier.width(Dimens.smallPadding))
                    badge()
                }
                if (trailing == null) Spacer(Modifier.width(Dimens.mediumPadding))
            }
        },
        trailingContent = trailing?.let { controls ->
            {
                Row(
                    modifier = Modifier.align(Alignment.Center),
                    verticalAlignment = Alignment.CenterVertically,
                ) { controls() }
            }
        },
    )
}
