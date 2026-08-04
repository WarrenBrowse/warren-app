@file:Suppress("MatchingDeclarationName")

package com.warrenbrowse.vpn.app

import android.Manifest
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.scene.DialogSceneStrategy
import androidx.navigation3.scene.Scene
import androidx.navigation3.scene.SinglePaneSceneStrategy
import androidx.navigation3.ui.NavDisplay
import co.touchlab.kermit.Logger
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.isGranted
import com.google.accompanist.permissions.rememberPermissionState
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.common.compose.accessibilityDataSensitive
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.core.animation.ENTER_TRANSITION_SLIDE_FACTOR
import com.warrenbrowse.vpn.core.animation.TRANSITION_DEFAULT_DURATION_MS
import com.warrenbrowse.vpn.core.rememberNavigationState
import com.warrenbrowse.vpn.core.rememberResultStore
import com.warrenbrowse.vpn.core.scene.SingleOverlaySceneStrategy
import com.warrenbrowse.vpn.core.scene.rememberListDetailSceneStrategy
import com.warrenbrowse.vpn.core.toEntries
import com.warrenbrowse.vpn.feature.appinfo.impl.navigation.changelogEntry
import com.warrenbrowse.vpn.feature.autoconnect.impl.navigation.autoConnectEntry
import com.warrenbrowse.vpn.feature.home.impl.navigation.homeEntry
import com.warrenbrowse.vpn.feature.language.impl.navigation.languageEntry
import com.warrenbrowse.vpn.feature.login.impl.navigation.walletEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.walletSettingsEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenDaitaSettingsEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenLocationPickerEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenMultihopSettingsEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenPortForwardingSettingsEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.warrenTunnelSettingsEntry
import com.warrenbrowse.vpn.feature.notification.impl.navigation.notificationEntry
import com.warrenbrowse.vpn.feature.settings.impl.navigation.settingsEntry
import com.warrenbrowse.vpn.feature.splittunneling.impl.navigation.splitTunnelingEntry
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import com.warrenbrowse.vpn.screen.navigation.NoDaemonNavKey
import com.warrenbrowse.vpn.screen.navigation.SplashNavKey
import com.warrenbrowse.vpn.screen.navigation.noDaemonEntry
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.screen.navigation.OnboardingBetaAccessNavKey
import com.warrenbrowse.vpn.screen.navigation.OnboardingSubscriptionNavKey
import com.warrenbrowse.vpn.screen.navigation.OutOfTimeNavKey
import com.warrenbrowse.vpn.screen.navigation.onboardingDoneEntry
import com.warrenbrowse.vpn.screen.navigation.onboardingEntry
import com.warrenbrowse.vpn.screen.navigation.onboardingBetaAccessEntry
import com.warrenbrowse.vpn.screen.navigation.onboardingPreferencesEntry
import com.warrenbrowse.vpn.screen.navigation.onboardingSubscriptionEntry
import com.warrenbrowse.vpn.screen.navigation.outOfTimeEntry
import com.warrenbrowse.vpn.screen.navigation.privacyDisclaimerEntry
import com.warrenbrowse.vpn.screen.navigation.reachedFromSplash
import com.warrenbrowse.vpn.screen.navigation.splashEntry
import com.warrenbrowse.vpn.screen.outoftime.OutOfTimeGateAction
import com.warrenbrowse.vpn.screen.outoftime.outOfTimeGateAction
import com.warrenbrowse.vpn.screen.outoftime.outOfTimeGateActive
import com.warrenbrowse.vpn.screen.unsupportedversion.UnsupportedVersionBlocked
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionManager
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionState
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

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

    // Forced-update gate: when the signed update manifest reports the running
    // version is no longer supported, replace the whole UI with the blocking
    // screen (the tunnel is left untouched).
    val appVersionInfoRepository = koinInject<AppVersionInfoRepository>()
    val appVersionInfo = appVersionInfoRepository.versionInfo.collectAsState().value

    // Out-of-time gate (desktop ExpiredAccountErrorView, iOS .outOfTime): when
    // a previously funded subscription has lapsed, or the exit refused the
    // account, replace the main UI with the full-screen renewal surface. Only
    // raised inside the main (Connect-rooted) flow: onboarding / login /
    // funding flows win over the gate, and it dismisses reactively the moment
    // credit lands in the shared cached expiry.
    val localSettings = koinInject<WarrenLocalSettingsRepository>()
    val productFlags = koinInject<WarrenProductFlags>()
    val tunnelStateProvider = koinInject<WarrenTunnelStateProvider>()
    val cachedExpiry = localSettings.cachedSubscriptionExpiry.collectAsState().value
    val connectedInfo = tunnelStateProvider.connectedInfo.collectAsState().value
    val tunnelExpired = when (connectedInfo) {
        is WarrenConnectedInfo.Blocking -> connectedInfo.expired
        is WarrenConnectedInfo.Failed -> connectedInfo.expired
        else -> false
    }
    var nowSecs by remember { mutableLongStateOf(System.currentTimeMillis() / 1000) }
    LaunchedEffect(cachedExpiry) {
        // Re-arm on every expiry change so the gate raises itself the moment
        // the subscription lapses while the app is open (iOS timer parity);
        // between boundaries a coarse tick is enough.
        while (true) {
            nowSecs = System.currentTimeMillis() / 1000
            val untilExpirySecs = cachedExpiry - nowSecs
            val tickSecs =
                if (untilExpirySecs in 1..OUT_OF_TIME_TICK_SECS) untilExpirySecs
                else OUT_OF_TIME_TICK_SECS
            delay(tickSecs * 1_000)
        }
    }
    val inMainFlow = navigationState.backStack.any { it is ConnectNavKey }
    val outOfTime = outOfTimeGateActive(
        expiryUnixSecs = cachedExpiry,
        nowSecs = nowSecs,
        inMainFlow = inMainFlow,
        tunnelExpired = tunnelExpired,
    )
    val outOfTimeInStack = navigationState.backStack.any { it is OutOfTimeNavKey }
    // Clearing pops the gate AND anything opened from it (Settings, the account
    // page): those were an escape from the interruption, so when the
    // interruption lifts the app returns to where it started.
    LaunchedEffect(outOfTime, outOfTimeInStack) {
        when (outOfTimeGateAction(active = outOfTime, inStack = outOfTimeInStack)) {
            OutOfTimeGateAction.Push -> nav3.navigate(OutOfTimeNavKey)
            OutOfTimeGateAction.Pop -> nav3.goBackUntil(OutOfTimeNavKey, inclusive = true)
            OutOfTimeGateAction.None -> Unit
        }
    }

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
        autoConnectEntry(nav3)
        changelogEntry(nav3)
        homeEntry(nav3)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            languageEntry(nav3)
        }
        // First-run onboarding continues the post-wallet step into the
        // subscription -> done wizard (beta flavor: the auto-granted beta
        // access step, no payment surface); every other wallet entry goes
        // to Connect.
        walletEntry(
            navigator = nav3,
            postWalletDestination = { onboarding ->
                when {
                    onboarding && productFlags.isBeta -> OnboardingBetaAccessNavKey
                    onboarding -> OnboardingSubscriptionNavKey
                    else -> ConnectNavKey
                }
            },
            reachedByRootSwap = { nav3.reachedFromSplash() },
        )
        walletSettingsEntry(nav3)
        warrenTunnelSettingsEntry(nav3)
        warrenDaitaSettingsEntry(nav3)
        warrenMultihopSettingsEntry(nav3)
        warrenPortForwardingSettingsEntry(nav3)
        warrenLocationPickerEntry(nav3)
        noDaemonEntry(nav3)
        notificationEntry(nav3)
        onboardingEntry(nav3)
        onboardingSubscriptionEntry(nav3)
        onboardingBetaAccessEntry(nav3)
        onboardingPreferencesEntry(nav3)
        onboardingDoneEntry(nav3)
        outOfTimeEntry(nav3)
        privacyDisclaimerEntry(nav3)
        settingsEntry(nav3)
        splashEntry(nav3)
        splitTunnelingEntry(nav3)
    }

    val gate =
        when {
            !appVersionInfo.isSupported -> AppGate.UnsupportedVersion
            else -> AppGate.None
        }

    // The forced-update gate raises and clears itself reactively while the user is looking at
    // the app (the signed update manifest), and there is nothing to navigate to behind it, so
    // it stays a modal interrupt rather than a destination: it rises over the running UI and
    // drops back off it. The out-of-time gate is a destination instead (see OutOfTimeNavKey):
    // Settings and the account page have to stay reachable from it.
    AnimatedContent(
        targetState = gate,
        modifier = Modifier.fillMaxSize(),
        transitionSpec = { appGateTransitionSpec() },
        label = "app_gate",
    ) { currentGate ->
        when (currentGate) {
            AppGate.UnsupportedVersion -> UnsupportedVersionBlocked()
            // The SharedTransitionLayout stays inside the branch so the gate transition
            // does not tear its scope down under the screens that are using it.
            AppGate.None ->
                SharedTransitionLayout {
                    CompositionLocalProvider(
                        LocalSharedTransitionScope provides this@SharedTransitionLayout
                    ) {
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
                                // The gate is not dismissible: backing out of it would reveal
                                // the main flow it exists to hold, and the reconciler above
                                // would immediately push it back.
                                onBack = {
                                    val top = navigationState.backStack.lastOrNull()
                                    if (top !is OutOfTimeNavKey) nav3.goBack()
                                },
                                sharedTransitionScope = this@SharedTransitionLayout,
                                transitionSpec = { forwardNavDisplayTransitionSpec() },
                                popTransitionSpec = { popNavDisplayTransitionSpec() },
                                predictivePopTransitionSpec = { popNavDisplayTransitionSpec() },
                            )
                        }
                    }
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

/** Which full-screen interrupt, if any, is currently standing in front of the whole app. */
private sealed interface AppGate {
    data object None : AppGate

    data object UnsupportedVersion : AppGate
}

private fun AnimatedContentTransitionScope<AppGate>.appGateTransitionSpec(): ContentTransform =
    if (targetState is AppGate.None) {
        // Clearing: the gate drops away and reveals the app that was behind it, so the app
        // has to be drawn underneath rather than on top of the leaving gate.
        ContentTransform(
            targetContentEnter = EnterTransition.None,
            initialContentExit =
                slideOutVertically(
                    animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
                    targetOffsetY = { it },
                ) + fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS)),
            targetContentZIndex = -1f,
            sizeTransform = SizeTransform(clip = false),
        )
    } else {
        ContentTransform(
            targetContentEnter =
                slideInVertically(
                    animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
                    initialOffsetY = { it },
                ) + fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)),
            initialContentExit = ExitTransition.None,
            targetContentZIndex = 1f,
            sizeTransform = SizeTransform(clip = false),
        )
    }

// Fallback for an entry that declares no transition metadata: still directional, so a screen
// added later reads as going deeper and coming back rather than as an undirected crossfade.
private fun AnimatedContentTransitionScope<Scene<NavKey2>>.forwardNavDisplayTransitionSpec():
    ContentTransform =
    fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)) +
        slideIntoContainer(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            towards = AnimatedContentTransitionScope.SlideDirection.Start,
            initialOffset = { (it * ENTER_TRANSITION_SLIDE_FACTOR).toInt() },
        ) togetherWith ExitTransition.None

// Exact mirror of the forward spec, shared by the button and the predictive back gesture so
// the gesture preview shows the same movement the pop will finish.
private fun AnimatedContentTransitionScope<Scene<NavKey2>>.popNavDisplayTransitionSpec():
    ContentTransform =
    EnterTransition.None togetherWith
        slideOutOfContainer(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            towards = AnimatedContentTransitionScope.SlideDirection.End,
            targetOffset = { (it * ENTER_TRANSITION_SLIDE_FACTOR).toInt() },
        ) + fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS))

// Coarse re-evaluation tick for the out-of-time gate between expiry boundaries.
private const val OUT_OF_TIME_TICK_SECS = 60L

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
