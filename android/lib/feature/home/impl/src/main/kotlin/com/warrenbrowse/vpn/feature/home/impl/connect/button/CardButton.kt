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
import androidx.compose.foundation.layout.height
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
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces

/**
 * Neutral: the location and shuffle buttons, a raised fill of the card itself. Connect,
 * Disconnect, Cancel: the action, white on a fill that says what the click does rather than the
 * state the tunnel is in (desktop `CardButton`).
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
                surfaces.button,
                surfaces.buttonPressed,
                surfaces.buttonLine,
                surfaces.text,
            )
        CardButtonTone.Connect ->
            CardButtonColors(
                surfaces.connect,
                surfaces.connectPressed,
                Color.Transparent,
                surfaces.actionText,
            )
        CardButtonTone.Disconnect ->
            CardButtonColors(
                surfaces.disconnect,
                surfaces.disconnectPressed,
                Color.Transparent,
                surfaces.actionText,
            )
        CardButtonTone.Cancel ->
            CardButtonColors(
                surfaces.cancel,
                surfaces.cancelPressed,
                Color.Transparent,
                surfaces.actionText,
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
private const val DISABLED_CONTENT_ALPHA = 0.5f

/**
 * A 32 dp card button (desktop `CardButton`): radius 6, a hairline border, the tone's fill, a
 * darker fill while pressed. No Material inflation: the platform extends the touch bounds of a
 * target smaller than 48 dp on its own (DesignParityTest).
 */
@Composable
fun CardButton(
    colors: CardButtonColors,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
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
    val shape = RoundedCornerShape(Dimens.cardButtonRadius)
    Box(
        modifier =
            modifier
                .height(Dimens.cardButtonHeight)
                .clip(shape)
                .background(fill)
                .border(Dimens.surfaceBorderWidth, border, shape)
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
        CompositionLocalProvider(LocalContentColor provides text) {
            Box(
                modifier = if (enabled) Modifier else Modifier.alpha(DISABLED_CONTENT_ALPHA),
                contentAlignment = Alignment.Center,
                content = content,
            )
        }
    }
}
