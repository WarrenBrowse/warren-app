package com.warrenbrowse.vpn.screen.nodaemon

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.core.app.ActivityCompat.finishAffinity
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.R
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.component.WarrenLogoMark
import com.warrenbrowse.vpn.lib.ui.component.WarrenLogoState
import com.warrenbrowse.vpn.lib.ui.component.WarrenLogoTone
import com.warrenbrowse.vpn.lib.ui.component.WarrenWordmark
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Preview
@Composable
private fun PreviewNoDaemonScreen() {
    AppTheme { NoDaemonScreen(onNavigateToSettings = {}) }
}

// Set this as the start destination of the default nav graph
@Composable
fun NoDaemon(navigator: Navigator) {
    NoDaemonScreen(onNavigateToSettings = dropUnlessResumed { navigator.navigate(SettingsNavKey) })
}

@Composable
fun NoDaemonScreen(onNavigateToSettings: () -> Unit) {

    val backgroundColor = MaterialTheme.colorScheme.primary

    val context = LocalContext.current
    BackHandler { finishAffinity(context.getActivity()!!) }

    ScaffoldWithTopBar(
        topBarColor = backgroundColor,
        onSettingsClicked = onNavigateToSettings,
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
                    WarrenLogoMark(
                        state = WarrenLogoState.Exposed,
                        tone = WarrenLogoTone.Light,
                        height = Dimens.splashLogoSize,
                    )
                    WarrenWordmark(
                        color = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.padding(top = Dimens.mediumPadding),
                    )
                    Text(
                        text = stringResource(id = R.string.connecting_to_daemon),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier =
                            Modifier.padding(top = Dimens.mediumPadding)
                                .padding(horizontal = Dimens.sideMargin),
                        textAlign = TextAlign.Center,
                    )
                }
            }
        },
    )
}
