package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

@Preview
@Composable
private fun PreviewConnectionStatusText(
    @PreviewParameter(TunnelStatePreviewParameterProvider::class) tunnelState: TunnelState
) {
    AppTheme {
        Column(modifier = Modifier.background(MaterialTheme.colorScheme.surface)) {
            ConnectionStatusText(state = tunnelState)
        }
    }
}

@Composable
fun ConnectionStatusText(state: TunnelState, hostOffline: Boolean = false) {
    Text(
        text = state.text(hostOffline),
        color = state.textColor(hostOffline),
        style = MaterialTheme.typography.titleLarge,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
private fun TunnelState.text(hostOffline: Boolean) =
    when (this) {
        // Connected with the host offline is a real window (the native
        // session holds Connected through its transparent redial): a green
        // "Connected" there is a lie. Only the connected state degrades;
        // every other state's copy already tells the truth (desktop
        // "interrupted" phase parity). The kill switch still holds, so the
        // user stays protected.
        is TunnelState.Connected ->
            if (hostOffline) {
                stringResource(id = R.string.connection_interrupted)
            } else {
                stringResource(id = R.string.connected)
            }
        is TunnelState.Connecting -> stringResource(id = R.string.connecting)
        is TunnelState.Disconnected -> stringResource(id = R.string.disconnected)
        is TunnelState.Disconnecting ->
            when (actionAfterDisconnect) {
                ActionAfterDisconnect.Nothing -> stringResource(id = R.string.disconnecting)
                ActionAfterDisconnect.Block -> stringResource(id = R.string.blocking)
                ActionAfterDisconnect.Reconnect -> stringResource(id = R.string.connecting)
            }
        is TunnelState.Error ->
            stringResource(
                id =
                    if (errorState.isBlocking) R.string.blocked_connection else R.string.error_state
            )
    }.uppercase()

@Composable
private fun TunnelState.textColor(hostOffline: Boolean) =
    when (this) {
        is TunnelState.Connected ->
            if (hostOffline) {
                MaterialTheme.colorScheme.warning
            } else {
                MaterialTheme.colorScheme.positive
            }
        is TunnelState.Connecting -> MaterialTheme.colorScheme.onSurface
        is TunnelState.Disconnected -> MaterialTheme.colorScheme.error
        is TunnelState.Disconnecting ->
            when (actionAfterDisconnect) {
                ActionAfterDisconnect.Nothing -> MaterialTheme.colorScheme.error
                ActionAfterDisconnect.Block -> MaterialTheme.colorScheme.onSurface
                ActionAfterDisconnect.Reconnect -> MaterialTheme.colorScheme.onSurface
            }
        is TunnelState.Error ->
            if (errorState.isBlocking) MaterialTheme.colorScheme.onSurface
            else MaterialTheme.colorScheme.error
    }
