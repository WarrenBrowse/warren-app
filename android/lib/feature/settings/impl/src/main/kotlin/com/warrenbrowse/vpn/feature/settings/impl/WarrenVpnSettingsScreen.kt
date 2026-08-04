package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.autoconnect.api.AutoConnectNavKey
import com.warrenbrowse.vpn.lib.repository.AutoStartAndConnectOnBootRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.listitem.NavigationListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeButton
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * VPN settings page (desktop `VpnSettingsView` parity). Carries the toggles
 * read by [com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder] at
 * connect time, in the desktop row order: launch on start-up, auto-connect,
 * local network sharing, DNS content blockers + custom DNS, in-tunnel IPv6,
 * kill switch + lockdown mode, anti-censorship, MTU, and the exit-key pin
 * reset. DAITA, Multihop and Port forwarding each have their own page.
 *
 * Changes here take effect on the next connect; an explicit "Reconnect now" is
 * offered while connected (Android has no live control channel, unlike
 * desktop).
 *
 * Desktop's two top rows both exist here, but Android backs them differently:
 * "Launch app on start-up" is the BOOT_COMPLETED receiver (via
 * [AutoStartAndConnectOnBootRepository]), and desktop's "Auto-connect" is the
 * OS "Always-on VPN" system setting, so its row navigates to the Always-on VPN
 * guidance instead of toggling an app-layer flag.
 *
 * Rows desktop has that Warren's engine does not back are intentionally
 * omitted rather than shown as dead toggles: Allow external DNS resolvers,
 * Quantum-resistant tunnel, Device IP version and Server IP override are all
 * WireGuard / Mullvad-daemon concepts with no Warren client knob.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenVpnSettings(navigator: Navigator) {
    val repo = koinInject<WarrenLocalSettingsRepository>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    // Beta builds show the bandwidth row read-only: the cap there is
    // network-imposed and server-enforced, never a user setting.
    val productFlags = koinInject<WarrenProductFlags>()
    val networkInfoProvider = koinInject<WarrenNetworkInfoProvider>()
    val networkInfo by networkInfoProvider.networkInfo.collectAsStateWithLifecycle()
    val reconnectInvoker = koinInject<WarrenQuinnReconnectInvoker>()
    val autoStartRepo = koinInject<AutoStartAndConnectOnBootRepository>()

    val autoStartOnBoot by autoStartRepo.autoStartAndConnectOnBoot.collectAsStateWithLifecycle()
    val lockdown by repo.lockdownMode.collectAsStateWithLifecycle()
    val ipv6 by repo.ipv6Enabled.collectAsStateWithLifecycle()
    val allowLan by repo.allowLan.collectAsStateWithLifecycle()
    val tunnelMtu by repo.tunnelMtu.collectAsStateWithLifecycle()
    val maxRateBps by repo.maxRateBps.collectAsStateWithLifecycle()
    val dnsState by repo.dnsState.collectAsStateWithLifecycle()
    val customDns by repo.customDnsServers.collectAsStateWithLifecycle()
    val blockAds by repo.blockAds.collectAsStateWithLifecycle()
    val blockTrackers by repo.blockTrackers.collectAsStateWithLifecycle()
    val blockMalware by repo.blockMalware.collectAsStateWithLifecycle()
    val blockAdult by repo.blockAdultContent.collectAsStateWithLifecycle()
    val blockGambling by repo.blockGambling.collectAsStateWithLifecycle()
    val blockSocial by repo.blockSocialMedia.collectAsStateWithLifecycle()
    val connectedInfo by tunnelStateProvider.connectedInfo.collectAsStateWithLifecycle()

    val customDnsEnabled = dnsState == WarrenLocalSettingsRepository.DNS_STATE_CUSTOM
    // The exclusion is symmetric on desktop: blockers off while a custom
    // resolver is set, AND the custom-DNS switch off while any blocker is on.
    // Enforcing only the first direction let a user silently kill the blockers.
    val blockersOn =
        blockAds || blockTrackers || blockMalware || blockGambling || blockAdult || blockSocial

    var confirmResetPins by rememberSaveable { mutableStateOf(false) }
    var resetPinsCount by rememberSaveable { mutableStateOf<Int?>(null) }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.tunnel_settings_title),
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
            verticalArrangement = Arrangement.spacedBy(Dimens.listItemDivider),
        ) {
            TunnelChangesBanner(connectedInfo) { reconnectInvoker.reconnect() }

            // Launch on start-up (desktop's top row). On Android this enables
            // the BOOT_COMPLETED receiver, which starts and connects the tunnel
            // on device boot; it is independent of the connect-time knobs below.
            Spacer(Modifier.height(Dimens.smallPadding))
            ToggleCell(
                title = stringResource(R.string.tunnel_launch_on_startup_title),
                subtitle = stringResource(R.string.tunnel_launch_on_startup_subtitle),
                value = autoStartOnBoot,
                onValueChange = autoStartRepo::setAutoStartAndConnectOnBoot,
                position = Position.Top,
            )
            // Desktop's second top row. Android's equivalent is the OS
            // Always-on VPN setting, which no app flag can set, so the row
            // opens the guidance that walks the user through it.
            NavigationListItem(
                title = stringResource(R.string.tunnel_auto_connect_title),
                subtitle = stringResource(R.string.tunnel_auto_connect_subtitle),
                position = Position.Bottom,
                onClick = dropUnlessResumed { navigator.navigate(AutoConnectNavKey) },
            )

            // Local network sharing.
            Spacer(Modifier.height(Dimens.smallPadding))
            ToggleCell(
                title = stringResource(R.string.tunnel_local_network_title),
                subtitle = stringResource(R.string.tunnel_local_network_subtitle),
                value = allowLan,
                onValueChange = repo::setAllowLan,
                position = Position.Single,
                infoParagraphs = listOf(
                    stringResource(R.string.tunnel_local_network_info_1),
                    stringResource(R.string.tunnel_local_network_info_2) +
                        "\n" + stringResource(R.string.local_network_sharing_ip_ranges_plain),
                ),
            )

            // DNS content blockers, in the desktop accordion order: collapsed
            // by default so the six rows do not push the rest of the page down,
            // with the explanation desktop puts behind its info button. Mutually
            // exclusive with custom DNS (the exit resolver, not the custom one,
            // is what enforces content blocking).
            Spacer(Modifier.height(Dimens.smallPadding))
            DnsBlockerSection(
                enabled = !customDnsEnabled,
                enabledCount = listOf(
                    blockAds,
                    blockTrackers,
                    blockMalware,
                    blockGambling,
                    blockAdult,
                    blockSocial,
                ).count { it },
            ) {
                ToggleCell(
                    title = stringResource(R.string.tunnel_block_ads),
                    subtitle = "",
                    value = blockAds,
                    onValueChange = repo::setBlockAds,
                    position = Position.Top,
                    enabled = !customDnsEnabled,
                )
                ToggleCell(
                    title = stringResource(R.string.tunnel_block_trackers),
                    subtitle = "",
                    value = blockTrackers,
                    onValueChange = repo::setBlockTrackers,
                    position = Position.Middle,
                    enabled = !customDnsEnabled,
                )
                ToggleCell(
                    title = stringResource(R.string.tunnel_block_malware),
                    subtitle = "",
                    value = blockMalware,
                    onValueChange = repo::setBlockMalware,
                    position = Position.Middle,
                    enabled = !customDnsEnabled,
                )
                ToggleCell(
                    title = stringResource(R.string.tunnel_block_gambling),
                    subtitle = "",
                    value = blockGambling,
                    onValueChange = repo::setBlockGambling,
                    position = Position.Middle,
                    enabled = !customDnsEnabled,
                )
                ToggleCell(
                    title = stringResource(R.string.tunnel_block_adult_content),
                    subtitle = "",
                    value = blockAdult,
                    onValueChange = repo::setBlockAdultContent,
                    position = Position.Middle,
                    enabled = !customDnsEnabled,
                )
                ToggleCell(
                    title = stringResource(R.string.tunnel_block_social_media),
                    subtitle = "",
                    value = blockSocial,
                    onValueChange = repo::setBlockSocialMedia,
                    position = Position.Bottom,
                    enabled = !customDnsEnabled,
                )
            }

            Spacer(Modifier.height(Dimens.smallPadding))
            ToggleCell(
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
                position = Position.Single,
                enabled = !blockersOn,
            )

            if (blockersOn) {
                SettingsFootnote(stringResource(R.string.tunnel_custom_dns_blocked_by_blockers))
            }

            if (customDnsEnabled) {
                SettingsFootnote(stringResource(R.string.tunnel_custom_dns_blockers_disabled))
                CustomDnsField(
                    initial = customDns.joinToString(", "),
                    onCommit = repo::setCustomDnsServers,
                )
            }

            // In-tunnel IPv6.
            Spacer(Modifier.height(Dimens.smallPadding))
            ToggleCell(
                title = stringResource(R.string.tunnel_ipv6_title),
                subtitle = stringResource(R.string.tunnel_ipv6_subtitle),
                value = ipv6,
                onValueChange = repo::setIpv6Enabled,
                position = Position.Single,
                infoParagraphs = listOf(
                    stringResource(R.string.tunnel_ipv6_info_1),
                    stringResource(R.string.tunnel_ipv6_info_2),
                ),
            )

            // Kill switch is always on (Android's VpnService blocks non-tunnel
            // traffic whenever the tunnel is up): an info row, not a toggle.
            // Lockdown mode is the separate opt-in that also blocks while
            // deliberately disconnected. Mirrors desktop's two rows, each
            // carrying the modal that states the distinction between them.
            Spacer(Modifier.height(Dimens.smallPadding))
            KillSwitchInfoRow()
            Spacer(Modifier.height(Dimens.smallPadding))
            ToggleCell(
                title = stringResource(R.string.tunnel_lockdown_mode_title),
                subtitle = stringResource(R.string.tunnel_lockdown_mode_subtitle),
                value = lockdown,
                onValueChange = repo::setLockdownMode,
                position = Position.Single,
                infoParagraphs = listOf(
                    stringResource(R.string.tunnel_lockdown_mode_info_1),
                    stringResource(R.string.tunnel_lockdown_mode_info_2),
                ),
            )

            SectionHeader(stringResource(R.string.tunnel_anti_censorship_section))
            ObfuscationIndicator(connectedInfo)

            SectionHeader(stringResource(R.string.mtu))
            MtuField(mtu = tunnelMtu, onCommit = repo::setTunnelMtu)

            SectionHeader(stringResource(R.string.max_bandwidth))
            if (productFlags.isBeta) {
                val capMbps = networkInfo?.defaultRateBps
                    ?.let { (it / 1_000_000L).toInt() }
                    ?.takeIf { it > 0 }
                Text(
                    text = if (capMbps != null) {
                        stringResource(R.string.max_bandwidth_mbps_value, capMbps)
                    } else {
                        stringResource(R.string.max_bandwidth_managed)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = Dimens.smallPadding),
                )
                SettingsFootnote(stringResource(R.string.max_bandwidth_beta_hint))
            } else {
                MaxRateField(maxRateBps = maxRateBps, onCommit = repo::setMaxRateBps)
            }

            SectionHeader(stringResource(R.string.tunnel_exit_key_pinning_section))
            SettingsFootnote(stringResource(R.string.tunnel_exit_key_pinning_desc))
            NegativeButton(
                modifier = Modifier.fillMaxWidth().padding(top = Dimens.smallPadding),
                // A reset disarms exit-key substitution detection until every
                // exit is re-pinned, so it is never a single tap: confirm
                // first, then report how many entries went (desktop parity).
                onClick = { confirmResetPins = true },
                text = stringResource(R.string.tunnel_reset_pinned_keys),
            )
        }
    }

    if (confirmResetPins) {
        AlertDialog(
            onDismissRequest = { confirmResetPins = false },
            title = { Text(stringResource(R.string.tunnel_reset_pinned_keys_confirm_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
                    Text(stringResource(R.string.tunnel_reset_pinned_keys_confirm_body_1))
                    Text(stringResource(R.string.tunnel_reset_pinned_keys_confirm_body_2))
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        resetPinsCount = repo.resetExitKeyPins()
                        confirmResetPins = false
                    },
                ) {
                    Text(
                        text = stringResource(R.string.tunnel_reset_pinned_keys_confirm),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmResetPins = false }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    }

    resetPinsCount?.let { count ->
        AlertDialog(
            onDismissRequest = { resetPinsCount = null },
            title = { Text(stringResource(R.string.tunnel_reset_pinned_keys_done_title)) },
            text = { Text(stringResource(R.string.tunnel_reset_pinned_keys_done_body, count)) },
            confirmButton = {
                TextButton(onClick = { resetPinsCount = null }) {
                    Text(stringResource(R.string.got_it))
                }
            },
        )
    }
}

/** Secondary explanatory line under a setting, matching desktop's footers. */
@Composable
private fun SettingsFootnote(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = Dimens.smallPadding),
    )
}

/**
 * Collapsible DNS-content-blockers block (desktop `SettingsAccordion`):
 * collapsed by default with an on-count, an info affordance carrying the three
 * desktop paragraphs, and a chevron revealing the six toggles.
 */
@Composable
private fun DnsBlockerSection(
    enabled: Boolean,
    enabledCount: Int,
    content: @Composable () -> Unit,
) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    var showInfo by rememberSaveable { mutableStateOf(false) }
    val title = stringResource(R.string.tunnel_content_blocking_header)
    WarrenListItem(
        position = if (expanded) Position.Top else Position.Single,
        onClick = { expanded = !expanded },
        content = {
            Column(modifier = Modifier.align(Alignment.CenterStart)) {
                Text(text = title, style = MaterialTheme.typography.titleSmall)
                Text(
                    text = if (enabledCount == 0) {
                        stringResource(R.string.tunnel_content_blocking_none_enabled)
                    } else {
                        stringResource(R.string.tunnel_content_blocking_enabled_count, enabledCount)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        trailingContent = {
            Row(
                modifier = Modifier.align(Alignment.Center),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                SettingsInfoIcon(title = title, onClick = { showInfo = true })
                Icon(
                    imageVector = if (expanded) {
                        Icons.Rounded.ExpandLess
                    } else {
                        Icons.Rounded.ExpandMore
                    },
                    contentDescription = null,
                )
            }
        },
        isEnabled = enabled,
    )
    AnimatedVisibility(visible = expanded) {
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.listItemDivider)) { content() }
    }
    if (showInfo) {
        SettingsInfoDialog(
            title = title,
            paragraphs = listOf(
                stringResource(R.string.tunnel_content_blocking_info_1),
                stringResource(R.string.tunnel_content_blocking_info_2),
                stringResource(R.string.tunnel_content_blocking_info_3),
            ),
            onDismiss = { showInfo = false },
        )
    }
}

/**
 * Kill switch info row (desktop parity): the built-in leak protection is
 * always on, so it reads as an on, non-togglable statement rather than a
 * switch. The togglable part is Lockdown mode, and the modal here is the one
 * that states the difference between the two.
 */
@Composable
private fun KillSwitchInfoRow() {
    var showInfo by rememberSaveable { mutableStateOf(false) }
    val title = stringResource(R.string.tunnel_kill_switch_title)
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = stringResource(R.string.tunnel_kill_switch_subtitle),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        SettingsInfoIcon(title = title, onClick = { showInfo = true })
    }
    if (showInfo) {
        SettingsInfoDialog(
            title = title,
            paragraphs = listOf(
                stringResource(R.string.tunnel_kill_switch_info_1),
                stringResource(R.string.tunnel_kill_switch_info_2),
            ),
            onDismiss = { showInfo = false },
        )
    }
}

/**
 * Anti-censorship status. Warren tunnels masquerade as ordinary HTTP/3 web
 * browsing; the mimicry is always-on and not togglable, so this is an
 * indicator rather than a picker (the legacy Mullvad obfuscation methods are
 * WireGuard-only and do not apply). The first line reports the LIVE state so
 * the user can verify it, as on desktop, rather than asserting it statically.
 */
@Composable
private fun ObfuscationIndicator(info: WarrenConnectedInfo) {
    // Mimicry is unconditional on a live Warren tunnel; off the tunnel there is
    // nothing to obfuscate, so the indicator reads inactive.
    val active = info is WarrenConnectedInfo.Connected
    Column {
        Text(
            text = if (active) {
                stringResource(R.string.tunnel_obfuscation_active)
            } else {
                stringResource(R.string.tunnel_obfuscation_inactive)
            },
            style = MaterialTheme.typography.titleSmall,
            color = if (active) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
        )
        Text(
            text = stringResource(R.string.tunnel_obfuscation_desc),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
