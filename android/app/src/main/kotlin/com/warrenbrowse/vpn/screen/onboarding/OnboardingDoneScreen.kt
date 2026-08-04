package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import org.koin.compose.koinInject

/**
 * Terminal onboarding step ("All set"), mirroring the desktop OnboardingDoneView.
 * Its CTA is one of the wizard's two real exits: it stamps the completed flag and
 * roots the connect screen. Desktop gives this step no skip link, and neither
 * does this one.
 */
@Composable
fun OnboardingDoneScreen(navigator: Navigator) {
    val settings = koinInject<WarrenLocalSettingsRepository>()

    OnboardingStepScaffold(navigator = navigator, verticalSpacing = ONBOARDING_STEP_WIDE_SPACING) {
        Text(
            text = stringResource(R.string.onboarding_done_title),
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
        )
        Text(
            text = stringResource(R.string.onboarding_done_body),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        VariantButton(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            onClick = { leaveWizard(settings, navigator, ConnectNavKey) },
            text = stringResource(R.string.onboarding_done_cta),
        )
    }
}
