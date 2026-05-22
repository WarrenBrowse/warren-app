@file:Suppress("MatchingDeclarationName")

package com.warrenbrowse.vpn.app

import android.Manifest
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.scene.DialogSceneStrategy
import androidx.navigation3.scene.SinglePaneSceneStrategy
import androidx.navigation3.ui.NavDisplay
import co.touchlab.kermit.Logger
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.isGranted
import com.google.accompanist.permissions.rememberPermissionState
import kotlinx.coroutines.cancel
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.common.compose.accessibilityDataSensitive
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.TRANSITION_DEFAULT_DURATION_MS
import com.warrenbrowse.vpn.core.rememberNavigationState
import com.warrenbrowse.vpn.core.rememberResultStore
import com.warrenbrowse.vpn.core.scene.SingleOverlaySceneStrategy
import com.warrenbrowse.vpn.core.scene.rememberListDetailSceneStrategy
import com.warrenbrowse.vpn.core.toEntries
import com.warrenbrowse.vpn.feature.appearance.impl.navigation.appearanceEntry
import com.warrenbrowse.vpn.feature.appicon.impl.navigation.appIconEntry
import com.warrenbrowse.vpn.feature.appinfo.impl.navigation.changelogEntry
import com.warrenbrowse.vpn.feature.autoconnect.impl.navigation.autoConnectEntry
import com.warrenbrowse.vpn.feature.home.impl.navigation.homeEntry
import com.warrenbrowse.vpn.feature.language.impl.navigation.languageEntry
import com.warrenbrowse.vpn.feature.login.impl.navigation.walletEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.walletSettingsEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenLocationPickerEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenTunnelSettingsEntry
import com.warrenbrowse.vpn.feature.notification.impl.navigation.notificationEntry
import com.warrenbrowse.vpn.feature.problemreport.impl.navigation.problemReportEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.settingsEntry
import com.warrenbrowse.vpn.feature.splittunneling.impl.navigation.splitTunnelingEntry
import com.warrenbrowse.vpn.screen.navigation.NoDaemonNavKey
import com.warrenbrowse.vpn.screen.navigation.SplashNavKey
import com.warrenbrowse.vpn.screen.navigation.noDaemonEntry
import com.warrenbrowse.vpn.screen.navigation.privacyDisclaimerEntry
import com.warrenbrowse.vpn.screen.navigation.splashEntry
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionManager
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionState
import org.koin.androidx.compose.koinViewModel

@OptIn(
    ExperimentalComposeUiApi::class,
    ExperimentalSharedTransitionApi::class,
    ExperimentalPermissionsApi::class,
)
@Composable
@Suppress("LongMethod")
fun WarrenApp(serviceConnectionManager: ServiceConnectionManager) {
    val resultStore = rememberResultStore()
    val navigationState = rememberNavigationState(SplashNavKey)

    val listDetailStrategy = rememberListDetailSceneStrategy<NavKey2>()
    val dialogStrategy = remember { DialogSceneStrategy<NavKey2>() }
    val bottomSheetStrategy = remember { SingleOverlaySceneStrategy<NavKey2>() }
    val singlePaneStrategy = remember { SinglePaneSceneStrategy<NavKey2>() }

    val nav3 = remember {
        Navigator(
            state = navigationState,
            resultStore = resultStore,
            screenIsListDetailTargetWidth = listDetailStrategy.isListDetailTargetWidth(),
        )
    }

    val warrenAppViewModel = koinViewModel<WarrenAppViewModel>()

    val lifecycleOwner = LocalLifecycleOwner.current
    LaunchedEffect(lifecycleOwner) {
        lifecycleOwner.lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            navigationState.backStackFlow.collect { backstack ->
                warrenAppViewModel.setCurrentBackStack(backstack)
            }
        }
    }

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        CheckNotificationPermission(serviceConnectionManager)
    }

    val entryProvider = entryProvider {
        // D.4 step 16: Mullvad-legacy entries (account / addTime / deleteAccount /
        // manageDevices / redeemVoucher) removed from the live navigation graph.
        // The screens still compile (modules retained) but no NavKey routes them
        // anymore. Full module deletion is a follow-up.
        // D.4 step 34: anticensorshipEntry removed (Warren uses native
        // Quinn + M4.0 toggle in WarrenTunnelSettings).
        // D.4 step 33: apiAccessEntry removed (Warren API endpoint fixed).
        appIconEntry(nav3)
        appearanceEntry(nav3)
        autoConnectEntry(nav3)
        changelogEntry(nav3)
        // D.4 step 26: customListEntry removed - Mullvad custom relay lists
        // were reached only from SelectLocationScreen (now unreachable).
        // D.4 step 32: daitaEntry removed - DAITA is now configured via
        // the unified WarrenTunnelSettings toggles (the dedicated Mullvad
        // DaitaScreen is unreachable).
        // D.4 step 26: filterEntry removed - Mullvad relay filter was
        // reached only from SelectLocationScreen (now unreachable).
        homeEntry(nav3)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            languageEntry(nav3)
        }
        // D.4 step 24: loginEntry + deviceListEntry +
        // removeDeviceConfirmationDialogEntry removed from the live
        // navigation graph - on Warren, onboarding goes via
        // WarrenWalletNavKey (D.5) and no path pushes Mullvad
        // LoginNavKey / DeviceListNavKey anymore.
        walletEntry(nav3)
        walletSettingsEntry(nav3)
        warrenTunnelSettingsEntry(nav3)
        warrenLocationPickerEntry(nav3)
        // D.4 step 32: multihopEntry removed - Multihop now configured
        // via the unified WarrenTunnelSettings toggles.
        noDaemonEntry(nav3)
        notificationEntry(nav3)
        privacyDisclaimerEntry(nav3)
        problemReportEntry(nav3)
        // D.4 step 25: selectLocationEntry removed - Mullvad SelectLocation
        // is unreachable (Switch button routes to WarrenLocationPicker).
        // D.4 step 35: serverIpOverrideEntry removed - Warren exit fleet
        // is sovereign, no per-relay overrides needed.
        settingsEntry(nav3)
        splashEntry(nav3)
        splitTunnelingEntry(nav3)
        // D.4 step 53: vpnSettingsEntry removed (VpnSettings module deleted —
        // Mullvad daemon settings sync dead, Warren-native settings live in
        // WarrenTunnelSettings).
    }

    SharedTransitionLayout {
        CompositionLocalProvider(LocalSharedTransitionScope provides this@SharedTransitionLayout) {
            CompositionLocalProvider(LocalResultStore provides resultStore) {
                NavDisplay(
                    modifier =
                        Modifier.semantics { testTagsAsResourceId = true }
                            .fillMaxSize()
                            .accessibilityDataSensitive(),
                    sceneStrategies =
                        listOf(
                            listDetailStrategy,
                            dialogStrategy,
                            bottomSheetStrategy,
                            singlePaneStrategy,
                        ),
                    entries = navigationState.toEntries(entryProvider),
                    onBack = { nav3.goBack() },
                    sharedTransitionScope = this@SharedTransitionLayout,
                    transitionSpec = { defaultNavDisplayTransitionSpec() },
                    popTransitionSpec = { defaultNavDisplayTransitionSpec() },
                    predictivePopTransitionSpec = { defaultNavDisplayTransitionSpec() },
                )
            }
        }
    }

    // For the following LaunchedEffect we do not use CollectSideEffectWithLifecycle since we
    // collect from StateFlow/SharedFlow with replay and don't want to trigger a navigation again.

    // Globally handle daemon dropped connection with NoDaemonScreen
    LaunchedEffect(Unit) {
        warrenAppViewModel.uiSideEffect.collect {
            Logger.i { "DaemonScreenEvent: $it" }
            when (it) {
                DaemonScreenEvent.Show -> nav3.navigate(NoDaemonNavKey)

                DaemonScreenEvent.Remove -> nav3.goBackUntil(NoDaemonNavKey, inclusive = true)
            }
        }
    }
}

private fun defaultNavDisplayTransitionSpec(): ContentTransform =
    fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)) togetherWith
        fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS))

@OptIn(ExperimentalPermissionsApi::class)
@Composable
@RequiresApi(Build.VERSION_CODES.TIRAMISU)
private fun CheckNotificationPermission(serviceConnectionManager: ServiceConnectionManager) {
    val notificationPermission =
        rememberPermissionState(permission = Manifest.permission.POST_NOTIFICATIONS)
    LaunchedEffect(Unit) {
        serviceConnectionManager.connectionState.collect {
            if (it is ServiceConnectionState.Bound) {
                if (!notificationPermission.status.isGranted) {
                    notificationPermission.launchPermissionRequest()
                    cancel(
                        message =
                            "We should only show one notification permission dialog per app start"
                    )
                }
            }
        }
    }
}
