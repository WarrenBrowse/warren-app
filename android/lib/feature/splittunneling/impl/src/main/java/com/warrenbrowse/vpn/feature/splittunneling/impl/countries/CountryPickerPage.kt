package com.warrenbrowse.vpn.feature.splittunneling.impl.countries

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.feature.splittunneling.impl.CardShape
import com.warrenbrowse.vpn.feature.splittunneling.impl.ChosenCheck
import com.warrenbrowse.vpn.feature.splittunneling.impl.SearchField
import com.warrenbrowse.vpn.feature.splittunneling.impl.SubPage
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.ui.component.CountryFlag
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

/**
 * The country, or a city of it, one app leaves from: the countries with an active server, each
 * unfolding to its cities. A tap chooses at once and goes back to the app's route. It only reports
 * the choice: the main connection's location is never touched here.
 */
@Composable
internal fun CountryPickerPage(state: CountryPickerUiState, actions: CountryPickerActions) {
    SubPage(
        title = stringResource(R.string.country_picker_title, state.app.name),
        scrolls = false,
        buttons = {
            PrimaryButton(
                onClick = actions.onCancel,
                text = stringResource(R.string.cancel),
                modifier = Modifier.fillMaxWidth(),
            )
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
            SearchField(query = state.searchTerm, onQueryChange = actions.onSearchChange)
            PickerBody(state, actions)
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
        else -> {
            val listState = rememberLazyListState()
            LazyColumn(
                state = listState,
                modifier =
                    Modifier.fillMaxSize()
                        .drawVerticalScrollbar(
                            state = listState,
                            color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                        ),
                verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
            ) {
                state.options.forEach { option -> countryOption(option, state, actions) }
            }
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
        ExitOptionRow(
            title = option.name,
            exit = AppExit(option.country),
            state = state,
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
            flag = false,
            onChoose = actions.onChoose,
            trailing = null,
            modifier = Modifier.padding(start = Dimens.largePadding),
        )
    }
}

/**
 * `[flag] name ...... [In use] [check] [cities]`. The chosen exit has a green border and a check,
 * an exit another app already leaves from says "In use".
 */
@Suppress("LongParameterList")
@Composable
private fun ExitOptionRow(
    title: String,
    exit: AppExit,
    state: CountryPickerUiState,
    flag: Boolean,
    onChoose: (AppExit) -> Unit,
    trailing: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val selected = state.current == exit
    val inUse = !selected && exit in state.exitsInUse
    Row(
        modifier =
            modifier
                .fillMaxWidth()
                .heightIn(min = RowMinHeight)
                .clip(CardShape)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .border(
                    2.dp,
                    if (selected) MaterialTheme.colorScheme.positive else Color.Transparent,
                    CardShape,
                ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            modifier =
                Modifier.weight(1f)
                    .heightIn(min = RowMinHeight)
                    .selectable(selected = selected, role = Role.RadioButton) { onChoose(exit) }
                    .padding(horizontal = Dimens.cellStartPadding - 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Dimens.cellStartPadding - 2.dp),
        ) {
            if (flag) CountryFlag(countryCode = exit.country, size = Dimens.countryFlagSize)
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (inUse) {
                Text(
                    text = stringResource(R.string.country_picker_in_use),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                )
            }
            if (selected) ChosenCheck()
        }
        trailing?.invoke()
    }
}

private val RowMinHeight = 56.dp
