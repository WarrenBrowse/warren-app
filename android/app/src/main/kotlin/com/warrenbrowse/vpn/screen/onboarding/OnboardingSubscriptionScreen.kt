package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
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
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.settings.impl.RedeemVoucherDialog
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.ui.component.WarrenHelpLink
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.screen.navigation.OnboardingPreferencesNavKey
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * First-run onboarding "Your subscription" step. Mirrors the desktop
 * OnboardingSubscriptionView: four converging ways forward (checkout, voucher,
 * restore of an existing subscription, skip), and the step advances by itself
 * the moment the account is funded, whatever credited it.
 *
 * Reuses the same purchase plumbing as the account screen ([WarrenSubscriptionInvoker]
 * + the wpid checkout poll) so there is no new backend.
 */
@Composable
@Suppress("LongMethod")
fun OnboardingSubscriptionScreen(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val uriHandler = LocalUriHandler.current
    val walletRepository = koinInject<WalletRepository>()
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val productFlags = koinInject<WarrenProductFlags>()
    val scope = rememberCoroutineScope()

    val cachedExpiry by settings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    val walletState by walletRepository.state.collectAsStateWithLifecycle()
    val pubkey = when (val s = walletState) {
        is WalletState.Ready -> s.pubkey.value
        is WalletState.Locked -> s.pubkey.value
        WalletState.Absent -> null
    }
    val isSubscribed = cachedExpiry > System.currentTimeMillis() / 1000

    var pendingPurchaseWpid by remember { mutableStateOf<String?>(null) }
    var checking by remember { mutableStateOf(false) }
    var errorText by remember { mutableStateOf<String?>(null) }
    var showVoucherDialog by remember { mutableStateOf(false) }

    // Arm the signed redeem poll only when the user returns from the browser,
    // matching the account-screen flow (the unlock prompt lands after payment).
    LifecycleResumeEffect(Unit) {
        pendingPurchaseWpid?.let { wpid ->
            pendingPurchaseWpid = null
            subscriptionInvoker.startPurchasePoll(activity, wpid)
        }
        // Refresh the cached expiry on (re)entry so an already-subscribed wallet
        // is recognised without the user tapping "I already have a subscription".
        scope.launch { runCatching { subscriptionInvoker.fetch(activity) } }
        onPauseOrDispose {}
    }

    val toNext = { navigator.navigate(OnboardingPreferencesNavKey) }

    // Desktop parity: the step moves on by itself once credit lands, held while
    // the voucher dialog is still showing its confirmation. Saveable, because the
    // step stays on the back stack and must not throw the user forward again when
    // they walk back into it.
    var advanced by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(isSubscribed, showVoucherDialog) {
        if (
            shouldAdvanceFromFundingStep(
                alreadyAdvanced = advanced,
                funded = isSubscribed,
                held = showVoucherDialog,
            )
        ) {
            advanced = true
            toNext()
        }
    }

    val verifySubscription = {
        checking = true
        errorText = null
        scope.launch {
            val fetched = runCatching { subscriptionInvoker.fetch(activity) }
            val funded = settings.cachedSubscriptionExpiry.value > System.currentTimeMillis() / 1000
            checking = false
            errorText = when {
                fetched.isFailure ->
                    activity.getString(R.string.onboarding_subscription_check_failed)
                funded -> null
                else -> activity.getString(R.string.onboarding_subscription_none_found)
            }
        }
        Unit
    }

    OnboardingStepScaffold(navigator = navigator) {
        Text(
            text = stringResource(R.string.onboarding_subscription_title),
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
        )
        Text(
            text = stringResource(
                if (isSubscribed) {
                    R.string.onboarding_subscription_active
                } else {
                    R.string.onboarding_subscription_body
                },
            ),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        // While subscribed the step is already on its way out (the advance effect
        // above), so it holds the confirmation instead of offering a purchase the
        // user no longer needs. The Continue button is what a user who walked back
        // into the step needs, since the advance has already been spent.
        if (isSubscribed) {
            VariantButton(
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                onClick = toNext,
                text = stringResource(R.string.cont),
            )
            return@OnboardingStepScaffold
        }

        errorText?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center,
            )
            // Desktop shows the help page under every onboarding error: the
            // forum is gated on having paid, so it is not yet a door here.
            WarrenHelpLink()
        }

        VariantButton(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            onClick = {
                // App-initiated purchase (doc 35): mint a wpid, open the
                // checkout bound to it, arm the redeem poll for the return.
                val wpid = newOnboardingPurchaseId()
                pendingPurchaseWpid = wpid
                val acct = pubkey?.let {
                    java.net.URLEncoder.encode(it.shortWarrenAddress(), "UTF-8")
                }.orEmpty()
                uriHandler.safeOpenUri("$ONBOARDING_CHECKOUT_URL?pid=$wpid#acct=$acct")
            },
            text = stringResource(R.string.onboarding_subscription_view_plans),
        )
        // Same gating as the account page: beta builds expose no voucher or
        // payment surface, access is granted by the server-side auto-voucher.
        if (!productFlags.isBeta) {
            VariantButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = { showVoucherDialog = true },
                text = stringResource(R.string.subscription_redeem_voucher),
            )
        }
        PrimaryButton(
            modifier = Modifier.fillMaxWidth(),
            onClick = verifySubscription,
            text = stringResource(
                if (errorText == null) {
                    R.string.onboarding_subscription_restore
                } else {
                    R.string.onboarding_subscription_check_again
                },
            ),
            isLoading = checking,
        )
        PrimaryTextButton(
            onClick = { leaveWizard(settings, navigator, ConnectNavKey) },
            text = stringResource(R.string.onboarding_subscription_skip),
        )
    }

    if (showVoucherDialog) {
        RedeemVoucherDialog(onDismiss = { showVoucherDialog = false })
    }
}

private const val ONBOARDING_CHECKOUT_URL = "https://checkout.warrenbrowse.com/"

/** Random 128-bit purchase id (wpid) as 32 lowercase hex chars (doc 35). */
private fun newOnboardingPurchaseId(): String {
    val bytes = ByteArray(16)
    java.security.SecureRandom().nextBytes(bytes)
    return bytes.joinToString("") { "%02x".format(it) }
}
