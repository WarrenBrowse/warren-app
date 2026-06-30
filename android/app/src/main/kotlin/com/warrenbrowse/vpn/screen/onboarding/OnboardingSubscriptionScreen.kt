package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.screen.navigation.OnboardingDoneNavKey
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * First-run onboarding "Your subscription" step. Mirrors the desktop
 * OnboardingSubscriptionView: surfaces the purchase up front (a fresh wallet has
 * no subscription) while letting the user restore an existing one or skip.
 *
 * Reuses the same purchase plumbing as the account screen ([WarrenSubscriptionInvoker]
 * + the wpid checkout poll) so there is no new backend. When a subscription
 * becomes active (the cached expiry flips), the step shows "Continue".
 */
@Composable
fun OnboardingSubscriptionScreen(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val uriHandler = LocalUriHandler.current
    val walletRepository = koinInject<WalletRepository>()
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
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

    val toDone = { navigator.navigate(OnboardingDoneNavKey, clearBackStack = true) }

    ScaffoldWithTopBar(
        topBarColor = MaterialTheme.colorScheme.surface,
        onSettingsClicked = null,
        onAccountClicked = null,
    ) { pv ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.surface)
                .padding(pv)
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
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

            if (isSubscribed) {
                VariantButton(
                    modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    onClick = toDone,
                    text = stringResource(R.string.cont),
                )
            } else {
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
                    text = stringResource(R.string.subscription_get),
                )
                PrimaryButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        scope.launch { runCatching { subscriptionInvoker.fetch(activity) } }
                    },
                    text = stringResource(R.string.onboarding_subscription_restore),
                )
                PrimaryTextButton(
                    onClick = toDone,
                    text = stringResource(R.string.onboarding_subscription_skip),
                )
            }
        }
    }
}

private const val ONBOARDING_CHECKOUT_URL = "https://checkout.warrenbrowse.com/"

/** Random 128-bit purchase id (wpid) as 32 lowercase hex chars (doc 35). */
private fun newOnboardingPurchaseId(): String {
    val bytes = ByteArray(16)
    java.security.SecureRandom().nextBytes(bytes)
    return bytes.joinToString("") { "%02x".format(it) }
}
