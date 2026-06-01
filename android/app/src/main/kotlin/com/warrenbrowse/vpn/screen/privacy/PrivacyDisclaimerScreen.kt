package com.warrenbrowse.vpn.screen.privacy

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import com.warrenbrowse.vpn.R
import com.warrenbrowse.vpn.app.MainActivity
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.common.util.appendHideNavOnPlayBuild
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorMedium
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.screen.navigation.SplashNavKey
import com.warrenbrowse.vpn.screen.splash.DAEMON_READY_TIMEOUT_MS
import org.koin.androidx.compose.koinViewModel

@Preview
@Composable
private fun PreviewPrivacyDisclaimerScreen() {
    AppTheme {
        PrivacyDisclaimerScreen(
            state = PrivacyDisclaimerViewState(isStartingService = false, isPlayBuild = false),
            onAcceptClicked = {},
        )
    }
}

@Composable
fun PrivacyDisclaimer(navigator: Navigator) {
    val viewModel: PrivacyDisclaimerViewModel = koinViewModel()
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    val context = LocalContext.current
    CollectSideEffectWithLifecycle(viewModel.uiSideEffect) {
        when (it) {
            PrivacyDisclaimerUiSideEffect.NavigateToLogin ->
                // Post-privacy routes straight to the wallet onboarding. The
                // wallet flow self-routes to ConnectNavKey on completion via
                // the `WarrenWalletEvent.WalletReady` channel.
                navigator.navigate(WarrenWalletNavKey, clearBackStack = true)
            PrivacyDisclaimerUiSideEffect.StartService ->
                launch {
                    try {
                        withTimeout(DAEMON_READY_TIMEOUT_MS) {
                            (context as MainActivity).bindService()
                        }
                        viewModel.onServiceStartedSuccessful()
                    } catch (e: CancellationException) {
                        // Timeout
                        viewModel.onServiceStartedTimeout()
                    }
                }
            PrivacyDisclaimerUiSideEffect.NavigateToSplash -> navigator.navigate(SplashNavKey)
        }
    }
    PrivacyDisclaimerScreen(
        state = state,
        onAcceptClicked = viewModel::setPrivacyDisclosureAccepted,
    )
}

@Composable
fun PrivacyDisclaimerScreen(state: PrivacyDisclaimerViewState, onAcceptClicked: () -> Unit) {
    val topColor = MaterialTheme.colorScheme.primary
    ScaffoldWithTopBar(topBarColor = topColor, onAccountClicked = null, onSettingsClicked = null) {
        val scrollState = rememberScrollState()
        Column(
            Modifier.padding(it)
                .fillMaxSize()
                .background(color = MaterialTheme.colorScheme.surface)
                .verticalScroll(scrollState)
                .padding(
                    start = Dimens.sideMargin,
                    end = Dimens.sideMargin,
                    top = Dimens.screenTopMargin,
                    bottom = Dimens.screenBottomMargin,
                )
                .drawVerticalScrollbar(
                    state = scrollState,
                    color = MaterialTheme.colorScheme.onPrimary.copy(alpha = AlphaScrollbar),
                ),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Content(state.isPlayBuild)

            ButtonPanel(state.isStartingService, onAcceptClicked)
        }
    }
}

@Composable
private fun Content(isPlayBuild: Boolean) {
    Column {
        Text(
            text = stringResource(id = R.string.privacy_disclaimer_title),
            style = MaterialTheme.typography.headlineLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )

        Spacer(modifier = Modifier.height(Dimens.smallPadding))

        Text(
            text = stringResource(id = R.string.privacy_disclaimer_body_first_paragraph),
            color = MaterialTheme.colorScheme.onSurface,
            style = MaterialTheme.typography.bodyMedium,
        )

        Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing))

        Text(
            text = stringResource(id = R.string.privacy_disclaimer_body_second_paragraph),
            color = MaterialTheme.colorScheme.onSurface,
            style = MaterialTheme.typography.bodyMedium,
        )

        Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing))

        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = buildPrivacyPolicyAnnotatedString(isPlayBuild),
                modifier = Modifier.padding(end = Dimens.miniPadding),
                style = MaterialTheme.typography.bodyMedium,
            )

            Icon(
                imageVector = Icons.AutoMirrored.Rounded.OpenInNew,
                contentDescription = null,
                modifier =
                    Modifier.align(Alignment.CenterVertically).size(Dimens.privacyPolicyIconSize),
                tint = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun buildPrivacyPolicyAnnotatedString(isPlayBuild: Boolean) = buildAnnotatedString {
    withLink(
        LinkAnnotation.Url(
            stringResource(R.string.privacy_policy_url).appendHideNavOnPlayBuild(isPlayBuild)
        )
    ) {
        withStyle(
            style =
                SpanStyle(
                    color = MaterialTheme.colorScheme.onSurface,
                    textDecoration = TextDecoration.Underline,
                )
        ) {
            append(stringResource(id = R.string.privacy_policy_label))
        }
    }
}

@Composable
private fun ButtonPanel(isStartingService: Boolean, onAcceptClicked: () -> Unit) {
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        if (isStartingService) {
            WarrenCircularProgressIndicatorMedium()
        } else {
            PrimaryButton(
                text = stringResource(id = R.string.agree_and_continue),
                onClick = onAcceptClicked,
            )
        }
    }
}
