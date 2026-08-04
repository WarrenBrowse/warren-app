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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import com.warrenbrowse.vpn.lib.ui.component.WarrenHelpLink
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.screen.navigation.OnboardingPreferencesNavKey
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.delay
import org.koin.compose.koinInject

/**
 * Beta-flavor onboarding access step, replacing the subscription step:
 * the app redeems the server's beta campaign voucher automatically
 * (empty voucher = auto-redeem, idempotent) and explains the degraded
 * free network. Never blocks: activation failures retry with backoff
 * while the screen is up (and again at next launch through the
 * out-of-time gate), and the user can continue regardless.
 */
@Composable
fun OnboardingBetaAccessScreen(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val networkInfoProvider = koinInject<WarrenNetworkInfoProvider>()

    val cachedExpiry by settings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    val networkInfo by networkInfoProvider.networkInfo.collectAsStateWithLifecycle()
    val hasAccess = cachedExpiry > System.currentTimeMillis() / 1000

    var activating by remember { mutableStateOf(!hasAccess) }
    var failed by remember { mutableStateOf(false) }
    var retryNonce by remember { mutableStateOf(0) }

    // Auto-activate with capped backoff while the screen is up. Keyed on
    // the manual retry nonce so "Retry now" restarts the chain.
    LaunchedEffect(retryNonce) {
        var delayMs = 5_000L
        while (cachedExpiry <= System.currentTimeMillis() / 1000) {
            activating = true
            failed = false
            val redeemed =
                runCatching { subscriptionInvoker.redeemVoucher(activity, "") }.getOrNull()
            runCatching { subscriptionInvoker.fetch(activity) }
            activating = false
            if (redeemed is WarrenVoucherOutcome.Success) {
                break
            }
            failed = true
            delay(delayMs)
            delayMs = (delayMs * 2).coerceAtMost(60_000L)
        }
    }

    val toNext = { navigator.navigate(OnboardingPreferencesNavKey) }

    val capMbps = networkInfo?.defaultRateBps?.let { (it / 1_000_000L).toInt() }?.takeIf { it > 0 }

    OnboardingStepScaffold(navigator = navigator) {
        Text(
            text = stringResource(R.string.onboarding_beta_title),
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
        )
        Text(
            text = stringResource(R.string.onboarding_beta_body),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Text(
            text = if (capMbps != null) {
                stringResource(R.string.beta_dialog_cap_capped, capMbps)
            } else {
                stringResource(R.string.beta_dialog_cap)
            } + " " + stringResource(R.string.beta_dialog_terms),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        if (hasAccess) {
            Text(
                text = stringResource(
                    R.string.onboarding_beta_active_until,
                    DateFormat.getDateInstance(DateFormat.LONG)
                        .format(Date(cachedExpiry * 1000)),
                ),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center,
            )
        }
        if (failed && !hasAccess) {
            Text(
                text = stringResource(R.string.onboarding_beta_error),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center,
            )
            // Desktop shows the help page under every onboarding error: this
            // one lands before the account has ever paid, so the forum, which
            // is gated on that, is not yet a door.
            WarrenHelpLink()
            PrimaryButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = { retryNonce += 1 },
                text = stringResource(R.string.onboarding_beta_retry),
            )
        }

        VariantButton(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            onClick = toNext,
            text = stringResource(R.string.cont),
            isLoading = activating,
        )
        PrimaryTextButton(
            onClick = { leaveWizard(settings, navigator, ConnectNavKey) },
            text = stringResource(R.string.onboarding_skip),
        )
    }
}
