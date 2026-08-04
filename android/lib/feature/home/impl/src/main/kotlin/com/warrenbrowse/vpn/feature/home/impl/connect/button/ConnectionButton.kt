package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.ButtonColors
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import com.warrenbrowse.vpn.feature.home.impl.connect.TunnelStatePreviewParameterProvider
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.pending
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.tertiaryDisabled

@Composable
@Preview
private fun PreviewConnectionButton(
    @PreviewParameter(TunnelStatePreviewParameterProvider::class) tunnelState: TunnelState
) {
    AppTheme {
        ConnectionButton(
            state = tunnelState,
            disconnectClick = {},
            cancelClick = {},
            connectClick = {},
        )
    }
}

/**
 * The primary connection action, signalling the ACTION its click performs, in
 * lockstep with the desktop `ConnectionActionButton` + `DisconnectButton`:
 *   - Disconnected  -> green "Connect"      (connect)
 *   - Connecting    -> orange "Cancel"      (abort the in-flight attempt)
 *   - Connected     -> red "Disconnect"     (tear down)
 *   - Disconnecting -> disabled green "Connect" (teardown in flight, nothing to do)
 *   - Error/blocked -> neutral "Disconnect" (turn the switch off)
 *
 * The orange (rather than red) Cancel and the disabled green Connect while
 * disconnecting were the two Android divergences from the desktop; they are the
 * reason the color/text/enabled are derived from one `when` on the state.
 */
@Composable
fun ConnectionButton(
    modifier: Modifier = Modifier,
    state: TunnelState,
    disconnectClick: () -> Unit,
    cancelClick: () -> Unit,
    connectClick: () -> Unit,
) {
    // The variant changes at the exact instants the user is watching the button
    // (the tap on Connect, the moment the handshake completes), so the colour
    // crosses over instead of snapping. 150ms is the desktop Button transition.
    val containerColor by
        animateColorAsState(
            targetValue = state.containerColor(),
            animationSpec = tween(COLOR_TRANSITION_MILLIS),
            label = "connect_button_container",
        )
    val contentColor by
        animateColorAsState(
            targetValue = state.contentColor(),
            animationSpec = tween(COLOR_TRANSITION_MILLIS),
            label = "connect_button_content",
        )

    val colors: ButtonColors =
        ButtonDefaults.buttonColors(
            containerColor = containerColor,
            contentColor = contentColor,
            // Disabled green Connect: the teardown is under way, so the only
            // button offered is the (inert) next action.
            disabledContainerColor = MaterialTheme.colorScheme.tertiaryDisabled,
            disabledContentColor = MaterialTheme.colorScheme.onTertiary.copy(alpha = Alpha20),
        )

    val buttonText = stringResource(id = state.actionLabel())

    val onClick =
        when (state) {
            is TunnelState.Disconnected -> connectClick
            is TunnelState.Connecting -> cancelClick
            // Disconnecting is disabled, so its click never fires; keep a no-op
            // instead of an action that could double-trigger a teardown.
            is TunnelState.Disconnecting -> {
                {}
            }
            else -> disconnectClick
        }

    PrimaryButton(
        onClick = onClick,
        colors = colors,
        isEnabled = state !is TunnelState.Disconnecting,
        modifier = modifier,
        text = buttonText,
        // "Connect" -> "Cancel" -> "Disconnect" crosses over on the same clock
        // as the colour, so the label does not flicker under a fading button.
        label = {
            AnimatedContent(
                targetState = buttonText,
                transitionSpec = {
                    fadeIn(tween(COLOR_TRANSITION_MILLIS)) togetherWith
                        fadeOut(tween(COLOR_TRANSITION_MILLIS))
                },
                modifier = Modifier.weight(1f),
                label = "connect_button_label",
            ) { label ->
                Text(
                    text = label,
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
    )
}

@Composable
private fun TunnelState.containerColor() =
    when (this) {
        is TunnelState.Disconnected,
        is TunnelState.Disconnecting -> MaterialTheme.colorScheme.positive
        is TunnelState.Connecting -> MaterialTheme.colorScheme.pending
        is TunnelState.Connected -> MaterialTheme.colorScheme.error
        // Neutral, whether the error is blocking (kill switch up) or not: the
        // action is "turn off the switch", not an alarm.
        is TunnelState.Error -> MaterialTheme.colorScheme.primary
    }

@Composable
private fun TunnelState.contentColor() =
    when (this) {
        is TunnelState.Disconnected,
        is TunnelState.Disconnecting -> MaterialTheme.colorScheme.onTertiary
        is TunnelState.Connecting,
        is TunnelState.Connected -> MaterialTheme.colorScheme.onError
        is TunnelState.Error -> MaterialTheme.colorScheme.onPrimary
    }

private fun TunnelState.actionLabel() =
    when (this) {
        is TunnelState.Disconnected,
        is TunnelState.Disconnecting -> R.string.connect
        is TunnelState.Connecting -> R.string.cancel
        is TunnelState.Connected,
        is TunnelState.Error -> R.string.disconnect
    }

// The desktop Button transitions its background over 150ms on a variant change.
private const val COLOR_TRANSITION_MILLIS = 150
