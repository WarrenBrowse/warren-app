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
import com.warrenbrowse.vpn.feature.account.impl.navigation.accountEntry
import com.warrenbrowse.vpn.feature.addtime.impl.navigation.addTimeVerificationPendingEntry
import com.warrenbrowse.vpn.feature.anticensorship.impl.navigation.anticensorshipEntry
import com.warrenbrowse.vpn.feature.apiaccess.impl.navigation.apiAccessEntry
import com.warrenbrowse.vpn.feature.appearance.impl.navigation.appearanceEntry
import com.warrenbrowse.vpn.feature.appicon.impl.navigation.appIconEntry
import com.warrenbrowse.vpn.feature.appinfo.impl.navigation.changelogEntry
import com.warrenbrowse.vpn.feature.autoconnect.impl.navigation.autoConnectEntry
import com.warrenbrowse.vpn.feature.customlist.impl.navigation.customListEntry
import com.warrenbrowse.vpn.feature.daita.impl.navigation.daitaEntry
import com.warrenbrowse.vpn.feature.deleteaccount.impl.navigation.deleteAccountEntry
import com.warrenbrowse.vpn.feature.filter.impl.navigation.filterEntry
import com.warrenbrowse.vpn.feature.home.impl.navigation.homeEntry
import com.warrenbrowse.vpn.feature.language.impl.navigation.languageEntry
import com.warrenbrowse.vpn.feature.location.impl.navigation.selectLocationEntry
import com.warrenbrowse.vpn.feature.login.impl.devicelist.navigation.deviceListEntry
import com.warrenbrowse.vpn.feature.login.impl.devicelist.navigation.removeDeviceConfirmationDialogEntry
import com.warrenbrowse.vpn.feature.login.impl.navigation.loginEntry
import com.warrenbrowse.vpn.feature.managedevices.impl.navigation.manageDevicesEntry
import com.warrenbrowse.vpn.feature.multihop.impl.navigation.multihopEntry
import com.warrenbrowse.vpn.feature.notification.impl.navigation.notificationEntry
import com.warrenbrowse.vpn.feature.problemreport.impl.navigation.problemReportEntry
import com.warrenbrowse.vpn.feature.redeemvoucher.impl.navigation.redeemVoucherEntry
import com.warrenbrowse.vpn.feature.serveripoverride.impl.navigation.serverIpOverrideEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.settingsEntry
import com.warrenbrowse.vpn.feature.splittunneling.impl.navigation.splitTunnelingEntry
import com.warrenbrowse.vpn.feature.vpnsettings.impl.navigation.vpnSettingsEntry
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

    val mullvadAppViewModel = koinViewModel<WarrenAppViewModel>()

    val lifecycleOwner = LocalLifecycleOwner.current
    LaunchedEffect(lifecycleOwner) {
        lifecycleOwner.lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            navigationState.backStackFlow.collect { backstack ->
                mullvadAppViewModel.setCurrentBackStack(backstack)
            }
        }
    }

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        CheckNotificationPermission(serviceConnectionManager)
    }

    val entryProvider = entryProvider {
        accountEntry(nav3)
        addTimeVerificationPendingEntry(nav3)
        anticensorshipEntry(nav3)
        apiAccessEntry(nav3)
        appIconEntry(nav3)
        appearanceEntry(nav3)
        autoConnectEntry(nav3)
        changelogEntry(nav3)
        customListEntry(nav3)
        daitaEntry(nav3)
        deleteAccountEntry(nav3)
        deviceListEntry(nav3)
        filterEntry(nav3)
        homeEntry(nav3)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            languageEntry(nav3)
        }
        loginEntry(nav3)
        manageDevicesEntry(nav3)
        multihopEntry(nav3)
        noDaemonEntry(nav3)
        notificationEntry(nav3)
        privacyDisclaimerEntry(nav3)
        problemReportEntry(nav3)
        redeemVoucherEntry(nav3)
        removeDeviceConfirmationDialogEntry(nav3)
        selectLocationEntry(nav3)
        serverIpOverrideEntry(nav3)
        settingsEntry(nav3)
        splashEntry(nav3)
        splitTunnelingEntry(nav3)
        vpnSettingsEntry(nav3)
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
        mullvadAppViewModel.uiSideEffect.collect {
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
