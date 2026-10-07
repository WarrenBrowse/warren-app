package com.warrenbrowse.vpn.feature.splittunneling.impl

import android.graphics.drawable.Drawable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.outlined.Language
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.exitLabel
import com.warrenbrowse.vpn.lib.model.AppRoute
import com.warrenbrowse.vpn.lib.model.DefaultRoute
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaDisabled
import com.warrenbrowse.vpn.lib.ui.theme.color.errorText
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.positiveText

/**
 * The route of one app: through the VPN, through the VPN from another country, or outside it. The
 * option the other apps take carries "Default", and choosing it removes the rule.
 */
@Composable
internal fun RoutePage(
    page: AppRoutingPage.Route,
    countrySupported: Boolean,
    actions: AppRoutingActions,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    SubPage(
        title = stringResource(R.string.app_route_title, page.app.name),
        titleLeading = { AppIcon(page.app.packageName, onResolveIcon, TitleIconSize) },
        buttons = {
            if (page.hasRule) {
                TextButton(
                    onClick = actions.onRemoveRule,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 44.dp),
                ) {
                    Text(
                        text = stringResource(R.string.app_route_remove_rule),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.errorText,
                    )
                }
            }
            PrimaryButton(
                onClick = actions.onDone,
                text = stringResource(R.string.wallet_settings_done),
                modifier = Modifier.fillMaxWidth(),
            )
        },
    ) {
        RouteOptions(page, countrySupported, actions)
        Spacer(modifier = Modifier.heightIn(min = Dimens.mediumPadding))
        LockOption(
            appName = page.app.name,
            locked = page.locked,
            // A lock on, even on an app sent outside the VPN since, can always be lifted.
            enabled = page.route != AppRoute.Direct || page.locked,
            onChange = actions.onSetLocked,
        )
    }
}

/** The three routes of an app, one of them chosen. */
@Composable
private fun RouteOptions(
    page: AppRoutingPage.Route,
    countrySupported: Boolean,
    actions: AppRoutingActions,
) {
    Column(
        modifier = Modifier.selectableGroup(),
        verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        val country = page.route as? AppRoute.Country
        RouteOption(
            glyph = { RouteGlyph(AppRoute.Vpn, OptionGlyphSize) },
            title = stringResource(R.string.app_route_through_vpn),
            subtitle = stringResource(R.string.app_route_vpn_subtitle),
            isDefault = page.defaultRoute == DefaultRoute.Vpn,
            selected = page.route == AppRoute.Vpn,
            onClick = { actions.onChooseRoute(AppRoute.Vpn) },
        )
        RouteOption(
            glyph = {
                if (country != null) {
                    RouteGlyph(country, OptionGlyphSize)
                } else {
                    Icon(
                        imageVector = Icons.Outlined.Language,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(OptionGlyphSize),
                    )
                }
            },
            title =
                country?.let { exitLabel(it.exit, ::countryDisplayName) }
                    ?: stringResource(R.string.app_route_other_country),
            subtitle =
                when {
                    !countrySupported -> stringResource(R.string.country_per_app_needs_android_10)
                    country != null -> stringResource(R.string.app_route_from_country)
                    else -> stringResource(R.string.app_route_choose_country)
                },
            isDefault = false,
            selected = country != null,
            enabled = countrySupported,
            chevron = true,
            onClick = actions.onOpenCountries,
        )
        RouteOption(
            glyph = { RouteGlyph(AppRoute.Direct, OptionGlyphSize) },
            title = stringResource(R.string.app_route_outside_vpn),
            subtitle = stringResource(R.string.app_route_outside_subtitle),
            isDefault = page.defaultRoute == DefaultRoute.Direct,
            selected = page.route == AppRoute.Direct,
            onClick = { actions.onChooseRoute(AppRoute.Direct) },
        )
    }
}

/**
 * "Never without the VPN": a property of the route rather than a fourth one, so it sits apart from
 * the three options and combines with a country (docs/app-routing.md section 8).
 */
@Composable
private fun LockOption(
    appName: String,
    locked: Boolean,
    enabled: Boolean,
    onChange: (Boolean) -> Unit,
) {
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .heightIn(min = OptionMinHeight)
                .clip(CardShape)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .toggleable(
                    value = locked,
                    enabled = enabled,
                    role = Role.Switch,
                    onValueChange = onChange,
                )
                .alpha(if (enabled) 1f else AlphaDisabled)
                .padding(horizontal = Dimens.cellStartPadding - 2.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.cellStartPadding - 2.dp),
    ) {
        Icon(
            imageVector = Icons.Outlined.Lock,
            contentDescription = null,
            tint =
                if (locked) MaterialTheme.colorScheme.positiveText
                else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(OptionGlyphSize),
        )
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = stringResource(R.string.app_route_never_without_vpn),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text =
                    if (enabled) {
                        stringResource(R.string.app_route_never_without_vpn_subtitle, appName)
                    } else {
                        stringResource(R.string.app_route_lock_needs_vpn_route)
                    },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (enabled) {
                Text(
                    text = stringResource(R.string.app_route_lock_android_note),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        // The whole row toggles; the switch only shows the state.
        WarrenSwitch(checked = locked, onCheckedChange = null, enabled = enabled)
    }
}

/** `[glyph] title / subtitle ...... [Default] [check] [chevron]`, a green border when chosen. */
@Suppress("LongParameterList")
@Composable
private fun RouteOption(
    glyph: @Composable () -> Unit,
    title: String,
    subtitle: String,
    isDefault: Boolean,
    selected: Boolean,
    onClick: () -> Unit,
    enabled: Boolean = true,
    chevron: Boolean = false,
) {
    val container = MaterialTheme.colorScheme.surfaceContainerHigh
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .heightIn(min = OptionMinHeight)
                .clip(CardShape)
                .background(container)
                .border(
                    OptionBorder,
                    if (selected) MaterialTheme.colorScheme.positive else Color.Transparent,
                    CardShape,
                )
                .selectable(
                    selected = selected,
                    enabled = enabled,
                    role = Role.RadioButton,
                    onClick = onClick,
                )
                .alpha(if (enabled) 1f else AlphaDisabled)
                .padding(horizontal = Dimens.cellStartPadding - 2.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.cellStartPadding - 2.dp),
    ) {
        glyph()
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (isDefault) DefaultBadge()
        if (selected) ChosenCheck()
        if (chevron) {
            Icon(
                imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun DefaultBadge() {
    Text(
        text = stringResource(R.string.app_route_default_badge),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 1,
        modifier =
            Modifier.border(1.5.dp, outlineColor(), CircleShape)
                .padding(horizontal = Dimens.smallPadding, vertical = 2.dp),
    )
}

/**
 * A page opened over the list: its title, a scrolling body, and its buttons kept at the bottom, as
 * the desktop lays them out.
 */
@Composable
internal fun SubPage(
    title: String,
    buttons: @Composable () -> Unit,
    titleLeading: (@Composable () -> Unit)? = null,
    scrolls: Boolean = true,
    body: @Composable () -> Unit,
) {
    Scaffold(containerColor = MaterialTheme.colorScheme.surface) { insets ->
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .padding(insets)
                    .padding(
                        start = Dimens.sideMarginNew,
                        end = Dimens.sideMarginNew,
                        top = Dimens.largePadding - 8.dp,
                        bottom = Dimens.mediumPadding,
                    ),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            PageTitle(text = title, leading = titleLeading)
            Column(
                modifier =
                    Modifier.weight(1f)
                        .fillMaxWidth()
                        .let { if (scrolls) it.verticalScroll(rememberScrollState()) else it }
            ) {
                body()
                Spacer(Modifier.weight(1f, fill = false))
            }
            PageButtons(buttons)
        }
    }
}

private val OptionMinHeight = 68.dp
private val OptionBorder = 2.dp
