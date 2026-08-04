package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.screen.navigation.OnboardingDoneNavKey
import org.koin.compose.koinInject

/**
 * Onboarding "Privacy preferences" step (desktop `OnboardingPreferencesView`),
 * inserted between the funding step and Done so a first-run user meets multi-hop
 * and DAITA here rather than only by going hunting in Settings.
 *
 * Where desktop still shows scaffold text, Android embeds the real switches: the
 * two settings pages already exist and write the same repository the tunnel
 * config builder reads, so a choice made here takes effect on the first connect.
 * Obfuscation has no switch because it cannot be turned off, so it is stated
 * rather than offered.
 */
@Composable
fun OnboardingPreferencesScreen(navigator: Navigator) {
    val settings = koinInject<WarrenLocalSettingsRepository>()

    val multiHopEnabled by settings.multiHopEnabled.collectAsStateWithLifecycle()
    val daitaEnabled by settings.daitaEnabled.collectAsStateWithLifecycle()

    OnboardingStepScaffold(navigator = navigator) {
        Text(
            text = stringResource(R.string.onboarding_preferences_title),
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
        )
        Text(
            text = stringResource(R.string.onboarding_preferences_body),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        PreferenceToggle(
            title = stringResource(R.string.multihop),
            explainer = stringResource(R.string.onboarding_preferences_multihop_desc),
            value = multiHopEnabled,
            onValueChange = settings::setMultiHopEnabled,
        )
        PreferenceToggle(
            title = stringResource(R.string.daita),
            explainer = stringResource(R.string.onboarding_preferences_daita_desc),
            value = daitaEnabled,
            onValueChange = settings::setDaitaEnabled,
        )

        Column(
            modifier = Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                text = stringResource(R.string.onboarding_prop_obfuscation_title),
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = stringResource(R.string.onboarding_preferences_obfuscation_desc),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        VariantButton(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            onClick = { navigator.navigate(OnboardingDoneNavKey) },
            text = stringResource(R.string.cont),
        )
    }
}

@Composable
private fun PreferenceToggle(
    title: String,
    explainer: String,
    value: Boolean,
    onValueChange: (Boolean) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        WarrenListItem(
            position = Position.Single,
            onClick = { onValueChange(!value) },
            content = {
                Text(
                    modifier = Modifier.align(Alignment.CenterStart),
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            },
            trailingContent = {
                WarrenSwitch(
                    modifier = Modifier.align(Alignment.Center),
                    checked = value,
                    onCheckedChange = onValueChange,
                )
            },
        )
        Text(
            text = explainer,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
