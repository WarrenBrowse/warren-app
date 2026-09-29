package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import com.warrenbrowse.vpn.feature.home.impl.connect.TunnelStatePreviewParameterProvider
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.CardTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces

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
 * The primary connection action, signalling the ACTION its click performs, in lockstep with the
 * desktop `ConnectButton` + `DisconnectButton`:
 * - Disconnected -> dark green "Connect" (connect)
 * - Connecting -> "Cancel" (abort the in-flight attempt)
 * - Connected -> brick red "Disconnect" (tear down)
 * - Disconnecting -> disabled "Connect" (teardown in flight, nothing to do)
 * - Error/blocked -> neutral "Disconnect" (turn the switch off)
 */
@Composable
fun ConnectionButton(
    modifier: Modifier = Modifier,
    state: TunnelState,
    disconnectClick: () -> Unit,
    cancelClick: () -> Unit,
    connectClick: () -> Unit,
) {
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

    CardButton(
        colors = state.actionTone().colors(LocalWarrenSurfaces.current),
        onClick = onClick,
        enabled = state !is TunnelState.Disconnecting,
        modifier = modifier,
    ) {
        // "Connect" -> "Cancel" -> "Disconnect" crosses over on the same clock
        // as the colour, so the label does not flicker under a fading button.
        AnimatedContent(
            targetState = buttonText,
            transitionSpec = {
                fadeIn(tween(CARD_BUTTON_COLOR_MILLIS)) togetherWith
                    fadeOut(tween(CARD_BUTTON_COLOR_MILLIS))
            },
            label = "connect_button_label",
        ) { label ->
            Text(
                text = label,
                textAlign = TextAlign.Center,
                style = CardTypography.button,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

private fun TunnelState.actionLabel() =
    when (this) {
        is TunnelState.Disconnected,
        is TunnelState.Disconnecting -> R.string.connect
        is TunnelState.Connecting -> R.string.cancel
        is TunnelState.Connected,
        is TunnelState.Error -> R.string.disconnect
    }
