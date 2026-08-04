package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.DAITA_UNAVAILABLE_BANNER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * Dedicated DAITA settings page (desktop `DaitaSettingsView` parity): an
 * explainer of what the defence does plus a single Enable switch. The
 * "direct only" mode desktop exposes does not apply to Warren, which always
 * runs multi-hop to enable DAITA. The switch feeds
 * [com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder] at connect time.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenDaitaSettings(navigator: Navigator) {
    val repo = koinInject<WarrenLocalSettingsRepository>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()

    val daita by repo.daitaEnabled.collectAsStateWithLifecycle()
    val multiHopEnabled by repo.multiHopEnabled.collectAsStateWithLifecycle()
    val connectedInfo by tunnelStateProvider.connectedInfo.collectAsStateWithLifecycle()

    // The switch says what the client asked for; this says what the exit
    // granted. A page that only shows the switch reads as protected while the
    // live connection may not be.
    val daitaUnavailable = daita &&
        (connectedInfo as? WarrenConnectedInfo.Connected)?.daita == false

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.daita),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
    ) { modifier ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(modifier)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            TunnelChangesBanner(connectedInfo) { reconnectInvoker.reconnect() }

            if (daitaUnavailable) {
                Text(
                    text = stringResource(R.string.daita_not_active_on_server),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(
                            color = MaterialTheme.colorScheme.errorContainer,
                            shape = RoundedCornerShape(Dimens.mediumPadding),
                        )
                        .padding(Dimens.mediumPadding)
                        .testTag(DAITA_UNAVAILABLE_BANNER_TEST_TAG),
                )
            }

            Text(
                text = stringResource(R.string.daita_explainer),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = stringResource(R.string.daita_explainer_note),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            // Multihop is what makes DAITA available on the chosen exit, so the
            // claim only holds while multihop is actually on.
            if (multiHopEnabled) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Info,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        text = stringResource(R.string.daita_multihop_note),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            ToggleCell(
                title = stringResource(R.string.enable),
                subtitle = stringResource(R.string.tunnel_daita_padding_subtitle),
                value = daita,
                onValueChange = repo::setDaitaEnabled,
                position = Position.Single,
            )
        }
    }
}
