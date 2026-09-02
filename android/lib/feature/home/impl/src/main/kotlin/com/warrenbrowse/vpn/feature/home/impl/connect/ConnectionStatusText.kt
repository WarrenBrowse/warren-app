package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80

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

private data class StatusCopy(
    @param:StringRes val title: Int,
    @param:StringRes val subtitle: Int?,
)

// Truth-statement status copy (desktop ConnectionStatus): the title states the
// user's exposure, the subtitle states the factual reason, instead of the
// upstream ALL-CAPS daemon state names.
private fun statusCopy(state: TunnelState, hostOffline: Boolean): StatusCopy =
    when (state) {
        is TunnelState.Connected ->
            if (hostOffline) {
                StatusCopy(R.string.connection_interrupted, R.string.you_are_protected)
            } else {
                StatusCopy(R.string.connection_established, R.string.you_are_protected)
            }
        is TunnelState.Connecting ->
            StatusCopy(R.string.you_are_visible, R.string.connection_in_progress)
        is TunnelState.Disconnected ->
            StatusCopy(R.string.you_are_visible, R.string.connection_not_encrypted)
        is TunnelState.Disconnecting ->
            when (state.actionAfterDisconnect) {
                // A reconnect is a teardown that is already coming back up, so it
                // reads as connection-in-progress. A plain teardown, kill switch
                // up (Block) or not (Nothing), reads as a transitional
                // "Disconnecting...": desktop never surfaces a distinct blocked
                // status mid-teardown.
                ActionAfterDisconnect.Reconnect ->
                    StatusCopy(R.string.you_are_visible, R.string.connection_in_progress)
                ActionAfterDisconnect.Nothing,
                ActionAfterDisconnect.Block ->
                    StatusCopy(R.string.you_are_visible, R.string.disconnecting)
            }
        is TunnelState.Error ->
            if (state.errorState.isBlocking) {
                StatusCopy(R.string.blocked_connection, null)
            } else {
                StatusCopy(R.string.you_are_visible, R.string.connection_not_encrypted)
            }
    }

/**
 * The connection card status header: a phase-colored eye (open while the user
 * is visible to the network, crossed once hidden) beside the accent-colored
 * status title and its factual subtitle.
 */
@Composable
fun ConnectionStatusText(
    state: TunnelState,
    hostOffline: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val phase = state.connectionPhase(hostOffline)
    val accent = phase.accentColor()
    val copy = statusCopy(state, hostOffline)

    Row(
        // The status is the whole answer to "did my tap do anything", so a
        // screen reader is told about it the moment it changes instead of the
        // user having to traverse back to the card. Merged so the title and its
        // subtitle are announced as one sentence.
        modifier =
            modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.Top,
    ) {
        Icon(
            imageVector =
                if (phase.isEyeOpen()) Icons.Rounded.Visibility else Icons.Rounded.VisibilityOff,
            contentDescription = null, // The status title carries the meaning.
            tint = accent,
            modifier = Modifier.padding(top = 2.dp).size(28.dp),
        )
        Column(modifier = Modifier.padding(start = Dimens.connectionStatusGap)) {
            Text(
                text = stringResource(id = copy.title),
                // The lifted tint, not the fill: the saturated accent reads at
                // about 3.5:1 on the card at this size (desktop rule).
                color = phase.titleColor(),
                // Desktop ConnectionStatus: 19/22 semibold.
                style =
                    MaterialTheme.typography.titleLarge.copy(fontSize = 19.sp, lineHeight = 22.sp),
                maxLines = 1,
                modifier = Modifier.marqueeLine(),
            )
            copy.subtitle?.let { subtitle ->
                Text(
                    text = stringResource(id = subtitle),
                    // Desktop: 13/18 at 80 % white.
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha80),
                    style =
                        MaterialTheme.typography.bodyMedium.copy(
                            fontSize = 13.sp,
                            lineHeight = 18.sp,
                        ),
                    maxLines = 1,
                    modifier = Modifier.marqueeLine(),
                )
            }
        }
    }
}
