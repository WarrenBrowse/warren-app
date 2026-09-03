package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import org.koin.compose.koinInject

/**
 * First-launch onboarding welcome. Introduces Warren's value props, then hands off to wallet
 * creation. The Warren mark sits in the top bar (like the desktop AppMainHeader). Gated to be shown
 * once (see [com.warrenbrowse.vpn.screen.splash.SplashViewModel]).
 *
 * "Get started" only advances; the wizard is marked completed at its real exits (see
 * [leaveWizard]). Skipping still routes through wallet creation because the wallet IS the identity
 * on Android: what is skipped is the guided funding and preferences steps, so the post-wallet
 * destination is Connect.
 */
@Composable
fun OnboardingScreen(navigator: Navigator) {
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val walletState by koinInject<WalletRepository>().state.collectAsStateWithLifecycle()
    val productFlags = koinInject<WarrenProductFlags>()

    OnboardingStepScaffold(navigator = navigator, verticalSpacing = ONBOARDING_STEP_WIDE_SPACING) {
        Text(
            text = stringResource(R.string.onboarding_welcome_title),
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = stringResource(R.string.onboarding_welcome_body),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        ValueProp(
            title = stringResource(R.string.onboarding_prop_obfuscation_title),
            body = stringResource(R.string.onboarding_prop_obfuscation_body),
        )
        ValueProp(
            title = stringResource(R.string.onboarding_prop_no_logs_title),
            body = stringResource(R.string.onboarding_prop_no_logs_body),
        )
        ValueProp(
            title = stringResource(R.string.onboarding_prop_keys_title),
            body = stringResource(R.string.onboarding_prop_keys_body),
        )

        VariantButton(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            onClick = {
                enterWalletStep(
                    navigator,
                    walletPresent = walletState !is WalletState.Absent,
                    isBeta = productFlags.isBeta,
                )
            },
            text = stringResource(R.string.onboarding_get_started),
        )
        PrimaryTextButton(
            onClick = { leaveWizard(settings, navigator, WarrenWalletNavKey(onboarding = false)) },
            text = stringResource(R.string.onboarding_skip),
        )
    }
}

@Composable
private fun ValueProp(title: String, body: String) {
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            text = "•",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.primary,
        )
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = body,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
