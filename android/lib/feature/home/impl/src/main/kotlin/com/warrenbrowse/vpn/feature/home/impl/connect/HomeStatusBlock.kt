package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.layout.layout
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
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.common.compose.isTv
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.HomeDetailsCard
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.model.countryDisplayName
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

// The touch floor of the exit row on a phone, reached without growing the drawn row.
private val EXIT_ROW_TOUCH_HEIGHT = 44.dp

private val APP_ROUTING_LABEL_RADIUS = 6.dp
private const val APP_ROUTING_LABEL_FILL = 0.12f
private const val APP_ROUTING_LABEL_BORDER = 0.45f
private val APP_ROUTING_LABEL_ICON = 14.dp
private val APP_ROUTING_LABEL_PADDING_H = 8.dp
private val APP_ROUTING_LABEL_PADDING_V = 4.dp
private val APP_ROUTING_LABEL_TOP = 6.dp

/**
 * The status block of the home panel (desktop `HomeStatus`): the phase pill with its eye, the
 * title, then the plain subtitle line, or once protected the exit row, one button that opens the
 * connection details card ([HomeDetails]) in place right under it. The block rides
 * [DesignTokens.Home.Panel.Overlap] up over the hero's fade.
 */
@Composable
@Suppress("LongMethod")
internal fun HomeStatusBlock(
    state: ConnectUiState,
    detailsOpen: Boolean,
    onToggleDetails: () -> Unit,
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
    val connected = state.tunnelState is TunnelState.Connected
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                // The status is the whole answer to "did my tap do anything", so a screen reader
                // hears it the moment it changes.
                .semantics(mergeDescendants = false) { liveRegion = LiveRegionMode.Polite },
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
        if (connected) {
            ExitRow(
                subtitle = status.subtitle,
                open = detailsOpen,
                onClick = onToggleDetails,
                exitLoad = exitLoad,
            )
        } else {
            val subtitle = DesignTokens.Home.Subtitle
            Text(
                // A blank line keeps the block's height, so the hero does not reframe.
                text = status.subtitle?.text() ?: NBSP,
                style = HomeTypography.subtitle,
                color = surfaces.homeTextSecondary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier =
                    Modifier.padding(top = subtitle.MarginTop)
                        .padding(vertical = subtitle.PaddingVertical),
            )
        }
    }
}

/**
 * The connection details card, opened in place under the exit row while [open], growing on the
 * hero's own resize clock so the controls glide down as the hero shrinks.
 */
@Composable
internal fun HomeDetails(open: Boolean, state: ConnectUiState, modifier: Modifier = Modifier) {
    val spec = SceneryFraming.COMPACT_TRANSITION_MS
    AnimatedVisibility(
        visible = open,
        modifier = modifier,
        enter =
            expandVertically(tween(spec, easing = COMPACT_EASING), expandFrom = Alignment.Top) +
                fadeIn(tween(spec)),
        exit =
            shrinkVertically(tween(spec, easing = COMPACT_EASING), shrinkTowards = Alignment.Top) +
                fadeOut(tween(spec)),
    ) {
        OpenDetails(state)
    }
}

/** The details card as it stands once open, under its margin. */
@Composable
internal fun OpenDetails(state: ConnectUiState, modifier: Modifier = Modifier) {
    HomeDetailsCard(
        connectionDetails = state.tunnelState.toConnectionsDetails(state.location),
        autoRecoveryCount = state.autoRecoveryCount,
        enableSelectableText = !isTv(),
        modifier = modifier.padding(top = DesignTokens.Home.Details.MarginTop),
    )
}

/**
 * The exit line once protected (desktop `StyledExitRow`): the host name in the mono face, the
 * exit's load, and a thin chevron at the far end that turns over while the details are open. The
 * whole row is the button; its pressed well reaches [DesignTokens.Home.ExitRow.Outset] past the
 * text on both sides, and its touch target reaches [EXIT_ROW_TOUCH_HEIGHT] above and below the
 * drawn row without moving anything around it.
 */
@Composable
private fun ExitRow(
    subtitle: HomeLine?,
    open: Boolean,
    onClick: () -> Unit,
    exitLoad: (@Composable () -> Unit)?,
) {
    val row = DesignTokens.Home.ExitRow
    val surfaces = LocalWarrenSurfaces.current
    val interactions = remember { MutableInteractionSource() }
    val pressed by interactions.collectIsPressedAsState()
    val focused by interactions.collectIsFocusedAsState()
    val hovered by interactions.collectIsHoveredAsState()
    val turn by
        animateFloatAsState(
            targetValue = if (open) HALF_TURN else 0f,
            animationSpec = tween(row.ChevronMs),
            label = "exit_row_chevron",
        )
    // Names what the tap does, as the card's own chevron did.
    val action = stringResource(if (open) R.string.collapse else R.string.expand)
    val label = stringResource(R.string.connect_panel_connection_details)
    Row(
        modifier =
            Modifier.padding(top = row.MarginTop)
                .outsetTouch(horizontal = row.Outset, minHeight = EXIT_ROW_TOUCH_HEIGHT)
                .hoverable(interactions)
                .clickable(
                    interactionSource = interactions,
                    indication = null,
                    role = Role.Button,
                    onClickLabel = "$action $label",
                    onClick = onClick,
                )
                .testTag(CONNECT_CARD_HEADER_TEST_TAG)
                // The drawn row, centred in the taller touch target.
                .wrapContentHeight()
                .clip(RoundedCornerShape(row.Radius))
                .background(
                    if (pressed || focused || hovered) surfaces.homeDetails else Color.Transparent
                )
                .padding(horizontal = row.Outset, vertical = row.PaddingVertical),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(row.Gap),
    ) {
        subtitle?.let {
            val mono = it is HomeLine.Hostname
            // A host name reads left to right in every language.
            CompositionLocalProvider(
                LocalLayoutDirection provides
                    if (mono) LayoutDirection.Ltr else LocalLayoutDirection.current
            ) {
                Text(
                    text = it.text(),
                    style = if (mono) HomeTypography.exitRow else HomeTypography.subtitle,
                    color = if (mono) surfaces.homeTextMono else surfaces.homeTextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
            }
        }
        exitLoad?.invoke()
        Spacer(Modifier.weight(1f))
        Icon(
            painter = painterResource(R.drawable.ic_expand_more_outline),
            contentDescription = null,
            tint = surfaces.homeTextMono,
            modifier = Modifier.size(row.Chevron).rotate(turn),
        )
    }
}

/**
 * Lays the row out [horizontal] wider than its slot on both sides, and at least [minHeight] tall
 * for the finger, while reporting the slot's own width and the row's drawn height to the column:
 * the well spills past the text as the mockup draws it, and the touch target grows without
 * pushing the title or the controls.
 */
private fun Modifier.outsetTouch(horizontal: Dp, minHeight: Dp): Modifier = layout { m, c ->
    val outset = horizontal.roundToPx()
    val width = c.maxWidth + outset * 2
    val drawn = m.minIntrinsicHeight(width)
    val height = maxOf(drawn, minHeight.roundToPx())
    val placeable = m.measure(Constraints(maxWidth = width, minHeight = height, maxHeight = height))
    layout(placeable.width - outset * 2, drawn) {
        placeable.place(-outset, (drawn - placeable.height) / 2)
    }
}

private const val HALF_TURN = 180f
private const val NBSP = "\u00A0"

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
internal fun AppRoutingLabels(state: ConnectUiState, onClick: () -> Unit) {
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
