package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import com.warrenbrowse.vpn.lib.model.TunnelState
import androidx.compose.ui.unit.Dp
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces

/**
 * Neutral: the location and shuffle fields, and the action that turns the kill switch off.
 * Connect: a solid go. Disconnect, Cancel: an outlined stop (desktop `HomeControls`), the fill
 * saying what the click does rather than the state the tunnel is in.
 */
enum class CardButtonTone {
    Neutral,
    Connect,
    Disconnect,
    Cancel,
}

data class CardButtonColors(val fill: Color, val pressed: Color, val border: Color, val text: Color)

fun CardButtonTone.colors(surfaces: WarrenSurfaces): CardButtonColors =
    when (this) {
        CardButtonTone.Neutral ->
            CardButtonColors(
                surfaces.homeField,
                surfaces.homeFieldPressed,
                Color.Transparent,
                surfaces.homeText,
            )
        CardButtonTone.Connect ->
            CardButtonColors(
                surfaces.homeConnect,
                surfaces.homeConnectPressed,
                Color.Transparent,
                surfaces.homeConnectText,
            )
        CardButtonTone.Disconnect ->
            CardButtonColors(
                Color.Transparent,
                surfaces.homeDisconnectHover,
                surfaces.homeDisconnectLine,
                surfaces.homeDisconnectText,
            )
        CardButtonTone.Cancel ->
            CardButtonColors(
                Color.Transparent,
                surfaces.homeCancelHover,
                surfaces.homeCancelLine,
                surfaces.homeCancelText,
            )
    }

/**
 * The action the primary button performs: connect while nothing is up (a teardown in flight
 * included, where the button is disabled), cancel an attempt, disconnect a tunnel, and a neutral
 * "turn off the switch" on an error, which is not an alarm.
 */
fun TunnelState.actionTone(): CardButtonTone =
    when (this) {
        is TunnelState.Disconnected,
        is TunnelState.Disconnecting -> CardButtonTone.Connect
        is TunnelState.Connecting -> CardButtonTone.Cancel
        is TunnelState.Connected -> CardButtonTone.Disconnect
        is TunnelState.Error -> CardButtonTone.Neutral
    }

// The desktop Button transitions its background over 150 ms on a variant change.
internal const val CARD_BUTTON_COLOR_MILLIS = 150
private const val DISABLED_ALPHA = 0.5f

/**
 * A home control (desktop `HomeControls` field and action button): radius 10, the tone's fill, an
 * inset outline for the outlined tones, a darker fill while pressed, the whole control at half
 * opacity while disabled.
 */
@Composable
fun CardButton(
    colors: CardButtonColors,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    height: Dp = DesignTokens.Home.Controls.ButtonHeight,
    enabled: Boolean = true,
    content: @Composable BoxScope.() -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val fill by
        animateColorAsState(
            if (pressed && enabled) colors.pressed else colors.fill,
            animationSpec = tween(CARD_BUTTON_COLOR_MILLIS),
            label = "card_button_fill",
        )
    val border by
        animateColorAsState(
            colors.border,
            animationSpec = tween(CARD_BUTTON_COLOR_MILLIS),
            label = "card_button_border",
        )
    val text by
        animateColorAsState(
            colors.text,
            animationSpec = tween(CARD_BUTTON_COLOR_MILLIS),
            label = "card_button_text",
        )
    val shape = RoundedCornerShape(DesignTokens.Home.Controls.Radius)
    Box(
        modifier =
            modifier
                // A floor, not a fixed size: a large font scale grows the control
                // instead of clipping its label.
                .heightIn(min = height)
                .alpha(if (enabled) 1f else DISABLED_ALPHA)
                .clip(shape)
                .background(fill)
                .border(DesignTokens.Home.Controls.OutlineWidth, border, shape)
                .clickable(
                    interactionSource = interaction,
                    // The ripple also draws the focus highlight a TV remote needs.
                    indication = LocalIndication.current,
                    enabled = enabled,
                    role = Role.Button,
                    onClick = onClick,
                ),
        contentAlignment = Alignment.Center,
    ) {
        CompositionLocalProvider(LocalContentColor provides text, content = { content() })
    }
}
