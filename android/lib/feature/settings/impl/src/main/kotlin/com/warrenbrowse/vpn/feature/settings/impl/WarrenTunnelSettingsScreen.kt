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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
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
import com.warrenbrowse.vpn.lib.repository.WarrenNatPmpStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import org.koin.compose.koinInject

/**
 * Warren-specific tunnel settings host screen. Surfaces the Warren toggles
 * read by [WarrenTunnelConfigBuilder] at connect time:
 *   - Privacy: kill switch (lockdown), IPv6, DNS
 *   - DAITA padding (Tamaraw)
 *   - NAT-PMP port forwarding
 *   - Multi-hop entry relay
 *   - Anti-censorship (read-only: M4.0 HTTP/3 mimicry is always-on)
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
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()
    val natPmpStatusProvider = koinInject<WarrenNatPmpStatusProvider>()
    val natPmpStatusJson by natPmpStatusProvider.natPmpStatus.collectAsStateWithLifecycle()
    val daita by repo.daitaEnabled.collectAsStateWithLifecycle()
    val natPmp by repo.natPmpEnabled.collectAsStateWithLifecycle()
    val natPmpProtocol by repo.natPmpProtocol.collectAsStateWithLifecycle()
    val natPmpExternalPort by repo.natPmpExternalPort.collectAsStateWithLifecycle()
    val natPmpLifetime by repo.natPmpLifetimeSecs.collectAsStateWithLifecycle()
    val exitCountry by repo.exitCountry.collectAsStateWithLifecycle()
    val lockdown by repo.lockdownMode.collectAsStateWithLifecycle()
    val ipv6 by repo.ipv6Enabled.collectAsStateWithLifecycle()
    val allowLan by repo.allowLan.collectAsStateWithLifecycle()
    val tunnelMtu by repo.tunnelMtu.collectAsStateWithLifecycle()
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
        appBarTitle = stringResource(R.string.tunnel_settings_title),
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
                text = stringResource(R.string.tunnel_state_line, tunnelState),
                style = MaterialTheme.typography.titleSmall,
                color = when {
                    tunnelState.startsWith("Connected") -> Color(0xFF2E7D32)
                    tunnelState.startsWith("Blocking") -> Color(0xFFC62828)
                    else -> MaterialTheme.colorScheme.onSurface
                },
            )

            // While connected, changing a flag only takes effect on the next
            // connect, so offer an explicit "Reconnect now" affordance that
            // tears down and re-establishes the tunnel with the new config
            // (reusing the cached mnemonic - no biometric re-prompt).
            val isConnected = tunnelState.startsWith("Connected")
            if (isConnected) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = stringResource(R.string.tunnel_changes_apply_next_connect),
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    OutlinedButton(onClick = { reconnectInvoker.reconnect() }) {
                        Text(stringResource(R.string.tunnel_reconnect_now))
                    }
                }
            } else {
                Text(
                    text = stringResource(R.string.tunnel_changes_apply_next_connect),
                    style = MaterialTheme.typography.bodySmall,
                )
            }

            SectionLabel(stringResource(R.string.privacy_disclaimer_title))

            ToggleRow(
                title = stringResource(R.string.tunnel_kill_switch_title),
                subtitle = stringResource(R.string.tunnel_kill_switch_subtitle),
                value = lockdown,
                onValueChange = repo::setLockdownMode,
            )

            ToggleRow(
                title = stringResource(R.string.tunnel_ipv6_title),
                subtitle = stringResource(R.string.tunnel_ipv6_subtitle),
                value = ipv6,
                onValueChange = repo::setIpv6Enabled,
            )

            ToggleRow(
                title = stringResource(R.string.tunnel_local_network_title),
                subtitle = stringResource(R.string.tunnel_local_network_subtitle),
                value = allowLan,
                onValueChange = repo::setAllowLan,
            )

            MtuField(
                mtu = tunnelMtu,
                onCommit = repo::setTunnelMtu,
            )

            HorizontalDivider()
            SectionLabel(stringResource(R.string.tunnel_dns_section))

            ToggleRow(
                title = stringResource(R.string.tunnel_use_custom_dns_title),
                subtitle = stringResource(R.string.tunnel_use_custom_dns_subtitle),
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
                text = stringResource(R.string.tunnel_content_blocking_header),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            ToggleRow(stringResource(R.string.tunnel_block_ads), "", blockAds, repo::setBlockAds)
            ToggleRow(stringResource(R.string.tunnel_block_trackers), "", blockTrackers, repo::setBlockTrackers)
            ToggleRow(stringResource(R.string.tunnel_block_malware), "", blockMalware, repo::setBlockMalware)
            ToggleRow(stringResource(R.string.tunnel_block_adult_content), "", blockAdult, repo::setBlockAdultContent)
            ToggleRow(stringResource(R.string.tunnel_block_gambling), "", blockGambling, repo::setBlockGambling)
            ToggleRow(stringResource(R.string.tunnel_block_social_media), "", blockSocial, repo::setBlockSocialMedia)

            HorizontalDivider()
            SectionLabel(stringResource(R.string.tunnel_section))

            ToggleRow(
                title = stringResource(R.string.tunnel_daita_padding_title),
                subtitle = stringResource(R.string.tunnel_daita_padding_subtitle),
                value = daita,
                onValueChange = repo::setDaitaEnabled,
            )

            ToggleRow(
                title = stringResource(R.string.tunnel_natpmp_title),
                subtitle = stringResource(R.string.tunnel_natpmp_subtitle),
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
                    statusLabel = natPmpStatusLabel(LocalContext.current, natPmpStatusJson),
                )
            }

            CountryField(
                label = stringResource(R.string.tunnel_exit_country_label),
                initial = exitCountry.orEmpty(),
                onCommit = repo::setExitCountry,
            )

            MultiHopIndicator()

            HorizontalDivider()
            SectionLabel(stringResource(R.string.tunnel_anti_censorship_section))
            ObfuscationIndicator()

            HorizontalDivider()
            SectionLabel(stringResource(R.string.tunnel_exit_key_pinning_section))
            Text(
                text = stringResource(R.string.tunnel_exit_key_pinning_desc),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = { repo.resetExitKeyPins() },
            ) { Text(stringResource(R.string.tunnel_reset_pinned_keys)) }

            OutlinedButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = { navigator.navigate(WarrenLocationPickerNavKey) },
            ) { Text(stringResource(R.string.tunnel_exit_relay)) }
        }
    }
}

/**
 * Read-only anti-censorship status. Warren tunnels run warren-core's QUIC
 * transport, which masquerades as standard browser HTTP/3 traffic (ALPN h3,
 * SNI warrenbrowse.com, Initial-packet split, UDP/443). This M4.0 mimicry is
 * always-on and not togglable: disabling it would make Warren clients
 * immediately recognisable on the network. The legacy Mullvad obfuscation
 * methods (Shadowsocks, UDP-over-TCP, QUIC, LWO) are WireGuard-only and do
 * not apply to Warren tunnels, so no picker is shown - this mirrors the
 * desktop anti-censorship view.
 */
@Composable
private fun MultiHopIndicator() {
    Text(
        text = stringResource(R.string.tunnel_multihop_title),
        style = MaterialTheme.typography.titleSmall,
    )
    Text(
        text = stringResource(R.string.tunnel_multihop_desc),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun ObfuscationIndicator() {
    Text(
        text = stringResource(R.string.tunnel_obfuscation_title),
        style = MaterialTheme.typography.titleSmall,
    )
    Text(
        text = stringResource(R.string.tunnel_obfuscation_desc),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Text(
        text = stringResource(R.string.tunnel_obfuscation_legacy),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/**
 * MTU input. Lets the user lower the TUN MTU (helps on networks that drop or
 * fragment large packets); the repository clamps to a safe range so this can
 * never raise the MTU above the Warren QUIC floor and black-hole traffic.
 */
@Composable
private fun MtuField(mtu: Int, onCommit: (Int) -> Unit) {
    var text by remember { mutableStateOf(mtu.toString()) }
    OutlinedTextField(
        value = text,
        onValueChange = {
            text = it.filter(Char::isDigit).take(4)
            text.toIntOrNull()?.let(onCommit)
        },
        modifier = Modifier.fillMaxWidth(),
        label = { Text(stringResource(R.string.mtu)) },
        placeholder = {
            Text(
                stringResource(
                    R.string.tunnel_mtu_range,
                    WarrenLocalSettingsRepository.MTU_MIN,
                    WarrenLocalSettingsRepository.MTU_MAX,
                ),
            )
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
    )
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
        label = { Text(stringResource(R.string.tunnel_resolver_addresses_label)) },
        placeholder = { Text(stringResource(R.string.tunnel_resolver_addresses_hint)) },
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
    statusLabel: String,
) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(start = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = statusLabel,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.primary,
        )

        Text(stringResource(R.string.tunnel_protocol_label), style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = protocol == "udp",
                onClick = { onProtocolChange("udp") },
                label = { Text(stringResource(R.string.udp)) },
            )
            FilterChip(
                selected = protocol == "tcp",
                onClick = { onProtocolChange("tcp") },
                label = { Text(stringResource(R.string.tcp)) },
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
            label = { Text(stringResource(R.string.tunnel_preferred_port_label)) },
            placeholder = { Text(stringResource(R.string.tunnel_preferred_port_hint)) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        )

        Text(stringResource(R.string.tunnel_mapping_lifetime_label), style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LifetimeChip(stringResource(R.string.tunnel_lifetime_1h), 3_600, lifetimeSecs, onLifetimeChange)
            LifetimeChip(stringResource(R.string.tunnel_lifetime_6h), 21_600, lifetimeSecs, onLifetimeChange)
            LifetimeChip(stringResource(R.string.tunnel_lifetime_24h), 86_400, lifetimeSecs, onLifetimeChange)
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

/**
 * Render the live NAT-PMP status JSON (from `WarrenJni.getNatPmpStatus`)
 * as a human-readable line. Parsed without a JSON dependency since the
 * payload is a small flat object.
 */
internal fun natPmpStatusLabel(context: android.content.Context, json: String): String =
    when (jsonField(json, "state") ?: "idle") {
        "mapped" -> buildString {
            append(context.getString(R.string.tunnel_natpmp_status_mapped))
            jsonField(json, "external_port")?.let {
                append(context.getString(R.string.tunnel_natpmp_status_mapped_port, it))
            }
            jsonField(json, "lifetime_secs")?.let {
                append(context.getString(R.string.tunnel_natpmp_status_mapped_lifetime, it))
            }
        }
        "requesting" -> context.getString(R.string.tunnel_natpmp_status_requesting)
        "rate_limited" ->
            context.getString(R.string.tunnel_natpmp_status_rate_limited) +
                (jsonField(json, "retry_after_secs")?.let {
                    context.getString(R.string.tunnel_natpmp_status_rate_limited_retry, it)
                } ?: "")
        "failed" ->
            context.getString(R.string.tunnel_natpmp_status_failed) +
                (jsonField(json, "reason")?.let {
                    context.getString(R.string.tunnel_natpmp_status_failed_reason, it)
                } ?: "")
        else -> context.getString(R.string.tunnel_natpmp_status_idle)
    }

/** Extract a flat JSON string/number value by key, unquoted, or null. */
internal fun jsonField(json: String, key: String): String? {
    val regex = Regex("\"" + Regex.escape(key) + "\"\\s*:\\s*(?:\"([^\"]*)\"|([^,}\\s]+))")
    val match = regex.find(json) ?: return null
    return match.groupValues[1].ifEmpty { match.groupValues[2] }.ifEmpty { null }
}

@Composable
private fun CountryField(label: String, initial: String, onCommit: (String?) -> Unit) {
    var text by remember { mutableStateOf(initial) }
    OutlinedTextField(
        value = text,
        onValueChange = {
            text = it.filter(Char::isLetter).take(2).uppercase()
            onCommit(text.ifBlank { null })
        },
        modifier = Modifier.fillMaxWidth(),
        label = { Text(label) },
        placeholder = { Text(stringResource(R.string.tunnel_exit_country_hint)) },
        singleLine = true,
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
