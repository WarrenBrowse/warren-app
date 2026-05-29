package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import org.koin.compose.koinInject

/**
 * Warren-specific tunnel settings host screen. Surfaces the Warren toggles
 * read by [WarrenTunnelConfigBuilder] at connect time:
 *   - Privacy: kill switch (lockdown), IPv6, DNS
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
 * Changes here only take effect on the next connect attempt; the running
 * session is not torn down.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenTunnelSettings(navigator: Navigator) {
    val repo = koinInject<WarrenLocalSettingsRepository>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val daita by repo.daitaEnabled.collectAsStateWithLifecycle()
    val natPmp by repo.natPmpEnabled.collectAsStateWithLifecycle()
    val natPmpProtocol by repo.natPmpProtocol.collectAsStateWithLifecycle()
    val natPmpExternalPort by repo.natPmpExternalPort.collectAsStateWithLifecycle()
    val natPmpLifetime by repo.natPmpLifetimeSecs.collectAsStateWithLifecycle()
    val multiHop by repo.multiHopEnabled.collectAsStateWithLifecycle()
    val obfuscation by repo.obfuscationM40.collectAsStateWithLifecycle()
    val lockdown by repo.lockdownMode.collectAsStateWithLifecycle()
    val ipv6 by repo.ipv6Enabled.collectAsStateWithLifecycle()
    val dnsState by repo.dnsState.collectAsStateWithLifecycle()
    val customDns by repo.customDnsServers.collectAsStateWithLifecycle()
    val blockAds by repo.blockAds.collectAsStateWithLifecycle()
    val blockTrackers by repo.blockTrackers.collectAsStateWithLifecycle()
    val blockMalware by repo.blockMalware.collectAsStateWithLifecycle()
    val blockAdult by repo.blockAdultContent.collectAsStateWithLifecycle()
    val blockGambling by repo.blockGambling.collectAsStateWithLifecycle()
    val blockSocial by repo.blockSocialMedia.collectAsStateWithLifecycle()
    val tunnelState by tunnelStateProvider.state.collectAsStateWithLifecycle()

    val customDnsEnabled = dnsState == WarrenLocalSettingsRepository.DNS_STATE_CUSTOM

    ScaffoldWithSmallTopBar(
        appBarTitle = "Warren tunnel",
        navigationIcon = {
            NavigateBackIconButton(onNavigateBack = {
                navigator.goBackUntil(SettingsNavKey)
            })
        },
    ) { modifier ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(modifier)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Live tunnel state, sourced from WarrenQuinnStateProxy.
            Text(
                text = "Tunnel: $tunnelState",
                style = MaterialTheme.typography.titleSmall,
                color = when {
                    tunnelState.startsWith("Connected") -> Color(0xFF2E7D32)
                    tunnelState.startsWith("Blocking") -> Color(0xFFC62828)
                    else -> MaterialTheme.colorScheme.onSurface
                },
            )

            Text(
                text = "Changes apply on next connect.",
                style = MaterialTheme.typography.bodySmall,
            )

            SectionLabel("Privacy")

            ToggleRow(
                title = "Kill switch (lockdown)",
                subtitle = "Block all traffic if the tunnel drops, instead of " +
                    "falling back to the unprotected network.",
                value = lockdown,
                onValueChange = repo::setLockdownMode,
            )

            ToggleRow(
                title = "Enable IPv6",
                subtitle = "Route IPv6 through the tunnel. When off, IPv6 is " +
                    "blocked to prevent leaks.",
                value = ipv6,
                onValueChange = repo::setIpv6Enabled,
            )

            HorizontalDivider()
            SectionLabel("DNS")

            ToggleRow(
                title = "Use custom DNS",
                subtitle = "Send DNS queries to your own resolvers instead of " +
                    "the Warren exit resolver. DNS always stays inside the tunnel.",
                value = customDnsEnabled,
                onValueChange = { useCustom ->
                    repo.setDnsState(
                        if (useCustom) {
                            WarrenLocalSettingsRepository.DNS_STATE_CUSTOM
                        } else {
                            WarrenLocalSettingsRepository.DNS_STATE_DEFAULT
                        },
                    )
                },
            )

            if (customDnsEnabled) {
                CustomDnsField(
                    initial = customDns.joinToString(", "),
                    onCommit = { raw ->
                        repo.setCustomDnsServers(raw.split(',', '\n'))
                    },
                )
            }

            Text(
                text = "Content blocking (applied by the Warren exit resolver):",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            ToggleRow("Block ads", "", blockAds, repo::setBlockAds)
            ToggleRow("Block trackers", "", blockTrackers, repo::setBlockTrackers)
            ToggleRow("Block malware", "", blockMalware, repo::setBlockMalware)
            ToggleRow("Block adult content", "", blockAdult, repo::setBlockAdultContent)
            ToggleRow("Block gambling", "", blockGambling, repo::setBlockGambling)
            ToggleRow("Block social media", "", blockSocial, repo::setBlockSocialMedia)

            HorizontalDivider()
            SectionLabel("Tunnel")

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

            if (natPmp) {
                PortForwardingAdvanced(
                    protocol = natPmpProtocol,
                    onProtocolChange = repo::setNatPmpProtocol,
                    externalPort = natPmpExternalPort,
                    onExternalPortChange = repo::setNatPmpExternalPort,
                    lifetimeSecs = natPmpLifetime,
                    onLifetimeChange = repo::setNatPmpLifetimeSecs,
                )
            }

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

            OutlinedButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = { navigator.navigate(WarrenLocationPickerNavKey) },
            ) { Text("Exit relay…") }
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun CustomDnsField(initial: String, onCommit: (String) -> Unit) {
    // Local edit buffer so typing commas is not fought by re-derivation from
    // the persisted list. The repository is updated on every change (it
    // drops blanks and trims itself).
    var text by remember { mutableStateOf(initial) }
    OutlinedTextField(
        value = text,
        onValueChange = {
            text = it
            onCommit(it)
        },
        modifier = Modifier.fillMaxWidth(),
        label = { Text("Resolver addresses (comma-separated)") },
        placeholder = { Text("e.g. 9.9.9.9, 149.112.112.112") },
        singleLine = false,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PortForwardingAdvanced(
    protocol: String,
    onProtocolChange: (String) -> Unit,
    externalPort: Int,
    onExternalPortChange: (Int) -> Unit,
    lifetimeSecs: Int,
    onLifetimeChange: (Int) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(start = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Protocol", style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = protocol == "udp",
                onClick = { onProtocolChange("udp") },
                label = { Text("UDP") },
            )
            FilterChip(
                selected = protocol == "tcp",
                onClick = { onProtocolChange("tcp") },
                label = { Text("TCP") },
            )
        }

        var portText by remember { mutableStateOf(if (externalPort == 0) "" else externalPort.toString()) }
        OutlinedTextField(
            value = portText,
            onValueChange = {
                portText = it.filter(Char::isDigit).take(5)
                onExternalPortChange(portText.toIntOrNull() ?: 0)
            },
            modifier = Modifier.fillMaxWidth(),
            label = { Text("Preferred external port (blank = automatic)") },
            placeholder = { Text("49152-65535") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        )

        Text("Mapping lifetime", style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LifetimeChip("1 h", 3_600, lifetimeSecs, onLifetimeChange)
            LifetimeChip("6 h", 21_600, lifetimeSecs, onLifetimeChange)
            LifetimeChip("24 h", 86_400, lifetimeSecs, onLifetimeChange)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LifetimeChip(label: String, seconds: Int, selected: Int, onSelect: (Int) -> Unit) {
    FilterChip(
        selected = selected == seconds,
        onClick = { onSelect(seconds) },
        label = { Text(label) },
    )
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
            if (subtitle.isNotEmpty()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Switch(checked = value, onCheckedChange = onValueChange)
    }
}
