package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.fragment.app.FragmentActivity
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import org.koin.compose.koinInject

/**
 * D.5 wallet settings host screen. Wraps [WarrenWalletSettingsSection]
 * inside the standard settings scaffold and threads the host
 * [FragmentActivity] required by `BiometricPrompt`.
 *
 * Reached via [com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey]
 * from the main Settings screen ("Wallet" entry).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenWalletSettings(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val walletRepository = koinInject<WalletRepository>()

    ScaffoldWithSmallTopBar(
        appBarTitle = "Wallet",
        navigationIcon = {
            NavigateBackIconButton(onNavigateBack = {
                navigator.goBackUntil(SettingsNavKey)
            })
        },
    ) { modifier ->
        Column(modifier = Modifier.fillMaxSize().then(modifier)) {
            WarrenWalletSettingsSection(
                activity = activity,
                walletRepository = walletRepository,
            )
        }
    }
}
