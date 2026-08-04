package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton

/**
 * Shared chrome for every step of the first-run wizard, mirroring the desktop
 * `OnboardingLayout`: the brand lockup stays in the top bar and a back chevron
 * appears on every step that has one behind it.
 *
 * The chevron is bound to the stack rather than to a per-screen flag so it can
 * never lie: the welcome step and the funding step are the roots of their stack
 * (the wallet backup gate clears it on purpose, so the cleartext phrase is not
 * kept alive behind the rest of the wizard), and a chevron there would be an
 * inert control.
 */
@Composable
internal fun OnboardingStepScaffold(
    navigator: Navigator,
    verticalSpacing: Dp = ONBOARDING_STEP_SPACING,
    content: @Composable ColumnScope.() -> Unit,
) {
    ScaffoldWithTopBar(
        topBarColor = MaterialTheme.colorScheme.surface,
        onSettingsClicked = null,
        onAccountClicked = null,
        navigationIcon = {
            if (navigator.backStack.size > 1) {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
    ) { pv ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.surface)
                .padding(pv)
                .verticalScroll(rememberScrollState())
                .padding(ONBOARDING_STEP_PADDING),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(verticalSpacing),
            content = content,
        )
    }
}

internal val ONBOARDING_STEP_SPACING = 16.dp
internal val ONBOARDING_STEP_WIDE_SPACING = 20.dp
private val ONBOARDING_STEP_PADDING = 24.dp
