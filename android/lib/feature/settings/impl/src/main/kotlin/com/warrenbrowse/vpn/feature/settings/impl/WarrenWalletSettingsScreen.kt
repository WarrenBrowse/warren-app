package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDecoration
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.common.compose.safeOpenUri
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeOutlinedButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * Warren account screen, mirroring the upstream Mullvad Android `AccountScreen`:
 * labelled identity rows at the top (public key, paid-until) over a [Spacer]
 * that pushes the destructive action to the bottom of the screen.
 *
 * Warren replaces the Mullvad account-number identity with the BIP39 wallet:
 *   - "Public key" row (the account identity, public, copyable).
 *   - "Paid until" row + an inline "Get subscription" action (the Warren
 *     equivalent of Mullvad's "Add time"; opens the hosted checkout).
 *   - a voucher row (Warren feature).
 *   - "View recovery phrase" (biometric-gated reveal) and "Erase wallet"
 *     (the destructive sign-out, styled like Mullvad's outlined "Log out").
 *
 * The public key is shown whether the wallet is locked or unlocked: it is not
 * a secret. Only revealing the recovery phrase requires an unlock.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenWalletSettings(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val walletRepository = koinInject<WalletRepository>()
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val cachedExpiry by settings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    val uriHandler = LocalUriHandler.current
    val scope = rememberCoroutineScope()

    val state by walletRepository.state.collectAsState()
    val snackbarHostState = remember { SnackbarHostState() }
    val copyToClipboard = createCopyToClipboardHandle(snackbarHostState, isSensitive = false)

    var voucherInput by remember { mutableStateOf("") }
    var subscriptionStatus by remember { mutableStateOf<String?>(null) }
    var viewMnemonic by remember { mutableStateOf<Mnemonic?>(null) }
    var viewError by remember { mutableStateOf<String?>(null) }
    var confirmErase by remember { mutableStateOf(false) }

    val redeemingVoucherStatus = stringResource(R.string.subscription_redeeming_voucher)
    val viewPhraseReason = stringResource(R.string.wallet_biometric_view_phrase_reason)
    val authRequiredError = stringResource(R.string.wallet_settings_auth_required_view_phrase)
    val unableToReadPrefix = stringResource(R.string.wallet_settings_unable_to_read)
    val pubkeyCopiedHint = stringResource(R.string.wallet_settings_pubkey_copied)
    val noSubscription = stringResource(R.string.subscription_none_active)

    // The public key is the account identity and is always available (locked or
    // ready); only Absent has no identity.
    val pubkey = when (val s = state) {
        is WalletState.Ready -> s.pubkey.value
        is WalletState.Locked -> s.pubkey.value
        WalletState.Absent -> null
    }
    val paidUntil = remember(cachedExpiry) { expiryDateOrNull(cachedExpiry) }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.settings_account),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = { navigator.goBack() }) },
        snackbarHostState = snackbarHostState,
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

            Column(verticalArrangement = Arrangement.spacedBy(Dimens.accountRowSpacing)) {
                // Public key (account identity): public, copyable.
                AccountRow(label = stringResource(R.string.account_public_key)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = pubkey,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(onClick = { copyToClipboard(pubkey, pubkeyCopiedHint) }) {
                            Icon(
                                imageVector = Icons.Rounded.ContentCopy,
                                contentDescription = stringResource(R.string.copy),
                            )
                        }
                    }
                }

                // Paid until + inline "Get subscription" (Warren's "Add time").
                AccountRow(label = stringResource(R.string.account_paid_until)) {
                    Row(
                        modifier = Modifier.heightIn(min = Dimens.accountRowMinHeight),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = paidUntil ?: noSubscription,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Spacer(modifier = Modifier.weight(1f))
                        PrimaryTextButton(
                            onClick = { uriHandler.safeOpenUri(CHECKOUT_URL) },
                            text = stringResource(R.string.subscription_get),
                            textDecoration = TextDecoration.Underline,
                        )
                    }
                }

                // Voucher redemption (Warren feature).
                AccountRow(label = stringResource(R.string.subscription_voucher_code_label)) {
                    OutlinedTextField(
                        value = voucherInput,
                        onValueChange = { voucherInput = it.uppercase() },
                        modifier = Modifier.fillMaxWidth(),
                        placeholder = { Text("XXXX-XXXX-XXXX-XXXX") },
                        singleLine = true,
                    )
                    subscriptionStatus?.let { msg ->
                        Text(
                            text = msg,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.padding(top = Dimens.smallPadding),
                        )
                    }
                    Row(modifier = Modifier.fillMaxWidth()) {
                        Spacer(modifier = Modifier.weight(1f))
                        PrimaryTextButton(
                            onClick = {
                                val code = voucherInput.trim()
                                scope.launch {
                                    subscriptionStatus = redeemingVoucherStatus
                                    val outcome = subscriptionInvoker.redeemVoucher(activity, code)
                                    if (outcome is WarrenVoucherOutcome.Success) {
                                        voucherInput = ""
                                        settings.setCachedSubscriptionExpiry(outcome.expiresAtUnixSecs)
                                    }
                                    subscriptionStatus = voucherLabel(activity, outcome)
                                }
                            },
                            text = stringResource(R.string.subscription_redeem_voucher),
                            textDecoration = TextDecoration.Underline,
                            isEnabled = voucherInput.isNotBlank(),
                        )
                    }
                }
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

            PrimaryButton(
                modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
                onClick = {
                    scope.launch {
                        try {
                            val authorizer = BiometricPromptAuthorizer(activity)
                            viewMnemonic = walletRepository.unlock(
                                authorizer = authorizer,
                                reason = viewPhraseReason,
                            )
                            viewError = null
                        } catch (e: WalletAuthorizationDeniedException) {
                            viewError = authRequiredError
                        } catch (e: Exception) {
                            Logger.w(throwable = e) { "wallet unlock failed" }
                            viewError = "$unableToReadPrefix ${e.message}"
                        }
                    }
                },
                text = stringResource(R.string.wallet_settings_view_phrase),
            )

            NegativeOutlinedButton(
                modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
                onClick = { confirmErase = true },
                text = stringResource(R.string.wallet_settings_erase),
            )
        }
    }

    viewMnemonic?.let { mnemonic ->
        AlertDialog(
            onDismissRequest = { viewMnemonic = null },
            title = { Text(stringResource(R.string.wallet_settings_recovery_phrase_title)) },
            text = { MnemonicDisplay(phrase = mnemonic.phrase, alwaysRevealed = true) },
            confirmButton = {
                TextButton(onClick = { viewMnemonic = null }) {
                    Text(stringResource(R.string.wallet_settings_done))
                }
            },
        )
    }

    if (confirmErase) {
        AlertDialog(
            onDismissRequest = { confirmErase = false },
            title = { Text(stringResource(R.string.wallet_settings_erase_confirm_title)) },
            text = { Text(stringResource(R.string.wallet_settings_erase_confirm_description)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmErase = false
                    scope.launch { walletRepository.erase() }
                }) { Text(stringResource(R.string.wallet_settings_erase_confirm_action)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmErase = false }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    }
}

/** A labelled account row: a small caption label above its value content. */
@Composable
private fun AccountRow(label: String, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        content()
    }
}

/** Format a unix-seconds expiry as a local date, or null when unset. */
private fun expiryDateOrNull(expiryUnixSecs: Long): String? {
    if (expiryUnixSecs <= 0L) return null
    return java.time.Instant.ofEpochSecond(expiryUnixSecs)
        .atZone(java.time.ZoneId.systemDefault())
        .toLocalDate()
        .toString()
}

/**
 * Render a [WarrenSubscriptionOutcome] as a user-facing line. The raw
 * failure message is intentionally not surfaced (it is loggable only).
 */
internal fun subscriptionLabel(
    context: android.content.Context,
    outcome: WarrenSubscriptionOutcome,
    nowSecs: Long = System.currentTimeMillis() / 1000,
): String = when (outcome) {
    is WarrenSubscriptionOutcome.Success -> {
        if (outcome.expiresAtUnixSecs <= 0L) {
            // Epoch expiry is the "no subscription bound yet" sentinel a 404
            // resolves to (mirrors iOS / desktop); show it as such instead of
            // a bogus "expired (1970-01-01)" date.
            context.getString(R.string.subscription_none_active)
        } else {
            val date = java.time.Instant.ofEpochSecond(outcome.expiresAtUnixSecs)
                .atZone(java.time.ZoneId.systemDefault())
                .toLocalDate()
                .toString()
            if (outcome.expiresAtUnixSecs > nowSecs) {
                context.getString(R.string.subscription_active_expires, date)
            } else {
                context.getString(R.string.subscription_expired, date)
            }
        }
    }
    WarrenSubscriptionOutcome.AuthorizationDenied ->
        context.getString(R.string.subscription_authorization_cancelled)
    WarrenSubscriptionOutcome.WalletNotReady ->
        context.getString(R.string.subscription_wallet_not_ready)
    is WarrenSubscriptionOutcome.Failure ->
        context.getString(R.string.subscription_fetch_failed)
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

/**
 * Render the cached subscription expiry as a proactive status line, or null
 * when the expiry is unknown (never fetched). Surfaces a near-expiry warning
 * within [WARN_WINDOW_SECS] of expiry so the user is nudged to renew before
 * the tunnel stops working.
 */
internal fun cachedSubscriptionLabel(
    context: android.content.Context,
    expiryUnixSecs: Long,
    nowSecs: Long = System.currentTimeMillis() / 1000,
): String? {
    if (expiryUnixSecs <= 0L) return null
    val date = java.time.Instant.ofEpochSecond(expiryUnixSecs)
        .atZone(java.time.ZoneId.systemDefault())
        .toLocalDate()
        .toString()
    return when {
        expiryUnixSecs <= nowSecs ->
            context.getString(R.string.subscription_expired_on, date)
        expiryUnixSecs - nowSecs <= WARN_WINDOW_SECS -> {
            val days = ((expiryUnixSecs - nowSecs) + 86_399) / 86_400 // ceil to whole days
            if (days == 1L) {
                context.getString(R.string.subscription_expires_in_day, days, date)
            } else {
                context.getString(R.string.subscription_expires_in_days, days, date)
            }
        }
        else -> context.getString(R.string.subscription_active_expires, date)
    }
}

private const val WARN_WINDOW_SECS = 7L * 86_400

// Hosted Stripe checkout funnel (matches desktop `urls.purchase`).
private const val CHECKOUT_URL = "https://checkout.warrenbrowse.com/"
