package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_CARD_HEADER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import java.util.Locale

// The desktop pill fades its fill on this clock when the phase changes.
private const val PILL_TRANSITION_MILLIS = 300

// The desktop pill shadow: 0 6 18 -6 of its own fill at 60 %.
private val PILL_SHADOW_BLUR = 18.dp
private val PILL_SHADOW_SPREAD = (-6).dp
private val PILL_SHADOW_OFFSET = 6.dp
private const val PILL_SHADOW_ALPHA = 0.6f

// Fixed whether or not the details button is there, so the hero keeps its frame in every state.
private val SUBTITLE_ROW_HEIGHT = 18.dp
private val SUBTITLE_ROW_GAP = 8.dp
private val DETAILS_BUTTON_SIZE = 18.dp
private val DETAILS_ICON_SIZE = 18.dp

private val APP_ROUTING_LABEL_RADIUS = 6.dp
private const val APP_ROUTING_LABEL_FILL = 0.12f
private const val APP_ROUTING_LABEL_BORDER = 0.45f
private val APP_ROUTING_LABEL_ICON = 14.dp
private val APP_ROUTING_LABEL_PADDING_H = 8.dp
private val APP_ROUTING_LABEL_PADDING_V = 4.dp
private val APP_ROUTING_LABEL_TOP = 6.dp

/**
 * The status block of the home panel (desktop `HomeStatus`): the phase pill with its eye, the
 * title, and the subtitle row with the exit's load and the button that opens the connection
 * details. The block rides [DesignTokens.Home.Panel.Overlap] up over the hero's fade.
 */
@Composable
internal fun HomeStatusBlock(
    state: ConnectUiState,
    detailsOpen: Boolean,
    onToggleDetails: () -> Unit,
    onAppRoutingClick: () -> Unit,
    exitLoad: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val surfaces = LocalWarrenSurfaces.current
    val phase = state.tunnelState.connectionPhase(state.hostOffline)
    val status =
        homeStatus(
            state.tunnelState,
            state.hostOffline,
            state.location,
            includeOnly = state.vpnOnlyForCount != null,
        )
    val hasDetails = state.tunnelState.isConnectingOrConnected()
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                // The status is the whole answer to "did my tap do anything", so a screen reader
                // hears it the moment it changes.
                .semantics(mergeDescendants = false) { liveRegion = LiveRegionMode.Polite }
                .clickable(enabled = hasDetails, onClick = onToggleDetails)
                .testTag(CONNECT_CARD_HEADER_TEST_TAG),
        verticalArrangement = Arrangement.spacedBy(DesignTokens.Home.Panel.Gap),
    ) {
        StatusPill(phase = phase, label = stringResource(status.pill))
        Text(
            text = status.title.text(),
            style = HomeTypography.title,
            color = surfaces.homeText,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = DesignTokens.Home.Title.MarginTop),
        )
        Row(
            modifier = Modifier.fillMaxWidth().height(SUBTITLE_ROW_HEIGHT),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(SUBTITLE_ROW_GAP),
        ) {
            // The subtitle gives way to the load badge, and the chevron keeps the row's end.
            Row(
                modifier = Modifier.weight(1f),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(SUBTITLE_ROW_GAP),
            ) {
                status.subtitle?.let { subtitle ->
                    val mono = subtitle is HomeLine.Hostname
                    // A host name reads left to right in every language.
                    CompositionLocalProvider(
                        LocalLayoutDirection provides
                            if (mono) LayoutDirection.Ltr else LocalLayoutDirection.current
                    ) {
                        Text(
                            text = subtitle.text(),
                            style =
                                if (mono) HomeTypography.subtitleMono else HomeTypography.subtitle,
                            color = if (mono) surfaces.homeTextMono else surfaces.homeTextSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                    }
                }
                if (state.tunnelState is TunnelState.Connected) exitLoad?.invoke()
            }
            if (hasDetails) {
                IconButton(onClick = onToggleDetails, modifier = Modifier.size(DETAILS_BUTTON_SIZE)) {
                    CompositionLocalProvider(LocalContentColor provides surfaces.homeTextSecondary) {
                        // The sheet opens upwards over the hero, so the arrow points up while
                        // it is closed; the announcement still names what the tap does.
                        ExpandChevron(
                            isExpanded = detailsOpen,
                            pointsUpWhenCollapsed = true,
                            modifier = Modifier.size(DETAILS_ICON_SIZE),
                        )
                    }
                }
            }
        }
        AppRoutingLabels(state, onAppRoutingClick)
    }
}

@Composable
private fun StatusPill(phase: ConnectionPhase, label: String) {
    val pill = DesignTokens.Home.Pill
    val target = phase.pillColors(LocalWarrenSurfaces.current)
    val background by
        animateColorAsState(
            target.background,
            tween(PILL_TRANSITION_MILLIS, easing = EaseOut),
            label = "home_pill_fill",
        )
    val text by
        animateColorAsState(
            target.text,
            tween(PILL_TRANSITION_MILLIS, easing = EaseOut),
            label = "home_pill_text",
        )
    Row(
        modifier =
            Modifier.testTag(HOME_PILL_TEST_TAG)
                .dropShadow(
                    CircleShape,
                    Shadow(
                        radius = PILL_SHADOW_BLUR,
                        color = background.copy(alpha = PILL_SHADOW_ALPHA),
                        spread = PILL_SHADOW_SPREAD,
                        offset = DpOffset(0.dp, PILL_SHADOW_OFFSET),
                    ),
                )
                .clip(CircleShape)
                .background(background)
                .padding(
                    start = pill.PaddingStart,
                    end = pill.PaddingEnd,
                    top = pill.PaddingVertical,
                    bottom = pill.PaddingVertical,
                ),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(pill.Gap),
    ) {
        Icon(
            painter =
                painterResource(
                    if (phase.isEyeOpen()) R.drawable.ic_eye_show else R.drawable.ic_eye_hide
                ),
            contentDescription = null, // The label carries the meaning.
            tint = text,
            modifier = Modifier.size(pill.Icon),
        )
        Text(text = label, style = HomeTypography.pill, color = text, maxLines = 1)
    }
}

/** A line of the status block in the words of the current locale. */
@Composable
internal fun HomeLine.text(): String =
    when (this) {
        is HomeLine.Res -> stringResource(id)
        is HomeLine.Literal -> text
        is HomeLine.Hostname ->
            if (entry != null) stringResource(R.string.x_via_x, exit, entry) else exit
        is HomeLine.Place -> {
            // The same "%1$s, %2$s" template names a place city first here and country first on
            // the location row.
            val comma = stringResource(R.string.country_comma_city)
            placeName(city, countryNames(countryCode)) { place, country ->
                comma.format(place, country)
            }
        }
        is HomeLine.ExitPlace -> {
            val comma = stringResource(R.string.country_comma_city)
            placeName(city, countryNames(countryCode)) { place, country ->
                comma.format(country, place)
            }
        }
    }

/** The country's name in the current locale, then in English, the language of the relay list. */
private fun countryNames(countryCode: String): List<String> =
    listOf(countryDisplayName(countryCode), countryDisplayName(countryCode, Locale.ENGLISH))

/** The labels App routing puts under the connection state, each opening it. */
@Composable
private fun AppRoutingLabels(state: ConnectUiState, onClick: () -> Unit) {
    state.vpnOnlyForCount?.let { count ->
        AppRoutingLabel(
            text = pluralStringResource(R.plurals.vpn_only_for_apps, count, count),
            icon = painterResource(R.drawable.ic_forum_alert_circle),
            onClick = onClick,
        )
    }
    // Locked apps have no Internet until the VPN connects, which an app that stopped working must
    // never leave unexplained (desktop LockedAppsLabel); and when their blackhole is down (another
    // VPN app took the slot) they reach the Internet without it, which must not read as blocked.
    if (state.lockedAppsCount > 0 && state.tunnelState !is TunnelState.Connected) {
        AppRoutingLabel(
            text =
                pluralStringResource(
                    if (state.lockedAppsHeld) R.plurals.apps_blocked_until_vpn
                    else R.plurals.apps_locked_not_held,
                    state.lockedAppsCount,
                    state.lockedAppsCount,
                ),
            icon =
                if (state.lockedAppsHeld) rememberVectorPainter(Icons.Outlined.Lock)
                else painterResource(R.drawable.ic_forum_alert_circle),
            onClick = onClick,
        )
    }
}

/**
 * A label under the connection state that opens App routing, so what App routing does to the
 * device is never a surprise whatever the tunnel is doing (desktop IncludeOnlyLabel and
 * LockedAppsLabel), in the pending colour of the home pill.
 */
@Composable
private fun AppRoutingLabel(text: String, icon: Painter, onClick: () -> Unit) {
    val shape = RoundedCornerShape(APP_ROUTING_LABEL_RADIUS)
    val surfaces = LocalWarrenSurfaces.current
    val warning = surfaces.homePillPending
    Row(
        modifier =
            Modifier.padding(top = APP_ROUTING_LABEL_TOP)
                .clip(shape)
                .background(warning.copy(alpha = APP_ROUTING_LABEL_FILL))
                .border(1.dp, warning.copy(alpha = APP_ROUTING_LABEL_BORDER), shape)
                .clickable(role = Role.Button, onClick = onClick)
                .padding(
                    horizontal = APP_ROUTING_LABEL_PADDING_H,
                    vertical = APP_ROUTING_LABEL_PADDING_V,
                ),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(APP_ROUTING_LABEL_PADDING_V),
    ) {
        Icon(
            painter = icon,
            contentDescription = null,
            tint = warning,
            modifier = Modifier.size(APP_ROUTING_LABEL_ICON),
        )
        Text(text = text, style = HomeTypography.subtitle, color = surfaces.homeText)
    }
}

internal const val HOME_PILL_TEST_TAG = "home_pill_test_tag"
