package com.warrenbrowse.vpn.feature.splittunneling.impl.countries

import android.graphics.drawable.Drawable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Android
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.RemoveCircleOutline
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import com.warrenbrowse.vpn.feature.splittunneling.impl.ContentType
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.extensions.hasValidSize
import com.warrenbrowse.vpn.feature.splittunneling.impl.extensions.isBelowMaxByteSize
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.ui.component.CountryFlag
import com.warrenbrowse.vpn.lib.ui.component.listitem.SwitchListItem
import com.warrenbrowse.vpn.lib.ui.component.text.ScreenDescription
import com.warrenbrowse.vpn.lib.ui.designsystem.ListHeader
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemClickArea
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

/** Everything the "Country per app" tab and its picker can ask of the screen. */
class CountryPerAppActions(
    val onSwitch: (Boolean) -> Unit,
    val onSearchChange: (String) -> Unit,
    val onPick: (AppData) -> Unit,
    val onClear: (PackageName) -> Unit,
    val picker: CountryPickerActions,
)

/** What a row of the picker, or its buttons, can do. */
class CountryPickerActions(
    val onSearchChange: (String) -> Unit,
    val onToggleCountry: (String) -> Unit,
    val onChoose: (AppExit) -> Unit,
    val onRemove: () -> Unit,
    val onDismiss: () -> Unit,
)

/** Actions that do nothing, for previews and tests of the other tabs. */
internal val NoCountryPerAppActions =
    CountryPerAppActions(
        onSwitch = {},
        onSearchChange = {},
        onPick = {},
        onClear = {},
        picker =
            CountryPickerActions(
                onSearchChange = {},
                onToggleCountry = {},
                onChoose = {},
                onRemove = {},
                onDismiss = {},
            ),
    )

/**
 * The tab's switch and description, then, where the device can run it, the search field and the
 * two sections. A country composes with both split modes: an app with a country is tunneled even
 * in "VPN only for", so the tab needs no note about it.
 */
internal fun LazyListScope.countryPerAppContent(
    state: CountryPerAppUiState,
    actions: CountryPerAppActions,
    onResolveIcon: (PackageName) -> Drawable?,
    systemAppsToggle: LazyListScope.() -> Unit,
) {
    item(key = CountryContentKey.SWITCH, contentType = ContentType.OTHER_ITEM) {
        SwitchListItem(
            title = stringResource(R.string.country_per_app),
            isToggled = state.enabled,
            isEnabled = state.supported,
            onCellClicked = actions.onSwitch,
            position = Position.Single,
            modifier = Modifier.animateItem(),
        )
    }
    item(key = CountryContentKey.DESCRIPTION, contentType = ContentType.DESCRIPTION) {
        Column(
            modifier =
                Modifier.animateItem()
                    .fillMaxWidth()
                    .padding(top = Dimens.smallPadding, bottom = Dimens.mediumPadding)
        ) {
            ScreenDescription(text = stringResource(R.string.country_per_app_description))
            if (!state.supported) {
                Text(
                    text = stringResource(R.string.country_per_app_needs_android_10),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.warning,
                    modifier = Modifier.padding(top = Dimens.smallPadding),
                )
            }
        }
    }
    if (!state.supported) return

    item(key = CountryContentKey.SEARCH, contentType = ContentType.OTHER_ITEM) {
        SearchField(
            query = state.searchTerm,
            onQueryChange = actions.onSearchChange,
            modifier = Modifier.animateItem(),
        )
    }
    systemAppsToggle()
    if (state.noSearchResult) {
        item(key = CountryContentKey.NO_RESULT, contentType = ContentType.DESCRIPTION) {
            ScreenDescription(
                text = stringResource(R.string.search_no_matches_for_text, state.searchTerm.trim()),
                modifier = Modifier.animateItem().padding(vertical = Dimens.mediumPadding),
            )
        }
    }
    appsWithCountry(state.withCountry, actions, onResolveIcon)
    otherApps(state.otherApps, actions, onResolveIcon)
    item(contentType = ContentType.SPACER) {
        Spacer(modifier = Modifier.animateItem().height(Dimens.cellVerticalSpacing))
    }
}

private fun LazyListScope.appsWithCountry(
    items: List<AppCountryItem>,
    actions: CountryPerAppActions,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    if (items.isEmpty()) return
    item(key = CountryContentKey.WITH_COUNTRY, contentType = ContentType.HEADER) {
        ListHeader(
            modifier = Modifier.animateItem(),
            text = stringResource(R.string.apps_with_a_country),
        )
    }
    itemsIndexed(
        items = items,
        key = { _, item -> item.app.packageName.value },
        contentType = { _, _ -> ContentType.ITEM },
    ) { index, item ->
        AppCountryRow(
            app = item.app,
            exit = item.exit,
            line = item.line,
            position = positionOf(index, items.size),
            onResolveIcon = onResolveIcon,
            onPick = { actions.onPick(item.app) },
            onClear = { actions.onClear(item.app.packageName) },
            modifier = Modifier.animateItem().padding(bottom = Dimens.listItemDivider),
        )
    }
    item(contentType = ContentType.SPACER) {
        Spacer(modifier = Modifier.animateItem().height(Dimens.cellVerticalSpacing))
    }
}

private fun LazyListScope.otherApps(
    apps: List<AppData>,
    actions: CountryPerAppActions,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    if (apps.isEmpty()) return
    item(key = CountryContentKey.OTHER_APPS, contentType = ContentType.HEADER) {
        ListHeader(modifier = Modifier.animateItem(), text = stringResource(R.string.all_apps))
    }
    itemsIndexed(
        items = apps,
        key = { _, app -> app.packageName.value },
        contentType = { _, _ -> ContentType.ITEM },
    ) { index, app ->
        AppCountryRow(
            app = app,
            exit = null,
            line = null,
            position = positionOf(index, apps.size),
            onResolveIcon = onResolveIcon,
            onPick = { actions.onPick(app) },
            onClear = null,
            modifier = Modifier.animateItem().padding(bottom = Dimens.listItemDivider),
        )
    }
}

/**
 * `[icon] name / status line ...... [country chip] [remove]`. The whole row opens the picker, as
 * the chip does, so choosing a country is one tap from anywhere on it.
 */
@Composable
private fun AppCountryRow(
    app: AppData,
    exit: AppExit?,
    line: AppRouteLine?,
    position: Position,
    onResolveIcon: (PackageName) -> Drawable?,
    onPick: () -> Unit,
    onClear: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    WarrenListItem(
        modifier = modifier,
        position = position,
        mainClickArea = ListItemClickArea.LeadingAndMain,
        onClick = onPick,
        leadingContent = {
            AppIcon(
                packageName = app.packageName,
                onResolveIcon = onResolveIcon,
                modifier = Modifier.align(Alignment.Center),
            )
        },
        content = {
            Column(
                modifier =
                    Modifier.align(Alignment.CenterStart)
                        .padding(start = Dimens.smallPadding, end = Dimens.smallPadding)
                        .padding(vertical = Dimens.smallPadding)
            ) {
                Text(
                    text = app.name,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (line != null) RouteStatusLine(line)
            }
        },
        trailingContent = {
            Row(
                modifier = Modifier.align(Alignment.Center),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                CountryChip(appName = app.name, exit = exit, onClick = onPick)
                if (onClear != null) {
                    IconButton(onClick = onClear) {
                        Icon(
                            imageVector = Icons.Rounded.RemoveCircleOutline,
                            contentDescription = stringResource(R.string.app_country_remove, app.name),
                            tint = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                        )
                    }
                } else {
                    Spacer(Modifier.size(Dimens.smallPadding))
                }
            }
        },
    )
}

/** A flag and the country (or "City, Country"), or a plus and "Country" while the app has none. */
@Composable
private fun CountryChip(appName: String, exit: AppExit?, onClick: () -> Unit) {
    val label = exit?.let { exitLabel(it, ::countryDisplayName) }
    val description =
        if (label == null) {
            stringResource(R.string.app_country_choose, appName)
        } else {
            stringResource(R.string.app_country_change, appName, label)
        }
    val shape = RoundedCornerShape(ChipRadius)
    val active = exit != null
    Row(
        modifier =
            Modifier.widthIn(max = ChipMaxWidth)
                .clip(shape)
                .background(if (active) MaterialTheme.colorScheme.surfaceContainerHighest else Color.Transparent)
                .border(
                    Dimens.thinBorderWidth,
                    MaterialTheme.colorScheme.onSurface.copy(alpha = if (active) Alpha40 else Alpha20),
                    shape,
                )
                .clickable(onClick = onClick)
                .clearAndSetSemantics {
                    contentDescription = description
                    role = Role.Button
                }
                .padding(
                    start = Dimens.tinyPadding,
                    end = Dimens.smallPadding,
                    top = Dimens.tinyPadding,
                    bottom = Dimens.tinyPadding,
                ),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.tinyPadding),
    ) {
        if (exit != null) {
            CountryFlag(countryCode = exit.country, size = ChipFlagSize)
        } else {
            Icon(
                imageVector = Icons.Rounded.Add,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                modifier = Modifier.size(ChipFlagSize),
            )
        }
        Text(
            text = label ?: stringResource(R.string.app_country_chip),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (active) 1f else Alpha60),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** One line whatever the state, so a status update never moves the rows. */
@Composable
private fun RouteStatusLine(line: AppRouteLine) {
    val text = appRouteLineText(line)
    val dot =
        when (line.tone()) {
            RouteTone.Positive -> MaterialTheme.colorScheme.positive
            RouteTone.Pending -> MaterialTheme.colorScheme.warning
            RouteTone.Error -> MaterialTheme.colorScheme.error
            RouteTone.Muted -> MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)
        }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(StatusDotSize).clip(CircleShape).background(dot))
        Text(
            text = text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = Dimens.tinyPadding),
        )
    }
}

@Composable
internal fun appRouteLineText(line: AppRouteLine): String =
    when (line) {
        AppRouteLine.Paused -> stringResource(R.string.app_route_paused)
        AppRouteLine.Bypassed -> stringResource(R.string.app_route_bypassed)
        AppRouteLine.Waiting -> stringResource(R.string.app_route_waiting)
        AppRouteLine.Connecting -> stringResource(R.string.app_route_connecting)
        is AppRouteLine.Connected ->
            line.publicIp?.let {
                stringResource(R.string.app_route_connected_ip, isolateLeftToRight(it))
            } ?: stringResource(R.string.app_route_connected)
        is AppRouteLine.Unavailable -> stringResource(unavailableText(line.reason))
    }

private fun unavailableText(reason: AppRouteUnavailableReason?): Int =
    when (reason) {
        AppRouteUnavailableReason.TunnelDown -> R.string.app_route_waiting
        AppRouteUnavailableReason.NoToken -> R.string.app_route_no_token
        AppRouteUnavailableReason.LimitReached -> R.string.app_route_limit_reached
        AppRouteUnavailableReason.WaitingForRoute -> R.string.app_route_waiting_for_route
        AppRouteUnavailableReason.NoRelay -> R.string.app_route_no_relay
        null -> R.string.app_route_unavailable
    }

/** The app's launcher icon, loaded off the main thread; the Android glyph when it has none. */
@Composable
private fun AppIcon(
    packageName: PackageName,
    onResolveIcon: (PackageName) -> Drawable?,
    modifier: Modifier = Modifier,
) {
    val icon by
        produceState<Drawable?>(initialValue = null, packageName) {
            value =
                withContext(Dispatchers.IO) {
                    onResolveIcon(packageName)?.takeIf { it.isBelowMaxByteSize() && it.hasValidSize() }
                }
        }
    val drawable = icon
    if (drawable != null) {
        Canvas(modifier.size(AppIconSize)) {
            drawIntoCanvas { canvas ->
                drawable.setBounds(0, 0, size.width.toInt(), size.height.toInt())
                drawable.draw(canvas.nativeCanvas)
            }
        }
    } else {
        Image(
            imageVector = Icons.Rounded.Android,
            contentDescription = null,
            colorFilter = ColorFilter.tint(MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)),
            modifier = modifier.size(AppIconSize),
        )
    }
}

/**
 * Search field of the tab and of the picker. No autofocus: raising the keyboard on entry would
 * cover the list the user came to browse.
 */
@Composable
internal fun SearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val keyboard = LocalSoftwareKeyboardController.current
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
        placeholder = { Text(stringResource(R.string.search_placeholder)) },
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
        keyboardActions = KeyboardActions(onSearch = { keyboard?.hide() }),
    )
}

internal fun positionOf(index: Int, size: Int): Position =
    when (index) {
        0 if size == 1 -> Position.Single
        0 -> Position.Top
        size - 1 -> Position.Bottom
        else -> Position.Middle
    }

private object CountryContentKey {
    const val SWITCH = "country_switch"
    const val DESCRIPTION = "country_description"
    const val SEARCH = "country_search"
    const val NO_RESULT = "country_no_result"
    const val WITH_COUNTRY = "country_with"
    const val OTHER_APPS = "country_others"
}

private val AppIconSize = 24.dp
private val ChipFlagSize = 18.dp
private val ChipMaxWidth = 150.dp
private val ChipRadius = 14.dp
private val StatusDotSize = 7.dp
