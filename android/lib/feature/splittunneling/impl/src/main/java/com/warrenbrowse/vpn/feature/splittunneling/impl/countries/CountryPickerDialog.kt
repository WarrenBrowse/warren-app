package com.warrenbrowse.vpn.feature.splittunneling.impl.countries

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.ui.component.CountryFlag
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemClickArea
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeButton
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

/**
 * Picks the country, or a city of it, one app leaves from: the countries with an active server,
 * each unfolding to its cities, with the flags and rows of the location list. A tap chooses at
 * once and closes the picker. It only reports the choice: the main connection's location is never
 * touched here.
 */
@Composable
internal fun CountryPickerDialog(state: CountryPickerUiState, actions: CountryPickerActions) {
    Dialog(
        onDismissRequest = actions.onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Surface(
            shape = RoundedCornerShape(Dimens.mediumPadding),
            color = MaterialTheme.colorScheme.surface,
            modifier = Modifier.fillMaxWidth(DIALOG_WIDTH).fillMaxHeight(DIALOG_HEIGHT),
        ) {
            Column(modifier = Modifier.padding(Dimens.mediumPadding)) {
                Text(
                    text = stringResource(R.string.country_picker_title, state.app.name),
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(bottom = Dimens.smallPadding),
                )
                SearchField(query = state.searchTerm, onQueryChange = actions.onSearchChange)
                Box(modifier = Modifier.weight(1f).fillMaxWidth()) { PickerBody(state, actions) }
                PickerButtons(hasCountry = state.current != null, actions = actions)
            }
        }
    }
}

@Composable
private fun PickerBody(state: CountryPickerUiState, actions: CountryPickerActions) {
    when {
        !state.catalogueLoaded ->
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                WarrenCircularProgressIndicatorLarge()
            }
        state.options.isEmpty() ->
            Text(
                text = stringResource(R.string.location_no_exits_match_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().padding(vertical = Dimens.mediumPadding),
            )
        else ->
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(Dimens.listItemDivider),
            ) {
                state.options.forEach { option -> countryOption(option, state, actions) }
            }
    }
}

private fun LazyListScope.countryOption(
    option: CountryOption,
    state: CountryPickerUiState,
    actions: CountryPickerActions,
) {
    val showCities = option.cities.size > 1
    val expanded = showCities && option.country in state.expanded
    item(key = option.country) {
        val exit = AppExit(option.country)
        ExitOptionRow(
            title = option.name,
            exit = exit,
            state = state,
            position = if (expanded) Position.Top else Position.Single,
            hierarchy = Hierarchy.Parent,
            flag = true,
            onChoose = actions.onChoose,
            trailing =
                if (showCities) {
                    {
                        val label = stringResource(R.string.country_picker_cities_in, option.name)
                        IconButton(
                            onClick = { actions.onToggleCountry(option.country) },
                            modifier = Modifier.semantics { contentDescription = label },
                        ) {
                            ExpandChevron(isExpanded = expanded)
                        }
                    }
                } else {
                    null
                },
        )
    }
    if (!expanded) return
    items(items = option.cities, key = { city -> "${option.country}/$city" }) { city ->
        ExitOptionRow(
            title = city,
            exit = AppExit(option.country, city),
            state = state,
            position = if (city == option.cities.last()) Position.Bottom else Position.Middle,
            hierarchy = Hierarchy.Child1,
            flag = false,
            onChoose = actions.onChoose,
            trailing = null,
        )
    }
}

/**
 * `[flag] name ...... [In use] [check] [chevron]`. The chosen exit is marked by a check, an exit
 * another app already leaves from by "In use".
 */
@Composable
private fun ExitOptionRow(
    title: String,
    exit: AppExit,
    state: CountryPickerUiState,
    position: Position,
    hierarchy: Hierarchy,
    flag: Boolean,
    onChoose: (AppExit) -> Unit,
    trailing: (@Composable () -> Unit)?,
) {
    val selected = state.current == exit
    val inUse = !selected && exit in state.exitsInUse
    WarrenListItem(
        position = position,
        hierarchy = hierarchy,
        isSelected = selected,
        mainClickArea =
            if (trailing == null) ListItemClickArea.All else ListItemClickArea.LeadingAndMain,
        onClick = { onChoose(exit) },
        modifier = Modifier.semantics { this.selected = selected },
        content = {
            Row(
                modifier =
                    Modifier.align(Alignment.CenterStart)
                        .fillMaxWidth()
                        .padding(vertical = Dimens.smallPadding),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (flag) {
                    CountryFlag(countryCode = exit.country, size = Dimens.countryFlagSize)
                    Box(Modifier.width(Dimens.mediumPadding))
                }
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color =
                        if (selected) MaterialTheme.colorScheme.positive
                        else MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                if (inUse) {
                    Text(
                        text = stringResource(R.string.country_picker_in_use),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                        modifier = Modifier.padding(horizontal = Dimens.smallPadding),
                    )
                }
                if (selected) {
                    Icon(
                        imageVector = Icons.Rounded.Check,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.positive,
                        modifier =
                            Modifier.padding(horizontal = Dimens.smallPadding)
                                .size(Dimens.smallIconSize),
                    )
                }
            }
        },
        trailingContent = trailing?.let { control -> { Box(Modifier.align(Alignment.Center)) { control() } } },
    )
}

@Composable
private fun PickerButtons(hasCountry: Boolean, actions: CountryPickerActions) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
        verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
    ) {
        if (hasCountry) {
            NegativeButton(
                onClick = actions.onRemove,
                text = stringResource(R.string.country_picker_remove),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        PrimaryButton(
            onClick = actions.onDismiss,
            text = stringResource(R.string.cancel),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

private const val DIALOG_WIDTH = 0.92f
private const val DIALOG_HEIGHT = 0.85f
