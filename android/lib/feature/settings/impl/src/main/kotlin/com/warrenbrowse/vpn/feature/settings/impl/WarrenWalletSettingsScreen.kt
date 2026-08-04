package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.common.compose.safeOpenUri
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.WarrenKeysNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.MnemonicCache
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.component.BetaBadge
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * Warren account screen, mirroring the desktop `AccountView` element list and
 * order: a subscription card (big remaining time while active, then the
 * paid-until row), the public-key card with an inline copy confirmation, then
 * the action stack (buy credit, redeem voucher, backup row) with the
 * destructive erase as a ghost red link at the very bottom.
 *
 * Warren has no email/password account: the BIP39 wallet is the identity, and
 * the action that removes it from the device carries the desktop wording ("Log
 * out") so the button, the dialog title and its body say the same thing.
 *
 * The public key is shown whether the wallet is locked or unlocked: it is not
 * a secret. Only revealing the recovery phrase requires an unlock, after which
 * the phrase is shown by the keys screen, a real destination rather than a
 * dialog, so the back gesture behaves and it can lead to the restore step.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenWalletSettings(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val walletRepository = koinInject<WalletRepository>()
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val cachedExpiry by settings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    // Beta program surfaces: payments are masked and the badge explains
    // the free degraded network. Gated on the COMPILED flavor.
    val productFlags = koinInject<WarrenProductFlags>()
    val networkInfoProvider = koinInject<WarrenNetworkInfoProvider>()
    val networkInfo by networkInfoProvider.networkInfo.collectAsStateWithLifecycle()
    // The badge holds its note until the cap is known (live feed or the cached
    // last answer), so a cold start never shows one wording and swaps it.
    val cachedRateBps by settings.cachedNetworkRateBps.collectAsStateWithLifecycle()
    val uriHandler = LocalUriHandler.current
    val scope = rememberCoroutineScope()

    val state by walletRepository.state.collectAsState()
    val copyToClipboard = createCopyToClipboardHandle(isSensitive = false)

    var showVoucherDialog by remember { mutableStateOf(false) }
    var viewError by remember { mutableStateOf<String?>(null) }
    var confirmErase by remember { mutableStateOf(false) }
    // Erasing the wallet is irreversible: gate the destructive action behind an
    // explicit "I backed up my phrase" acknowledgement, matching the desktop
    // logout checkbox gate (AccountView).
    var eraseBackupAck by remember { mutableStateOf(false) }
    // wpid awaiting its redeem poll: set when the user opens the checkout, then
    // consumed on the next return to the app (see LifecycleResumeEffect below).
    var pendingPurchaseWpid by remember { mutableStateOf<String?>(null) }

    // Defer the signed redeem poll until the user comes back from the browser
    // after paying: resuming with a pending wpid keeps the flow aligned with
    // the desktop buyCredit poll. The same resume hook refreshes the cached
    // expiry silently (desktop updateAccountData-on-mount parity) so the
    // paid-until row is live data, not a stale cache.
    LifecycleResumeEffect(Unit) {
        pendingPurchaseWpid?.let { wpid ->
            pendingPurchaseWpid = null
            subscriptionInvoker.startPurchasePoll(activity, wpid)
        }
        if (walletRepository.state.value !is WalletState.Absent) {
            scope.launch { runCatching { subscriptionInvoker.fetch(activity) } }
        }
        onPauseOrDispose {}
    }

    val viewPhraseReason = stringResource(R.string.wallet_biometric_view_phrase_reason)
    val authRequiredError = stringResource(R.string.wallet_settings_auth_required_view_phrase)
    val unableToReadError = stringResource(R.string.wallet_settings_unable_to_read)
    val eraseFailedError = stringResource(R.string.wallet_settings_erase_failed)

    // The reveal is reused by the backup row AND the erase dialog's "Back up my
    // phrase" escape hatch, so it lives outside the button callbacks. The
    // unlocked phrase is staged out of band and read by the keys screen: it
    // must never ride in a NavKey, which Compose Navigation persists.
    val revealPhrase: () -> Unit = {
        scope.launch {
            try {
                val authorizer = BiometricPromptAuthorizer(activity)
                MnemonicCache.put(
                    walletRepository.unlock(authorizer = authorizer, reason = viewPhraseReason),
                )
                viewError = null
                navigator.navigate(WarrenKeysNavKey)
            } catch (e: WalletAuthorizationDeniedException) {
                viewError = authRequiredError
            } catch (e: Exception) {
                Logger.w(throwable = e) { "wallet unlock failed" }
                // The engine message can carry paths or key handles; it stays
                // in the log, never on screen.
                viewError = unableToReadError
            }
        }
    }

    // The public key is the account identity and is always available (locked or
    // ready); only Absent has no identity.
    val pubkey = when (val s = state) {
        is WalletState.Ready -> s.pubkey.value
        is WalletState.Locked -> s.pubkey.value
        WalletState.Absent -> null
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.settings_account),
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
                .padding(horizontal = Dimens.sideMargin)
                .padding(top = Dimens.screenTopMargin, bottom = Dimens.screenBottomMargin),
        ) {
            if (pubkey == null) {
                Text(
                    text = stringResource(R.string.wallet_settings_absent_hint),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                return@Column
            }

            Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
                SubscriptionCard(expiryUnixSecs = cachedExpiry)
                if (productFlags.isBeta) {
                    BetaBadge(
                        capBps =
                            if (networkInfo != null) networkInfo?.defaultRateBps else cachedRateBps,
                        capResolved = networkInfo != null || cachedRateBps != null,
                    )
                }
                PubkeyCard(pubkey = pubkey, onCopy = { copyToClipboard(pubkey, null) })
            }

            viewError?.let { msg ->
                Text(
                    text = msg,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = Dimens.smallPadding),
                )
            }

            Spacer(modifier = Modifier.weight(1f).heightIn(min = Dimens.mediumPadding))

            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                // Beta builds never expose payment or voucher flows: access
                // is granted by the server-side auto-voucher instead.
                if (!productFlags.isBeta) {
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = {
                            // App-initiated purchase (doc 35): mint a random wpid and
                            // open the checkout bound to it (short pubkey rides in the
                            // fragment for the landing page only, never sent to the
                            // server). The signed redeem poll is armed here but only
                            // fires when the user returns to the app (see the
                            // LifecycleResumeEffect), so the unlock prompt appears
                            // after payment, not before.
                            val wpid = newPurchaseId()
                            val acct = java.net.URLEncoder.encode(
                                pubkey.shortWarrenAddress(), "UTF-8",
                            )
                            pendingPurchaseWpid = wpid
                            uriHandler.safeOpenUri("$CHECKOUT_URL?pid=$wpid#acct=$acct")
                        },
                        text = stringResource(R.string.account_buy_more_credit),
                        icon = {
                            Icon(
                                imageVector = Icons.AutoMirrored.Rounded.OpenInNew,
                                contentDescription = null,
                            )
                        },
                    )

                    PrimaryButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { showVoucherDialog = true },
                        text = stringResource(R.string.subscription_redeem_voucher),
                    )
                }

                BackupPhraseRow(onClick = revealPhrase)

                // Rare and destructive: a ghost red link kept visually apart
                // from the everyday actions above (desktop LogoutButton).
                TextButton(
                    modifier = Modifier.align(Alignment.CenterHorizontally),
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                    onClick = { confirmErase = true },
                ) {
                    Text(
                        text = stringResource(R.string.wallet_settings_erase),
                        style = MaterialTheme.typography.titleSmall,
                    )
                }
            }
        }
    }

    if (showVoucherDialog) {
        RedeemVoucherDialog(onDismiss = { showVoucherDialog = false })
    }

    if (confirmErase) {
        val dismiss = {
            confirmErase = false
            eraseBackupAck = false
        }
        // Destructive and irreversible: the red icon and the red confirm say so
        // before the tap, and the acknowledgement gates it (desktop logout
        // modal). The escape hatch sits with the actions, not in the message.
        NegativeConfirmationDialog(
            message = stringResource(R.string.wallet_settings_erase_confirm_title),
            messageStyle = MaterialTheme.typography.titleMedium,
            confirmationText = stringResource(R.string.wallet_settings_erase_confirm_action),
            isConfirmEnabled = eraseBackupAck,
            body = {
                Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding)) {
                    Text(stringResource(R.string.account_erase_body_device))
                    Text(stringResource(R.string.account_erase_body_backup))
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { eraseBackupAck = !eraseBackupAck },
                        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(
                            checked = eraseBackupAck,
                            onCheckedChange = { eraseBackupAck = it },
                        )
                        Text(stringResource(R.string.wallet_settings_erase_backup_ack))
                    }
                }
            },
            secondaryAction = {
                VariantButton(
                    onClick = {
                        dismiss()
                        revealPhrase()
                    },
                    text = stringResource(R.string.account_backup_my_phrase),
                )
            },
            onConfirm = {
                dismiss()
                scope.launch {
                    val erased = runCatching { walletRepository.erase() }
                    if (erased.isFailure) {
                        Logger.w(throwable = erased.exceptionOrNull()) { "wallet erase failed" }
                        // A dead coroutine would leave the user on an account
                        // screen for a wallet that is still there, with no sign
                        // anything went wrong.
                        viewError = eraseFailedError
                        return@launch
                    }
                    // Sign-out: route back to the wallet login screen,
                    // clearing the stack (mirrors Mullvad logout ->
                    // NavigateToLogin). Without this the user is stranded
                    // on an empty account screen after erasing.
                    navigator.navigate(WarrenWalletNavKey(), clearBackStack = true)
                }
            },
            onBack = dismiss,
        )
    }
}

/**
 * Desktop SubscriptionCard: the remaining time as a big positive headline only
 * while the subscription is active, over the "Paid until" labelled row whose
 * value is the red OUT OF TIME when expired and "Currently unavailable" when
 * the expiry is unknown.
 */
@Composable
private fun SubscriptionCard(expiryUnixSecs: Long) {
    val nowSecs = System.currentTimeMillis() / 1000
    AccountCard {
        when (val remaining = remainingTime(expiryUnixSecs, nowSecs)) {
            RemainingTime.None -> Unit
            else -> Text(
                text = remainingTimeText(remaining),
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.positive,
                modifier = Modifier.padding(bottom = Dimens.smallPadding),
            )
        }
        Text(
            text = stringResource(R.string.account_paid_until),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        when (val display = paidUntilDisplay(expiryUnixSecs, nowSecs)) {
            PaidUntilDisplay.OutOfTime -> Text(
                text = stringResource(R.string.account_out_of_time),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
            )
            is PaidUntilDisplay.Date -> Text(
                text = formatExpiryDateTime(display.expiryUnixSecs),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            PaidUntilDisplay.Unavailable -> Text(
                text = stringResource(R.string.account_expiry_unavailable),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

/** Shared with the voucher success line, which names the duration it credited. */
@Composable
internal fun remainingTimeText(remaining: RemainingTime): String = when (remaining) {
    RemainingTime.None -> ""
    RemainingTime.LessThanADay -> stringResource(R.string.account_remaining_less_than_day)
    is RemainingTime.Days -> pluralStringResource(
        R.plurals.account_remaining_days,
        remaining.days.toInt(),
        remaining.days.toInt(),
    )
    is RemainingTime.Months -> pluralStringResource(
        R.plurals.account_remaining_months,
        remaining.months.toInt(),
        remaining.months.toInt(),
    )
    is RemainingTime.Years -> pluralStringResource(
        R.plurals.account_remaining_years,
        remaining.years.toInt(),
        remaining.years.toInt(),
    )
}

/**
 * Desktop pubkey card: the full-form label, the shortened address, and a copy
 * button that swaps to a green checkmark for two seconds instead of raising a
 * snackbar. The copy still yields the full address.
 */
@Composable
private fun PubkeyCard(pubkey: String, onCopy: () -> Unit) {
    var justCopied by remember { mutableStateOf(false) }
    LaunchedEffect(justCopied) {
        if (justCopied) {
            delay(COPIED_ICON_DURATION_MS)
            justCopied = false
        }
    }

    AccountCard {
        Text(
            text = stringResource(R.string.account_pubkey_card_label),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = pubkey.shortWarrenAddress(),
                style = MaterialTheme.typography.bodyMedium,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f),
            )
            if (justCopied) {
                Icon(
                    imageVector = Icons.Rounded.Check,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.positive,
                    modifier = Modifier.padding(Dimens.smallPadding),
                )
            } else {
                IconButton(
                    onClick = {
                        onCopy()
                        justCopied = true
                    },
                ) {
                    Icon(
                        imageVector = Icons.Rounded.ContentCopy,
                        contentDescription = stringResource(R.string.copy),
                    )
                }
            }
        }
    }
}

/** Desktop account Card: a lifted rounded surface for the account facts. */
@Composable
private fun AccountCard(content: @Composable () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                color = MaterialTheme.colorScheme.surfaceContainer,
                shape = RoundedCornerShape(Dimens.mediumPadding),
            )
            .padding(Dimens.mediumPadding),
    ) {
        content()
    }
}

/**
 * Desktop BackupRow: the phrase backup is navigation, not a coloured command,
 * so it renders as an outlined row with a trailing chevron, distinct from the
 * solid buy/voucher buttons above it. The biometric gate on the reveal is the
 * intentional Android posture and is kept.
 */
@Composable
private fun BackupPhraseRow(onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .border(
                border = BorderStroke(
                    width = Dimens.outLineButtonBorderWidth,
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20),
                ),
                shape = RoundedCornerShape(12.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = Dimens.mediumPadding, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.wallet_settings_view_phrase),
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        Icon(
            imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** Render a [WarrenVoucherOutcome] as a user-facing line. */
internal fun voucherLabel(
    context: android.content.Context,
    outcome: WarrenVoucherOutcome,
): String = when (outcome) {
    is WarrenVoucherOutcome.Success -> {
        val date = java.time.Instant.ofEpochSecond(outcome.expiresAtUnixSecs)
            .atZone(java.time.ZoneId.systemDefault())
            .toLocalDate()
            .toString()
        context.getString(R.string.subscription_voucher_redeemed, date)
    }
    WarrenVoucherOutcome.AuthorizationDenied ->
        context.getString(R.string.subscription_authorization_cancelled)
    WarrenVoucherOutcome.WalletNotReady ->
        context.getString(R.string.subscription_wallet_not_ready)
    is WarrenVoucherOutcome.Failure ->
        context.getString(R.string.subscription_voucher_redeem_failed)
}

private const val COPIED_ICON_DURATION_MS = 2_000L

// Hosted Stripe checkout funnel (matches desktop `urls.purchase`).
private const val CHECKOUT_URL = "https://checkout.warrenbrowse.com/"

/** Random 128-bit purchase id (wpid) as 32 lowercase hex chars (doc 35). */
private fun newPurchaseId(): String {
    val bytes = ByteArray(16)
    java.security.SecureRandom().nextBytes(bytes)
    return bytes.joinToString("") { "%02x".format(it) }
}
