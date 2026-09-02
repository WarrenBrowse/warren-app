package com.warrenbrowse.vpn.screen.outoftime

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.common.compose.safeOpenUri
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.impl.RedeemVoucherDialog
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * Full-screen out-of-time gate, mirroring the desktop
 * `ExpiredAccountErrorView`: the negative badge, the "Out of time" message
 * with a recovery hint, then Disconnect (only while the kill switch blocks),
 * Buy more credit, a manual "I've completed payment" re-check, and Redeem
 * voucher. Shown by [com.warrenbrowse.vpn.app.WarrenApp] when
 * [outOfTimeGateActive] raises; it dismisses itself reactively because every
 * refresh path writes the shared cached expiry.
 *
 * The gate is a navigation destination rather than a branch that swaps the whole
 * UI out, so it keeps the main header with both actions: a user whose credit
 * lapsed can still reach the account page (to restore the wallet that actually
 * holds the subscription) and Settings, exactly as on desktop.
 */
@Composable
fun OutOfTimeScreen(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val uriHandler = LocalUriHandler.current
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val disconnectInvoker = koinInject<WarrenQuinnDisconnectInvoker>()
    val walletRepository = koinInject<WalletRepository>()
    // Beta builds never expose payment flows: the recovery is the free,
    // idempotent auto-voucher re-register (empty voucher = auto-redeem).
    val productFlags = koinInject<WarrenProductFlags>()
    val scope = rememberCoroutineScope()

    val connectedInfo by tunnelStateProvider.connectedInfo.collectAsStateWithLifecycle()
    // While the kill switch blocks, no traffic leaves the device: the checkout
    // browser and the subscription check would both dead-end, so the screen
    // offers Disconnect instead (desktop paymentRecoveryAction parity).
    val isBlocked = connectedInfo is WarrenConnectedInfo.Blocking

    var checking by remember { mutableStateOf(false) }
    var refreshFailed by remember { mutableStateOf(false) }
    var showVoucherDialog by remember { mutableStateOf(false) }
    var showDisconnectAndBuy by remember { mutableStateOf(false) }
    var pendingPurchaseWpid by remember { mutableStateOf<String?>(null) }

    val pubkey = when (val s = walletRepository.state.value) {
        is WalletState.Ready -> s.pubkey.value
        is WalletState.Locked -> s.pubkey.value
        WalletState.Absent -> null
    }

    val openCheckout = {
        // App-initiated purchase (doc 35), same plumbing as the account screen:
        // mint a wpid, open the checkout bound to it, and arm the signed redeem
        // poll for the return to the app.
        val wpid = newPurchaseId()
        val acct = pubkey?.let {
            java.net.URLEncoder.encode(it.shortWarrenAddress(), "UTF-8")
        }.orEmpty()
        pendingPurchaseWpid = wpid
        uriHandler.safeOpenUri("$CHECKOUT_URL?pid=$wpid#acct=$acct")
    }

    LifecycleResumeEffect(Unit) {
        pendingPurchaseWpid?.let { wpid ->
            pendingPurchaseWpid = null
            subscriptionInvoker.startPurchasePoll(activity, wpid)
        }
        onPauseOrDispose {}
    }

    // Periodic silent re-check (iOS OutOfTimeInteractor parity): credit bought
    // out-of-app dismisses the gate without a manual tap. The fetch writes the
    // shared cached expiry, which is what the gate observes. Harmless while
    // blocked (it just fails) and on the first loop it doubles as the
    // entry-time refresh guarding against a stale cache.
    LaunchedEffect(Unit) {
        while (true) {
            runCatching { subscriptionInvoker.fetch(activity) }
            delay(REFRESH_INTERVAL_MS)
        }
    }

    ScaffoldWithTopBar(
        topBarColor = MaterialTheme.colorScheme.surface,
        iconTintColor = MaterialTheme.colorScheme.onSurface,
        onSettingsClicked = { navigator.navigate(SettingsNavKey) },
        onAccountClicked = { navigator.navigate(WarrenWalletSettingsNavKey) },
    ) { pv ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.surface)
                .padding(pv)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = Dimens.sideMargin, vertical = Dimens.screenTopMargin),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(modifier = Modifier.height(Dimens.largePadding))
            Icon(
                imageVector = Icons.Rounded.ErrorOutline,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.error,
                modifier = Modifier.size(OUT_OF_TIME_BADGE_SIZE),
            )
            Text(
                text = stringResource(R.string.out_of_time_title),
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = Dimens.mediumPadding),
            )
            Text(
                text = stringResource(R.string.account_credit_has_expired) + " " +
                    stringResource(
                        if (isBlocked) {
                            R.string.out_of_time_recovery_disconnect
                        } else {
                            R.string.out_of_time_recovery_browser
                        },
                    ),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = Dimens.mediumPadding),
            )

            Spacer(modifier = Modifier.weight(1f).height(Dimens.largePadding))

            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
            ) {
                if (isBlocked) {
                    NegativeButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { disconnectInvoker.disconnect() },
                        text = stringResource(R.string.disconnect),
                    )
                }
                if (productFlags.isBeta) {
                    if (refreshFailed) {
                        Text(
                            text = stringResource(R.string.out_of_time_beta_refresh_failed),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.error,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.fillMaxWidth(),
                        )
                    } else {
                        Text(
                            text = stringResource(R.string.out_of_time_beta_message),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                    PrimaryButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = {
                            checking = true
                            refreshFailed = false
                            scope.launch {
                                // Empty voucher = redeem the server's beta
                                // auto-voucher, idempotent for an already
                                // active wallet; then refresh the cached
                                // expiry the gate observes.
                                val redeemed =
                                    runCatching {
                                        subscriptionInvoker.redeemVoucher(activity, "")
                                    }.getOrNull()
                                runCatching { subscriptionInvoker.fetch(activity) }
                                refreshFailed = redeemed !is WarrenVoucherOutcome.Success
                                checking = false
                            }
                        },
                        text = stringResource(R.string.out_of_time_beta_refresh),
                        isLoading = checking,
                    )
                } else {
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = {
                            // Desktop parity: while blocked the click routes through
                            // the unblocking step instead of a dead browser tab.
                            if (isBlocked) showDisconnectAndBuy = true else openCheckout()
                        },
                        text = stringResource(R.string.account_buy_more_credit),
                        icon = {
                            Icon(
                                imageVector = Icons.AutoMirrored.Rounded.OpenInNew,
                                contentDescription = null,
                            )
                        },
                    )
                    if (!isBlocked) {
                        PrimaryButton(
                            modifier = Modifier.fillMaxWidth(),
                            onClick = {
                                checking = true
                                scope.launch {
                                    runCatching { subscriptionInvoker.fetch(activity) }
                                    checking = false
                                }
                            },
                            text = stringResource(R.string.out_of_time_completed_payment),
                            isLoading = checking,
                        )
                    }
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { showVoucherDialog = true },
                        text = stringResource(R.string.subscription_redeem_voucher),
                    )
                }
            }
        }
    }

    if (showVoucherDialog) {
        RedeemVoucherDialog(onDismiss = { showVoucherDialog = false })
    }

    if (showDisconnectAndBuy) {
        AlertDialog(
            onDismissRequest = { showDisconnectAndBuy = false },
            // The consequence, not a second copy of the sentence already on the
            // page: protection drops and a browser opens, in that order.
            title = { Text(stringResource(R.string.out_of_time_disconnect_dialog_title)) },
            text = { Text(stringResource(R.string.out_of_time_disconnect_dialog_message)) },
            confirmButton = {
                WarrenTextButton(
                    onClick = {
                        showDisconnectAndBuy = false
                        scope.launch {
                            disconnectInvoker.disconnect()
                            // Give the blocking interface a moment to come down
                            // so the checkout tab does not load into a still
                            // blocked network (desktop waits the same way).
                            delay(DISCONNECT_SETTLE_MS)
                            openCheckout()
                        }
                    },
                ) { Text(stringResource(R.string.out_of_time_disconnect_dialog_confirm)) }
            },
            dismissButton = {
                WarrenTextButton(onClick = { showDisconnectAndBuy = false }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    }
}

private val OUT_OF_TIME_BADGE_SIZE = 56.dp

private const val REFRESH_INTERVAL_MS = 60_000L
private const val DISCONNECT_SETTLE_MS = 1_000L

// Hosted Stripe checkout funnel (matches desktop `urls.purchase`).
private const val CHECKOUT_URL = "https://checkout.warrenbrowse.com/"

/** Random 128-bit purchase id (wpid) as 32 lowercase hex chars (doc 35). */
private fun newPurchaseId(): String {
    val bytes = ByteArray(16)
    java.security.SecureRandom().nextBytes(bytes)
    return bytes.joinToString("") { "%02x".format(it) }
}
