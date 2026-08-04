package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.style.TextDecoration
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.clickableAnnotatedString
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNatPmpStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * Dedicated Port forwarding settings page (desktop `PortForwardingSettingsView`
 * parity): an explainer plus the enable switch and, once enabled, the Android
 * preferred-port / protocol / lifetime controls and the live NAT-PMP status.
 *
 * Warren keeps the single preferred-port model rather than the desktop
 * multi-rule editor: the desktop editor depends on daemon plumbing (a live
 * mapping table over the control channel) that the Android client does not
 * have, so mappings are applied at (re)connect time instead.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenPortForwardingSettings(navigator: Navigator) {
    val repo = koinInject<WarrenLocalSettingsRepository>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()
    val natPmpStatusProvider = koinInject<WarrenNatPmpStatusProvider>()

    val natPmp by repo.natPmpEnabled.collectAsStateWithLifecycle()
    val natPmpProtocol by repo.natPmpProtocol.collectAsStateWithLifecycle()
    val natPmpExternalPort by repo.natPmpExternalPort.collectAsStateWithLifecycle()
    val natPmpLifetime by repo.natPmpLifetimeSecs.collectAsStateWithLifecycle()
    val natPmpStatusJson by natPmpStatusProvider.natPmpStatus.collectAsStateWithLifecycle()
    val exitPin by repo.exitPin.collectAsStateWithLifecycle()
    val connectedInfo by tunnelStateProvider.connectedInfo.collectAsStateWithLifecycle()

    // The NAT-PMP status describes the mapping on the exit the live tunnel was
    // built for, and it keeps reporting it until the next mapping lands. So a
    // "choose another exit" pick, which returns here with a different exit
    // pinned, would otherwise leave the resolved conflict on screen.
    var mappedPin by remember { mutableStateOf(exitPin) }
    LaunchedEffect(connectedInfo) {
        if (connectedInfo is WarrenConnectedInfo.Connected) mappedPin = exitPin
    }
    val portConflict = mappedPin == exitPin && natPmpIsPortConflict(natPmpStatusJson)

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.tunnel_natpmp_title),
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

            Text(
                text = stringResource(R.string.port_forwarding_desc),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            AbuseNotice()

            ToggleCell(
                title = stringResource(R.string.tunnel_natpmp_title),
                subtitle = stringResource(R.string.tunnel_natpmp_subtitle),
                value = natPmp,
                onValueChange = repo::setNatPmpEnabled,
                position = Position.Single,
            )

            if (natPmp) {
                PortForwardingAdvanced(
                    protocol = natPmpProtocol,
                    onProtocolChange = repo::setNatPmpProtocol,
                    externalPort = natPmpExternalPort,
                    onExternalPortChange = repo::setNatPmpExternalPort,
                    lifetimeSecs = natPmpLifetime,
                    onLifetimeChange = repo::setNatPmpLifetimeSecs,
                    statusLabel = natPmpStatusLabel(
                        context = LocalContext.current,
                        tunnelConnected = connectedInfo is WarrenConnectedInfo.Connected,
                        json = natPmpStatusJson,
                    ),
                    portConflict = portConflict,
                    portBlock = rememberNatPmpPortBlock(natPmpStatusJson),
                    // "Assign a free port" drops the pin to 0 so the exit picks
                    // any free one. NAT-PMP config applies on (re)connect on
                    // Android (no live control channel, unlike desktop), so
                    // reconnect to apply it and clear the conflict now.
                    onAssignFreePort = {
                        repo.setNatPmpExternalPort(0)
                        reconnectInvoker.reconnect()
                    },
                    // No connectOnPick: this screen cannot consume the connect
                    // result, so a pick here only re-pins (and reconnects a
                    // live tunnel) before popping back to this page.
                    onChooseAnotherExit = { navigator.navigate(WarrenLocationPickerNavKey()) },
                )
            }
        }
    }
}

/**
 * An open port is reachable by anyone, so a third-party abuse report can reach
 * us about it. Stating the strike rule here, rather than only in the terms, is
 * what makes the consequence foreseeable to whoever opens the port. Only the
 * appeal URL is the link target: a whole clickable paragraph gives a screen
 * reader no way to tell where the link is.
 */
@Composable
private fun AbuseNotice() {
    val uriHandler = LocalUriHandler.current
    Text(
        text = clickableAnnotatedString(
            text = stringResource(R.string.port_forwarding_abuse_notice),
            argument = stringResource(R.string.reports_url),
            linkStyle = SpanStyle(
                color = MaterialTheme.colorScheme.onSurface,
                textDecoration = TextDecoration.Underline,
            ),
            onClick = uriHandler::openUri,
        ),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
