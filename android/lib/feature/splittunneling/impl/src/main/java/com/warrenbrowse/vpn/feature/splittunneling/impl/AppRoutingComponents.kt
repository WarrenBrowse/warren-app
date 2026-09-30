package com.warrenbrowse.vpn.feature.splittunneling.impl

import android.graphics.drawable.Drawable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.outlined.Shield
import androidx.compose.material.icons.rounded.Android
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
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
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.RouteTone
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.exitLabel
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.isolateLeftToRight
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.tone
import com.warrenbrowse.vpn.feature.splittunneling.impl.extensions.hasValidSize
import com.warrenbrowse.vpn.feature.splittunneling.impl.extensions.isBelowMaxByteSize
import com.warrenbrowse.vpn.lib.model.AppRoute
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.ui.component.CountryFlag
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.positiveText
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

/** The pill on the right of a rule: a flag and a country, outside the VPN, or the VPN. */
@Composable
internal fun RouteChip(route: AppRoute, modifier: Modifier = Modifier) {
    Row(
        modifier =
            modifier
                .widthIn(max = ChipMaxWidth)
                .heightIn(min = ChipMinHeight)
                .border(ChipBorder, outlineColor(), CircleShape)
                .padding(start = Dimens.tinyPadding + 2.dp, end = Dimens.smallPadding + 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        RouteGlyph(route = route, size = ChipGlyphSize)
        Text(
            text = routeLabel(route),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
internal fun routeLabel(route: AppRoute): String =
    when (route) {
        AppRoute.Vpn -> stringResource(R.string.app_route_chip_vpn)
        AppRoute.Direct -> stringResource(R.string.app_route_outside_vpn)
        is AppRoute.Country -> exitLabel(route.exit, ::countryDisplayName)
    }

/** The flag of a country, the "open outside" glyph, or the shield of the VPN. */
@Composable
internal fun RouteGlyph(route: AppRoute, size: Dp, modifier: Modifier = Modifier) {
    when (route) {
        is AppRoute.Country -> CountryFlag(countryCode = route.exit.country, size = size, modifier = modifier)
        AppRoute.Direct ->
            Icon(
                imageVector = Icons.AutoMirrored.Rounded.OpenInNew,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.warning,
                modifier = modifier.size(size),
            )
        AppRoute.Vpn ->
            Icon(
                imageVector = Icons.Outlined.Shield,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.positiveText,
                modifier = modifier.size(size),
            )
    }
}

/** The filled check of a chosen option. */
@Composable
internal fun ChosenCheck(modifier: Modifier = Modifier) {
    Icon(
        imageVector = Icons.Rounded.CheckCircle,
        contentDescription = null,
        tint = MaterialTheme.colorScheme.positive,
        modifier = modifier.size(CheckSize),
    )
}

/** One line whatever the state, so a status update never moves the rows. */
@Composable
internal fun RouteStatusLine(line: AppRouteLine, modifier: Modifier = Modifier) {
    val dot =
        when (line.tone()) {
            RouteTone.Positive -> MaterialTheme.colorScheme.positive
            RouteTone.Pending -> MaterialTheme.colorScheme.warning
            RouteTone.Error -> MaterialTheme.colorScheme.error
            RouteTone.Muted -> MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)
        }
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(StatusDotSize).clip(CircleShape).background(dot))
        Text(
            text = appRouteLineText(line),
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
        AppRouteUnavailableReason.NoDialableNetwork -> R.string.app_route_no_dialable_network
        null -> R.string.app_route_unavailable
    }

/** The app's launcher icon, loaded off the main thread; the Android glyph when it has none. */
@Composable
internal fun AppIcon(
    packageName: PackageName,
    onResolveIcon: (PackageName) -> Drawable?,
    size: Dp,
    modifier: Modifier = Modifier,
) {
    val icon by
        produceState<Drawable?>(initialValue = null, packageName) {
            value =
                withContext(Dispatchers.IO) {
                    onResolveIcon(packageName)?.takeIf {
                        it.isBelowMaxByteSize() && it.hasValidSize()
                    }
                }
        }
    val drawable = icon
    if (drawable != null) {
        Canvas(modifier.size(size)) {
            drawIntoCanvas { canvas ->
                drawable.setBounds(0, 0, this.size.width.toInt(), this.size.height.toInt())
                drawable.draw(canvas.nativeCanvas)
            }
        }
    } else {
        Image(
            imageVector = Icons.Rounded.Android,
            contentDescription = null,
            colorFilter =
                ColorFilter.tint(MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)),
            modifier = modifier.size(size),
        )
    }
}

/**
 * Search field of the add and country pages. No autofocus: raising the keyboard on entry would
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
        modifier = modifier.fillMaxWidth(),
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
        shape = RoundedCornerShape(Dimens.smallPadding),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { keyboard?.hide() }),
    )
}

/** The title of a page opened over the list, with an optional leading element. */
@Composable
internal fun PageTitle(
    text: String,
    modifier: Modifier = Modifier,
    leading: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.cellStartPadding - 4.dp),
    ) {
        leading?.invoke()
        Text(
            text = text,
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** A column of the page's bottom buttons, spaced as the rest of the app's. */
@Composable
internal fun PageButtons(content: @Composable () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.smallPadding),
        verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
    ) {
        content()
    }
}

@Composable internal fun outlineColor() = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40)

internal val CardShape = RoundedCornerShape(10.dp)
internal val AppIconSize = 36.dp
internal val TitleIconSize = 44.dp
internal val CheckSize = 24.dp
internal val OptionGlyphSize = 26.dp
private val ChipGlyphSize = 22.dp
private val ChipMinHeight = 34.dp
private val ChipMaxWidth = 170.dp
private val ChipBorder = 1.5.dp
private val StatusDotSize = 7.dp
