package com.warrenbrowse.vpn.screen.splash

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.R
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.screen.navigation.OnboardingNavKey
import com.warrenbrowse.vpn.screen.navigation.PrivacyDisclaimerNavKey
import org.koin.androidx.compose.koinViewModel

@Preview
@Composable
private fun PreviewLoadingScreen() {
    AppTheme { SplashScreen() }
}

// Set this as the start destination of the default nav graph
@Composable
fun Splash(navigator: Navigator) {
    val viewModel: SplashViewModel = koinViewModel()

    // We use CollectSideEffectWithLifecycle to re-evaluate the splash decision if the user
    // navigates away from the app to the resume before we leave the splash screen
    CollectSideEffectWithLifecycle(viewModel.uiSideEffect) {
        when (it) {
            SplashUiSideEffect.NavigateToConnect ->
                navigator.navigate(ConnectNavKey, clearBackStack = true)
            SplashUiSideEffect.NavigateToPrivacyDisclaimer ->
                navigator.navigate(PrivacyDisclaimerNavKey, clearBackStack = true)
            SplashUiSideEffect.NavigateToWallet ->
                navigator.navigate(WarrenWalletNavKey, clearBackStack = true)
            SplashUiSideEffect.NavigateToOnboarding ->
                navigator.navigate(OnboardingNavKey, clearBackStack = true)
        }
    }

    SplashScreen()
}

@Composable
fun SplashScreen() {

    val backgroundColor = MaterialTheme.colorScheme.primary

    ScaffoldWithTopBar(
        topBarColor = backgroundColor,
        onSettingsClicked = null,
        onAccountClicked = null,
        isIconAndLogoVisible = false,
        content = {
            Box(
                contentAlignment = Alignment.Center,
                modifier =
                    Modifier.background(backgroundColor)
                        .padding(it)
                        .padding(bottom = it.calculateTopPadding())
                        .fillMaxSize(),
            ) {
                Column(
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Image(
                        painter = painterResource(id = R.drawable.launch_logo),
                        contentDescription = "",
                        modifier = Modifier.size(Dimens.splashLogoSize),
                    )
                    Image(
                        painter = painterResource(id = R.drawable.logo_text),
                        contentDescription = "",
                        alpha = 0.6f,
                        modifier =
                            Modifier.padding(top = Dimens.mediumPadding)
                                .height(Dimens.splashLogoTextHeight),
                    )
                    Text(
                        text = stringResource(id = R.string.connecting_to_daemon),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = Dimens.mediumPadding),
                    )
                }
            }
        },
    )
}
