package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.ui.designsystem.ListHeader
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.SmallPrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaDisabled
import kotlinx.coroutines.delay

/**
 * Shared building blocks for the Warren settings pages (DAITA, Multihop,
 * Port forwarding, VPN settings). Each page changes a connect-time knob, so
 * they present the same "changes apply on next connect" affordance and reuse
 * the same list cells, pickers and NAT-PMP status helpers.
 */

/**
 * Live tunnel state plus the "changes apply on next connect" affordance. On
 * Android the settings feed the tunnel only at (re)connect time (there is no
 * live control channel like desktop), so any page that changes a connect-time
 * knob shows this: while connected it offers an explicit "Reconnect now" that
 * tears down and re-dials with the new config (reusing the cached mnemonic,
 * no biometric re-prompt).
 *
 * It consumes the typed state rather than a display string so every state and
 * every feature chip resolves to a localized resource; the engine's failure
 * reason is developer text and stays in the log.
 */
@Composable
internal fun TunnelChangesBanner(info: WarrenConnectedInfo, onReconnect: () -> Unit) {
    val connected = tunnelStateIsConnected(info)
    Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
        Text(
            text = stringResource(
                R.string.tunnel_state_line,
                tunnelStateLabel(LocalContext.current, info),
            ),
            style = MaterialTheme.typography.titleSmall,
            color = when {
                connected -> MaterialTheme.colorScheme.primary
                info is WarrenConnectedInfo.Blocking || info is WarrenConnectedInfo.Failed ->
                    MaterialTheme.colorScheme.error
                else -> MaterialTheme.colorScheme.onSurface
            },
        )
        if (connected) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
            ) {
                Text(
                    text = stringResource(R.string.tunnel_changes_apply_next_connect),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.weight(1f),
                )
                SmallPrimaryButton(
                    onClick = onReconnect,
                    text = stringResource(R.string.tunnel_reconnect_now),
                )
            }
        } else {
            Text(
                text = stringResource(R.string.tunnel_changes_apply_next_connect),
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

/** True while the tunnel is up, which is the only state offering a reconnect. */
internal fun tunnelStateIsConnected(info: WarrenConnectedInfo): Boolean =
    info is WarrenConnectedInfo.Connected

/**
 * Localized one-line description of [info]. A failure renders a generic line:
 * the engine reason names internal call sites, so it stays in the log rather
 * than reaching a settings page in untranslatable English.
 */
internal fun tunnelStateLabel(
    context: android.content.Context,
    info: WarrenConnectedInfo,
): String = when (info) {
    is WarrenConnectedInfo.Disconnected -> context.getString(R.string.tunnel_state_disconnected)
    is WarrenConnectedInfo.Connecting -> context.getString(R.string.tunnel_state_connecting)
    is WarrenConnectedInfo.Reconnecting -> context.getString(R.string.tunnel_state_reconnecting)
    is WarrenConnectedInfo.Disconnecting ->
        if (info.reconnecting) {
            context.getString(R.string.tunnel_state_reconnecting)
        } else {
            context.getString(R.string.tunnel_state_disconnecting)
        }
    is WarrenConnectedInfo.Connected -> {
        val features = listOfNotNull(
            if (info.multiHop) context.getString(R.string.multihop) else null,
            if (info.daita) context.getString(R.string.daita) else null,
            // HTTP/3 mimicry is always-on for Warren tunnels (not togglable),
            // so it is listed unconditionally rather than gated on a setting.
            context.getString(R.string.tunnel_feature_mimicry),
            info.assignedNatPmpPort?.let {
                context.getString(R.string.tunnel_feature_port, it.toString())
            },
        )
        context.getString(
            R.string.tunnel_state_with_features,
            context.getString(R.string.tunnel_state_connected),
            features.joinToString(),
        )
    }
    is WarrenConnectedInfo.Failed -> context.getString(R.string.tunnel_state_failed)
    is WarrenConnectedInfo.Blocking -> context.getString(R.string.tunnel_state_blocking)
}

/** Section header matching the upstream relay-list style (label + divider). */
@Composable
internal fun SectionHeader(text: String) {
    ListHeader(modifier = Modifier.padding(top = Dimens.mediumPadding), text = text)
}

/**
 * A single settings toggle styled as an upstream Mullvad list cell: title (+
 * optional subtitle) with a trailing switch, rounded into a block via
 * [position]. Tapping anywhere on the row flips the switch. [enabled] mirrors
 * the desktop mutual exclusion (the DNS content blockers are disabled while
 * custom DNS is in use): a disabled cell is dimmed and ignores taps.
 *
 * [infoParagraphs] carries the desktop info-button modal: when non-empty an
 * info icon sits before the switch and opens the paragraphs in a dialog. The
 * rows it exists for (kill switch vs lockdown, the LAN ranges, the IPv6
 * recommendation) answer a question a one-line subtitle cannot.
 */
@Composable
internal fun ToggleCell(
    title: String,
    subtitle: String,
    value: Boolean,
    onValueChange: (Boolean) -> Unit,
    position: Position,
    enabled: Boolean = true,
    infoParagraphs: List<String> = emptyList(),
) {
    val contentAlpha = if (enabled) 1f else AlphaDisabled
    var showInfo by remember { mutableStateOf(false) }
    WarrenListItem(
        position = position,
        onClick = if (enabled) {
            { onValueChange(!value) }
        } else {
            {}
        },
        content = {
            Column(modifier = Modifier.align(Alignment.CenterStart)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = contentAlpha),
                )
                if (subtitle.isNotEmpty()) {
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = contentAlpha),
                    )
                }
            }
        },
        trailingContent = {
            Row(
                modifier = Modifier.align(Alignment.Center),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (infoParagraphs.isNotEmpty()) {
                    SettingsInfoIcon(title = title, onClick = { showInfo = true })
                }
                WarrenSwitch(
                    checked = value,
                    onCheckedChange = onValueChange,
                    enabled = enabled,
                )
            }
        },
    )
    if (showInfo) {
        SettingsInfoDialog(
            title = title,
            paragraphs = infoParagraphs,
            onDismiss = { showInfo = false },
        )
    }
}

/** Info affordance shared by the settings rows carrying a desktop modal. */
@Composable
internal fun SettingsInfoIcon(title: String, onClick: () -> Unit) {
    IconButton(onClick = onClick) {
        Icon(
            imageVector = Icons.Rounded.Info,
            contentDescription = stringResource(R.string.more_information) + ": " + title,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** The desktop info-button modal, one paragraph per `ModalMessage`. */
@Composable
internal fun SettingsInfoDialog(
    title: String,
    paragraphs: List<String>,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
                paragraphs.forEach { Text(it) }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.got_it)) }
        },
    )
}

/**
 * Entry/exit country picker, driven by the signed relay catalogue. "Automatic"
 * (the first option) stores null so the native selector auto-picks. The chosen
 * country is matched case-insensitively against the relay list at connect time
 * (exit: [com.warrenbrowse.vpn.app.connect.WarrenTunnelConfigBuilder]; entry:
 * native `run_multi_hop_session`).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CountryDropdown(
    label: String,
    automaticLabel: String,
    options: List<Pair<String, String>>,
    selected: String?,
    onSelect: (String?) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val selectedDisplay = selected
        ?.takeIf { it.isNotBlank() }
        ?.let { code -> options.firstOrNull { it.first == code }?.second ?: code }
    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { expanded = it },
        modifier = Modifier.fillMaxWidth().padding(top = Dimens.smallPadding),
    ) {
        OutlinedTextField(
            value = selectedDisplay ?: automaticLabel,
            onValueChange = {},
            readOnly = true,
            label = { Text(label) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier = Modifier
                .fillMaxWidth()
                .menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(automaticLabel) },
                onClick = {
                    onSelect(null)
                    expanded = false
                },
            )
            options.forEach { (code, display) ->
                DropdownMenuItem(
                    text = { Text(display) },
                    onClick = {
                        onSelect(code)
                        expanded = false
                    },
                )
            }
        }
    }
}

/**
 * A numeric settings field that persists only a valid value (desktop parity).
 *
 * The typed text lives in a local buffer: committing on every keystroke would
 * persist each prefix of the number ("1", "12", "120" before "1200"), each one
 * clamped to something the user never asked for. So the value is written on
 * focus loss or IME Done, and only when [isValid] accepts it; an invalid
 * buffer is reverted to [storedText] instead, exactly as the desktop settings
 * call `reset()`.
 */
@Composable
private fun ValidatedNumberField(
    storedText: String,
    label: String,
    placeholder: String,
    supportingText: String,
    errorText: String,
    maxLength: Int,
    isValid: (String) -> Boolean,
    onCommit: (String) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    var text by remember(storedText) { mutableStateOf(storedText) }
    val valid = isValid(text)
    val keyboardController = LocalSoftwareKeyboardController.current
    val commit = {
        // Focus is also lost on first composition, so an unchanged buffer must
        // not write: it would re-persist the stored value on every screen open.
        when {
            text == storedText -> Unit
            isValid(text) -> onCommit(text)
            else -> text = storedText
        }
    }
    OutlinedTextField(
        value = text,
        onValueChange = { text = it.filter(Char::isDigit).take(maxLength) },
        modifier = modifier
            .fillMaxWidth()
            .padding(top = Dimens.smallPadding)
            .onFocusChanged { if (!it.isFocused) commit() },
        label = { Text(label) },
        placeholder = { Text(placeholder) },
        singleLine = true,
        enabled = enabled,
        isError = !valid,
        supportingText = { Text(if (valid) supportingText else errorText) },
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Number,
            imeAction = ImeAction.Done,
        ),
        keyboardActions = KeyboardActions(
            onDone = {
                commit()
                keyboardController?.hide()
            },
        ),
    )
}

/**
 * MTU input. Lets the user lower the TUN MTU (helps on networks that drop or
 * fragment large packets); the range keeps this from raising the MTU above the
 * Warren QUIC floor and black-holing traffic.
 *
 * The field is empty (showing the "Default" placeholder) while the MTU is at
 * its default, mirroring desktop. Clearing the field resets to the default:
 * VpnService always needs a concrete MTU, so there is no true "unset".
 */
@Composable
internal fun MtuField(mtu: Int, onCommit: (Int) -> Unit) {
    ValidatedNumberField(
        storedText = mtuFieldDisplay(mtu),
        label = stringResource(R.string.mtu),
        placeholder = stringResource(R.string.tunnel_mtu_default),
        supportingText = stringResource(
            R.string.tunnel_mtu_hint,
            SettingsRanges.MTU_MIN,
            SettingsRanges.MTU_MAX,
        ),
        errorText = stringResource(
            R.string.tunnel_mtu_invalid,
            SettingsRanges.MTU_MIN,
            SettingsRanges.MTU_MAX,
        ),
        maxLength = 4,
        isValid = ::mtuInputIsValid,
        onCommit = { onCommit(mtuFromInput(it)) },
    )
}

/**
 * Bandwidth-ceiling input in Mbps (stored as bits per second in the
 * repository; 0 = unlimited). Mirrors [MtuField]: empty field = no limit
 * ("Unlimited" placeholder). Applies on the next connection.
 */
@Composable
internal fun MaxRateField(maxRateBps: Long, onCommit: (Long) -> Unit) {
    ValidatedNumberField(
        storedText = maxRateFieldDisplay(maxRateBps),
        label = stringResource(R.string.max_bandwidth),
        placeholder = stringResource(R.string.max_bandwidth_unlimited),
        supportingText = stringResource(
            R.string.max_bandwidth_hint,
            SettingsRanges.MAX_RATE_MBPS_MIN,
            SettingsRanges.MAX_RATE_MBPS_MAX,
        ),
        errorText = stringResource(
            R.string.max_bandwidth_invalid,
            SettingsRanges.MAX_RATE_MBPS_MIN,
            SettingsRanges.MAX_RATE_MBPS_MAX,
        ),
        maxLength = 5,
        isValid = ::maxRateInputIsValid,
        onCommit = { onCommit(maxRateBpsFromInput(it)) },
    )
}

/**
 * Custom resolver list. The addresses here are what the tunnel is built with,
 * so a half-typed one must never be persisted: the buffer is validated per
 * token (each must parse as a numeric IP, no duplicates) and committed on
 * focus loss or IME Done.
 */
@Composable
internal fun CustomDnsField(initial: String, onCommit: (List<String>) -> Unit) {
    var text by remember(initial) { mutableStateOf(initial) }
    val valid = customDnsInputIsValid(text)
    val keyboardController = LocalSoftwareKeyboardController.current
    val commit = {
        when {
            text == initial -> Unit
            customDnsInputIsValid(text) -> onCommit(customDnsServersFromInput(text))
            else -> text = initial
        }
    }
    OutlinedTextField(
        value = text,
        onValueChange = { text = it },
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = Dimens.smallPadding)
            .onFocusChanged { if (!it.isFocused) commit() },
        label = { Text(stringResource(R.string.tunnel_resolver_addresses_label)) },
        placeholder = { Text(stringResource(R.string.tunnel_resolver_addresses_hint)) },
        singleLine = true,
        isError = !valid,
        supportingText = {
            if (!valid) Text(stringResource(R.string.tunnel_resolver_addresses_invalid))
        },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(
            onDone = {
                commit()
                keyboardController?.hide()
            },
        ),
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun PortForwardingAdvanced(
    protocol: String,
    onProtocolChange: (String) -> Unit,
    externalPort: Int,
    onExternalPortChange: (Int) -> Unit,
    lifetimeSecs: Int,
    onLifetimeChange: (Int) -> Unit,
    statusLabel: String,
    portConflict: Boolean,
    portBlock: NatPmpPortBlock,
    onAssignFreePort: () -> Unit,
    onChooseAnotherExit: () -> Unit,
) {
    val controlsEnabled = !portBlock.blocked
    Column(
        modifier = Modifier.fillMaxWidth().padding(start = Dimens.mediumPadding),
        verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        Text(
            text = statusLabel,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.primary,
        )

        NatPmpRateLimitNotice(portBlock)

        // The followed port is taken on this exit: offer the two recoveries
        // (let the exit pick a free port, or switch exit). Editing the port
        // field below is the third option. Both recoveries request a new
        // allocation, so they are withheld while the rate-limit window is
        // open; the countdown above says why.
        if (portConflict && controlsEnabled) {
            Row(horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                SmallPrimaryButton(
                    onClick = onAssignFreePort,
                    text = stringResource(R.string.tunnel_natpmp_conflict_assign_free),
                )
                SmallPrimaryButton(
                    onClick = onChooseAnotherExit,
                    text = stringResource(R.string.tunnel_natpmp_conflict_change_exit),
                )
            }
        }

        ProtocolChips(protocol, onProtocolChange, controlsEnabled)

        ValidatedNumberField(
            storedText = if (externalPort == 0) "" else externalPort.toString(),
            label = stringResource(R.string.tunnel_preferred_port_label),
            placeholder = stringResource(R.string.tunnel_preferred_port_hint),
            supportingText = stringResource(
                R.string.tunnel_preferred_port_range_hint,
                SettingsRanges.NATPMP_PORT_MIN,
                SettingsRanges.NATPMP_PORT_MAX,
            ),
            errorText = stringResource(
                R.string.tunnel_preferred_port_invalid,
                SettingsRanges.NATPMP_PORT_MIN,
                SettingsRanges.NATPMP_PORT_MAX,
            ),
            maxLength = 5,
            isValid = ::natPmpPortInputIsValid,
            onCommit = { onExternalPortChange(natPmpPortFromInput(it)) },
            enabled = controlsEnabled,
        )

        Text(
            text = stringResource(R.string.tunnel_mapping_lifetime_label),
            style = MaterialTheme.typography.bodySmall,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
            LifetimeChip(
                label = stringResource(R.string.tunnel_lifetime_1h),
                seconds = LIFETIME_1H_SECS,
                selected = lifetimeSecs,
                onSelect = onLifetimeChange,
                enabled = controlsEnabled,
            )
            LifetimeChip(
                label = stringResource(R.string.tunnel_lifetime_6h),
                seconds = LIFETIME_6H_SECS,
                selected = lifetimeSecs,
                onSelect = onLifetimeChange,
                enabled = controlsEnabled,
            )
            LifetimeChip(
                label = stringResource(R.string.tunnel_lifetime_24h),
                seconds = LIFETIME_24H_SECS,
                selected = lifetimeSecs,
                onSelect = onLifetimeChange,
                enabled = controlsEnabled,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProtocolChips(
    protocol: String,
    onProtocolChange: (String) -> Unit,
    enabled: Boolean,
) {
    Text(
        text = stringResource(R.string.tunnel_protocol_label),
        style = MaterialTheme.typography.bodySmall,
    )
    Row(horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
        FilterChip(
            selected = protocol == "udp",
            onClick = { onProtocolChange("udp") },
            enabled = enabled,
            label = { Text(stringResource(R.string.udp)) },
        )
        FilterChip(
            selected = protocol == "tcp",
            onClick = { onProtocolChange("tcp") },
            enabled = enabled,
            label = { Text(stringResource(R.string.tcp)) },
        )
    }
}

private const val LIFETIME_1H_SECS = 3_600
private const val LIFETIME_6H_SECS = 21_600
private const val LIFETIME_24H_SECS = 86_400

/**
 * The open rate-limit window, stated with a ticking countdown: blocked while
 * the exit refuses allocations, a heads-up when exactly one slot is left.
 */
@Composable
private fun NatPmpRateLimitNotice(portBlock: NatPmpPortBlock) {
    val message = when {
        portBlock.blocked -> stringResource(
            R.string.tunnel_natpmp_rate_limited_countdown,
            formatCountdown(portBlock.remainingSecs),
        )
        portBlock.lastChance -> stringResource(
            R.string.tunnel_natpmp_last_chance,
            formatCountdown(portBlock.remainingSecs),
        )
        else -> return
    }
    Text(
        text = message,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.error,
    )
}

/**
 * Ticking rate-limit verdict for the live NAT-PMP status. The window is
 * anchored on when THIS snapshot arrived, not on composition: the exit only
 * pushes a status on a mapping event, so anchoring on mount would keep a stale
 * snapshot's window alive forever and strand the controls.
 */
private const val COUNTDOWN_TICK_MS = 1_000L

@Composable
internal fun rememberNatPmpPortBlock(json: String): NatPmpPortBlock {
    var elapsedSecs by remember(json) { mutableIntStateOf(0) }
    val block = natPmpPortBlock(json, elapsedSecs.toLong())
    val counting = block.blocked || block.lastChance
    LaunchedEffect(json, counting) {
        while (counting) {
            delay(COUNTDOWN_TICK_MS)
            elapsedSecs += 1
        }
    }
    return block
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LifetimeChip(
    label: String,
    seconds: Int,
    selected: Int,
    onSelect: (Int) -> Unit,
    enabled: Boolean,
) {
    FilterChip(
        selected = selected == seconds,
        onClick = { onSelect(seconds) },
        enabled = enabled,
        label = { Text(label) },
    )
}

/**
 * Text shown in the MTU field for a stored [mtu]: empty at the default so the
 * "Default" placeholder shows (desktop parity), otherwise the number. The
 * repository default is [WarrenLocalSettingsRepository.MTU_MAX] (the Warren
 * QUIC floor), which is the "no override" value.
 */
internal fun mtuFieldDisplay(mtu: Int): String =
    if (mtu == WarrenLocalSettingsRepository.MTU_MAX) "" else mtu.toString()

/**
 * The MTU to persist for a raw field [input]: the default when blank (clearing
 * the field resets to default, since VpnService requires a concrete MTU), the
 * parsed number otherwise. Only in-range input reaches this.
 */
internal fun mtuFromInput(input: String): Int =
    input.toIntOrNull() ?: WarrenLocalSettingsRepository.MTU_MAX

/**
 * Text shown in the bandwidth field for a stored [bps] figure: empty when
 * unlimited (0), otherwise the whole number of Mbps.
 */
internal fun maxRateFieldDisplay(bps: Long): String =
    if (bps <= 0L) "" else (bps / 1_000_000L).toString()

/**
 * The bits-per-second figure to persist for a raw Mbps field [input]:
 * 0 (unlimited) when blank, else Mbps * 1e6.
 */
internal fun maxRateBpsFromInput(input: String): Long =
    (input.toLongOrNull() ?: 0L).coerceAtLeast(0L) * 1_000_000L

/**
 * Render the live NAT-PMP status JSON (from `WarrenJni.getNatPmpStatus`)
 * as a human-readable line. Parsed without a JSON dependency since the
 * payload is a small flat object.
 *
 * [tunnelConnected] splits the fallback: only a live tunnel can be idle, so
 * without one the line names the actual cause instead of leaving the user
 * unable to tell a broken feature from one that is simply waiting.
 */
internal fun natPmpStatusLabel(
    context: android.content.Context,
    tunnelConnected: Boolean,
    json: String,
): String =
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
                when (val reason = jsonField(json, "reason")) {
                    // A followed port taken by another client on a new exit:
                    // surface a friendly, actionable line instead of the raw
                    // enum name (the exit applies strict honour-or-error).
                    "SuggestedPortInUse" ->
                        context.getString(R.string.tunnel_natpmp_status_failed_port_in_use)
                    null -> ""
                    else -> context.getString(R.string.tunnel_natpmp_status_failed_reason, reason)
                }
        else -> natPmpNoMappingLabel(context, tunnelConnected)
    }

/**
 * The line shown when no mapping is live. Only a connected tunnel can be idle;
 * without one the cause is named, so the user can tell a feature that is
 * waiting from one that is broken.
 */
private fun natPmpNoMappingLabel(
    context: android.content.Context,
    tunnelConnected: Boolean,
): String =
    if (tunnelConnected) {
        context.getString(R.string.tunnel_natpmp_status_idle)
    } else {
        context.getString(R.string.tunnel_natpmp_status_waiting_tunnel)
    }

/**
 * True when the live NAT-PMP status reports a followed-port conflict: the
 * port the client asked to keep is already in use by another client on this
 * exit (the exit applies strict honour-or-error). The settings screen then
 * offers the user a way out (let the exit pick a free port, or switch exit).
 */
internal fun natPmpIsPortConflict(json: String): Boolean =
    jsonField(json, "state") == "failed" && jsonField(json, "reason") == "SuggestedPortInUse"

/** Extract a flat JSON string/number value by key, unquoted, or null. */
internal fun jsonField(json: String, key: String): String? {
    val regex = Regex("\"" + Regex.escape(key) + "\"\\s*:\\s*(?:\"([^\"]*)\"|([^,}\\s]+))")
    val match = regex.find(json) ?: return null
    return match.groupValues[1].ifEmpty { match.groupValues[2] }.ifEmpty { null }
}
