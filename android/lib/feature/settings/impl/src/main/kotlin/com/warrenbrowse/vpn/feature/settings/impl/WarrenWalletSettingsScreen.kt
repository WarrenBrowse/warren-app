package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Alignment
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceListOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceRemoveOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenDeviceSummary
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * D.5 wallet settings host screen. Wraps [WarrenWalletSettingsSection]
 * inside the standard settings scaffold and threads the host
 * [FragmentActivity] required by `BiometricPrompt`.
 *
 * Reached via [com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey]
 * from the main Settings screen ("Wallet" entry).
 *
 * Hosts a "Warren Connect (test)" button (D.4 step 7 follow-up) that
 * dispatches the end-to-end Quinn connect flow via the
 * `WarrenQuinnConnectInvoker` interface (registered in app/AppModule
 * pointing at `WarrenConnectUseCase`). The button lives here, behind
 * the wallet settings, so we can exercise the connect path without
 * having to refactor the main Connect button's wiring across the
 * lib/feature/home boundary.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenWalletSettings(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val walletRepository = koinInject<WalletRepository>()
    val quinnConnect = koinInject<WarrenQuinnConnectInvoker>()
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val cachedExpiry by settings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val tunnelState by tunnelStateProvider.state.collectAsStateWithLifecycle()
    val deviceInvoker = koinInject<WarrenDeviceInvoker>()
    val scope = rememberCoroutineScope()
    var connectStatus by remember { mutableStateOf<String?>(null) }
    var subscriptionStatus by remember { mutableStateOf<String?>(null) }
    var voucherInput by remember { mutableStateOf("") }
    var devices by remember { mutableStateOf<List<WarrenDeviceSummary>?>(null) }
    var deviceStatus by remember { mutableStateOf<String?>(null) }

    ScaffoldWithSmallTopBar(
        appBarTitle = "Wallet",
        navigationIcon = {
            NavigateBackIconButton(onNavigateBack = {
                navigator.goBackUntil(SettingsNavKey)
            })
        },
    ) { modifier ->
        Column(
            modifier = Modifier.fillMaxSize().then(modifier),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            WarrenWalletSettingsSection(
                activity = activity,
                walletRepository = walletRepository,
            )

            // Proactive status from the last-known cached expiry — shown
            // immediately, without a fresh biometric-gated request.
            cachedSubscriptionLabel(cachedExpiry)?.let { msg ->
                Text(
                    text = msg,
                    style = MaterialTheme.typography.titleSmall,
                    color = if (cachedExpiry <= System.currentTimeMillis() / 1000) {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.primary
                    },
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }

            // Subscription status: biometric-gated signed GET /v1/subscription.
            OutlinedButton(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                onClick = {
                    scope.launch {
                        subscriptionStatus = "Checking subscription…"
                        val outcome = subscriptionInvoker.fetch(activity)
                        if (outcome is WarrenSubscriptionOutcome.Success) {
                            settings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                        }
                        subscriptionStatus = subscriptionLabel(outcome)
                    }
                },
            ) { Text("Check subscription") }

            subscriptionStatus?.let { msg ->
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }

            // Voucher redemption (Crockford-32). The server normalizes the
            // dashed / raw form, so the input is sent verbatim.
            OutlinedTextField(
                value = voucherInput,
                onValueChange = { voucherInput = it.uppercase() },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                label = { Text("Voucher code") },
                placeholder = { Text("XXXX-XXXX-XXXX-XXXX") },
                singleLine = true,
            )
            OutlinedButton(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                enabled = voucherInput.isNotBlank(),
                onClick = {
                    val code = voucherInput.trim()
                    scope.launch {
                        subscriptionStatus = "Redeeming voucher…"
                        val outcome = subscriptionInvoker.redeemVoucher(activity, code)
                        if (outcome is WarrenVoucherOutcome.Success) {
                            voucherInput = ""
                            settings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                        }
                        subscriptionStatus = voucherLabel(outcome)
                    }
                },
            ) { Text("Redeem voucher") }

            // Device management: list + remove, each biometric-gated.
            OutlinedButton(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                onClick = {
                    scope.launch {
                        deviceStatus = "Loading devices…"
                        when (val outcome = deviceInvoker.list(activity)) {
                            is WarrenDeviceListOutcome.Success -> {
                                devices = outcome.devices
                                deviceStatus = if (outcome.devices.isEmpty()) "No devices registered." else null
                            }
                            WarrenDeviceListOutcome.AuthorizationDenied -> {
                                deviceStatus = "Authorization cancelled."; devices = null
                            }
                            WarrenDeviceListOutcome.WalletNotReady -> {
                                deviceStatus = "Set up your wallet first."; devices = null
                            }
                            is WarrenDeviceListOutcome.Failure -> {
                                deviceStatus = "Couldn't load devices."; devices = null
                            }
                        }
                    }
                },
            ) { Text("Manage devices") }

            deviceStatus?.let { msg ->
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }

            devices?.forEach { device ->
                DeviceRow(
                    device = device,
                    onRemove = {
                        scope.launch {
                            deviceStatus = "Removing ${device.name}…"
                            when (deviceInvoker.remove(activity, device.id)) {
                                WarrenDeviceRemoveOutcome.Success -> {
                                    devices = devices?.filterNot { it.id == device.id }
                                    deviceStatus = "Removed ${device.name}."
                                }
                                WarrenDeviceRemoveOutcome.AuthorizationDenied ->
                                    deviceStatus = "Authorization cancelled."
                                WarrenDeviceRemoveOutcome.WalletNotReady ->
                                    deviceStatus = "Set up your wallet first."
                                is WarrenDeviceRemoveOutcome.Failure ->
                                    deviceStatus = "Couldn't remove ${device.name}."
                            }
                        }
                    },
                )
            }

            // Test button: dispatch end-to-end Quinn connect via the
            // app-side use-case. Surfaces the result as inline text;
            // a Snackbar would be nicer once the Connect button proper
            // gets wired in lib/feature/home/impl.
            OutlinedButton(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                onClick = {
                    scope.launch {
                        connectStatus = "Connecting..."
                        val message = try {
                            quinnConnect.connect(activity)
                        } catch (e: Exception) {
                            Logger.e(throwable = e) { "Warren Connect invocation failed" }
                            "Connect failed: ${e.message}"
                        }
                        connectStatus = message
                    }
                },
            ) { Text("Warren Connect (test)") }

            connectStatus?.let { msg ->
                LaunchedEffect(msg) { Logger.i("Warren Connect status: $msg") }
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }

            // Live Quinn tunnel state mirrored via WarrenQuinnStateProxy.
            // Refreshes every time the service-side adapter transitions.
            Text(
                text = "Tunnel state: $tunnelState",
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
    }
}

/**
 * Render a [WarrenSubscriptionOutcome] as a user-facing line. The raw
 * failure message is intentionally not surfaced (it is loggable only).
 */
internal fun subscriptionLabel(
    outcome: WarrenSubscriptionOutcome,
    nowSecs: Long = System.currentTimeMillis() / 1000,
): String = when (outcome) {
    is WarrenSubscriptionOutcome.Success -> {
        val date = java.time.Instant.ofEpochSecond(outcome.expiresAtUnixSecs)
            .atZone(java.time.ZoneId.systemDefault())
            .toLocalDate()
            .toString()
        if (outcome.expiresAtUnixSecs > nowSecs) {
            "Subscription active — expires $date"
        } else {
            "Subscription expired ($date)"
        }
    }
    WarrenSubscriptionOutcome.AuthorizationDenied -> "Authorization cancelled."
    WarrenSubscriptionOutcome.WalletNotReady -> "Set up your wallet first."
    is WarrenSubscriptionOutcome.Failure -> "Couldn't fetch subscription status."
}

/** Render a [WarrenVoucherOutcome] as a user-facing line. */
internal fun voucherLabel(outcome: WarrenVoucherOutcome): String = when (outcome) {
    is WarrenVoucherOutcome.Success -> {
        val date = java.time.Instant.ofEpochSecond(outcome.expiresAtUnixSecs)
            .atZone(java.time.ZoneId.systemDefault())
            .toLocalDate()
            .toString()
        "Voucher redeemed — subscription expires $date"
    }
    WarrenVoucherOutcome.AuthorizationDenied -> "Authorization cancelled."
    WarrenVoucherOutcome.WalletNotReady -> "Set up your wallet first."
    is WarrenVoucherOutcome.Failure -> "Couldn't redeem voucher. Check the code and try again."
}

/**
 * Render the cached subscription expiry as a proactive status line, or null
 * when the expiry is unknown (never fetched). Surfaces a near-expiry warning
 * within [WARN_WINDOW_SECS] of expiry so the user is nudged to renew before
 * the tunnel stops working.
 */
internal fun cachedSubscriptionLabel(
    expiryUnixSecs: Long,
    nowSecs: Long = System.currentTimeMillis() / 1000,
): String? {
    if (expiryUnixSecs <= 0L) return null
    val date = java.time.Instant.ofEpochSecond(expiryUnixSecs)
        .atZone(java.time.ZoneId.systemDefault())
        .toLocalDate()
        .toString()
    return when {
        expiryUnixSecs <= nowSecs -> "Subscription expired on $date"
        expiryUnixSecs - nowSecs <= WARN_WINDOW_SECS -> {
            val days = ((expiryUnixSecs - nowSecs) + 86_399) / 86_400 // ceil to whole days
            "Subscription expires in $days day${if (days == 1L) "" else "s"} ($date)"
        }
        else -> "Subscription active — expires $date"
    }
}

private const val WARN_WINDOW_SECS = 7L * 86_400

/** Format a device creation timestamp as a local date, or a fallback. */
internal fun deviceCreatedLabel(unixSecs: Long): String =
    if (unixSecs <= 0L) {
        "unknown date"
    } else {
        java.time.Instant.ofEpochSecond(unixSecs)
            .atZone(java.time.ZoneId.systemDefault())
            .toLocalDate()
            .toString()
    }

@Composable
private fun DeviceRow(device: WarrenDeviceSummary, onRemove: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = device.name.ifBlank { device.id.take(8) },
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                text = "Added ${deviceCreatedLabel(device.createdAtUnixSecs)}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        OutlinedButton(onClick = onRemove) { Text("Remove") }
    }
}

