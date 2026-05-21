package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import org.koin.compose.koinInject

/**
 * Warren-specific tunnel settings host screen. Surfaces the four
 * Warren toggles read by [WarrenTunnelConfigBuilder] at connect time:
 *   - DAITA padding (Tamaraw)
 *   - NAT-PMP port forwarding
 *   - Multi-hop entry relay
 *   - M4.0 obfuscation
 *
 * Reached via [com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey]
 * from the main Settings screen ("Warren tunnel" entry). The switches
 * write through [WarrenLocalSettingsRepository] so the change is
 * persisted and picked up on the next connect.
 *
 * A `WarrenTunnelConfigBuilder.build()` is invoked at connect time
 * (not eagerly here), so changes here only take effect on the next
 * connect attempt; the running session is not torn down. D.4 step 9
 * will add a "Reconnect now" affordance when the user mutates a flag
 * while connected.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenTunnelSettings(navigator: Navigator) {
    val repo = koinInject<WarrenLocalSettingsRepository>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val daita by repo.daitaEnabled.collectAsStateWithLifecycle()
    val natPmp by repo.natPmpEnabled.collectAsStateWithLifecycle()
    val multiHop by repo.multiHopEnabled.collectAsStateWithLifecycle()
    val obfuscation by repo.obfuscationM40.collectAsStateWithLifecycle()
    val tunnelState by tunnelStateProvider.state.collectAsStateWithLifecycle()

    ScaffoldWithSmallTopBar(
        appBarTitle = "Warren tunnel",
        navigationIcon = {
            NavigateBackIconButton(onNavigateBack = {
                navigator.goBackUntil(SettingsNavKey)
            })
        },
    ) { modifier ->
        Column(
            modifier = Modifier.fillMaxSize().then(modifier).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Live tunnel state, sourced from WarrenQuinnStateProxy.
            Text(
                text = "Tunnel: $tunnelState",
                style = MaterialTheme.typography.titleSmall,
                color = if (tunnelState.startsWith("Connected")) {
                    Color(0xFF2E7D32)
                } else MaterialTheme.colorScheme.onSurface,
            )

            Text(
                text = "Changes apply on next connect.",
                style = MaterialTheme.typography.bodySmall,
            )

            ToggleRow(
                title = "DAITA padding",
                subtitle = "Tamaraw padding machine (constant-rate, anti-fingerprint).",
                value = daita,
                onValueChange = repo::setDaitaEnabled,
            )

            ToggleRow(
                title = "NAT-PMP port forwarding",
                subtitle = "Request a stable external port from the exit (BitTorrent, hosting).",
                value = natPmp,
                onValueChange = repo::setNatPmpEnabled,
            )

            ToggleRow(
                title = "Multi-hop entry",
                subtitle = "Route via a separate entry relay before the exit (slower, more private).",
                value = multiHop,
                onValueChange = repo::setMultiHopEnabled,
            )

            ToggleRow(
                title = "M4.0 obfuscation",
                subtitle = "Disguise QUIC traffic as plain HTTPS (anti-censorship).",
                value = obfuscation,
                onValueChange = repo::setObfuscationM40,
            )
        }
    }
}

@Composable
private fun ToggleRow(
    title: String,
    subtitle: String,
    value: Boolean,
    onValueChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, style = MaterialTheme.typography.titleSmall)
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = value, onCheckedChange = onValueChange)
    }
}
