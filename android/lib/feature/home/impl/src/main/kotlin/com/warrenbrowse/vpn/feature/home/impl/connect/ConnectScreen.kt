package com.warrenbrowse.vpn.feature.home.impl.connect

import android.app.Activity
import android.content.res.Resources
import androidx.activity.compose.BackHandler
import androidx.activity.compose.ReportDrawn
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.common.compose.LocalNavAnimatedVisibilityScope
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.common.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.isTv
import com.warrenbrowse.vpn.common.compose.safeOpenUri
import com.warrenbrowse.vpn.common.compose.showSnackbarImmediately
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.appinfo.api.ChangelogNavKey
import com.warrenbrowse.vpn.feature.home.api.Android16UpgradeInfoNavKey
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.AlwaysExpandedFeatureIndicators
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.ConnectionDetailPanel
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.isUnspecified
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.toInAddress
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.toOutAddress
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.NotificationBanner
import com.warrenbrowse.vpn.feature.managedevices.api.ManageDevicesNavKey
import com.warrenbrowse.vpn.feature.settings.api.ConnectAfterLocationPick
import com.warrenbrowse.vpn.feature.settings.api.ForumActivityNavKey
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenDaitaSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenMultihopSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenPortForwardingSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.splittunneling.api.SplitTunnelingNavKey
import com.warrenbrowse.vpn.lib.common.util.CreateVpnProfile
import com.warrenbrowse.vpn.lib.common.util.openVpnSettings
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.common.util.removeHtmlTags
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.model.loadBadgeOf
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectResult
import com.warrenbrowse.vpn.lib.repository.WarrenIncidentReporter
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.tv.NavigationDrawerTv
import com.warrenbrowse.vpn.lib.tv.TvForumItem
import com.warrenbrowse.vpn.lib.ui.component.BetaBadge
import com.warrenbrowse.vpn.lib.ui.component.BetaBadgeVariant
import com.warrenbrowse.vpn.lib.ui.component.ForumHeaderSlot
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.networkstats.ExitLoadAppearance
import com.warrenbrowse.vpn.lib.ui.component.networkstats.ExitLoadBadge
import com.warrenbrowse.vpn.lib.ui.component.networkstats.rememberSnapshotStale
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSnackbar
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

@Preview("Initial|Connected|Disconnected|Connecting|Error.VpnPermissionDenied")
@Composable
private fun PreviewAccountScreen(
    @PreviewParameter(ConnectUiStatePreviewParameterProvider::class) state: ConnectUiState
) {
    AppTheme {
        ConnectScreen(
            state = state,
            snackbarHostState = SnackbarHostState(),
            onDisconnectClick = {},
            onConnectClick = {},
            onCancelClick = {},
            onSwitchLocationClick = {},
            onShuffleClick = {},
            onOpenAppListing = {},
            onManageAccountClick = {},
            onChangelogClick = {},
            onDismissChangelogClick = {},
            onSettingsClick = {},
            onAccountClick = {},
            onNavigateToFeature = {},
            onClickDismissAndroid16UpgradeWarning = {},
            onClickShowAndroid16UpgradeInfo = {},
        )
    }
}

@Suppress("LongMethod", "CyclomaticComplexMethod")
@Composable
fun Connect(navigator: Navigator, animatedVisibilityScope: AnimatedVisibilityScope) {
    val connectViewModel: ConnectViewModel = koinViewModel()
    val warrenConnect = koinInject<WarrenQuinnConnectInvoker>()
    val warrenDisconnect = koinInject<WarrenQuinnDisconnectInvoker>()
    val warrenReconnect = koinInject<WarrenQuinnReconnectInvoker>()
    val subscriptionInvoker = koinInject<WarrenSubscriptionInvoker>()
    val localSettings = koinInject<WarrenLocalSettingsRepository>()
    // The key-mismatch dialog's "Report to Warren": a signed incident post,
    // the only telemetry this screen sends and only when the user asks.
    val incidentReporter = koinInject<WarrenIncidentReporter>()
    // Beta program surfaces: the badge is gated on the COMPILED flavor;
    // the network-info feed only supplies the live cap figure.
    val productFlags = koinInject<WarrenProductFlags>()
    val networkInfoProvider = koinInject<WarrenNetworkInfoProvider>()
    val networkInfo by networkInfoProvider.networkInfo.collectAsStateWithLifecycle()
    // The feed is a network round trip, so a cold start would render the
    // cap-unknown wording and swap it a second later. The last answer is
    // cached, and until an answer exists the badge holds its line rather than
    // showing one it is about to contradict.
    val cachedRateBps by localSettings.cachedNetworkRateBps.collectAsStateWithLifecycle()
    LaunchedEffect(networkInfo) {
        networkInfo?.let { localSettings.setCachedNetworkRateBps(it.defaultRateBps) }
    }
    val betaCapResolved = networkInfo != null || cachedRateBps != null
    val betaCapBps = if (networkInfo != null) networkInfo?.defaultRateBps else cachedRateBps
    val cachedExpiry by localSettings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    val context = LocalContext.current
    // The forum slot: one number and one verdict computed once for every
    // surface (the monitor in `lib/repository`), read here for the header.
    val forumActivity = koinInject<ForumActivityState>()
    val forumUnread by forumActivity.unread.collectAsStateWithLifecycle()
    val forumHeaderButton by forumActivity.headerButton.collectAsStateWithLifecycle()
    val forumSlot =
        remember(forumHeaderButton, forumUnread) {
            when (forumHeaderButton) {
                ForumHeaderButton.ACTIVITY -> ForumHeaderSlot.Activity(forumUnread)
                ForumHeaderButton.COMMUNITY -> ForumHeaderSlot.Community
                ForumHeaderButton.NONE -> null
            }
        }
    val forumUrl = stringResource(R.string.community_forum_url)

    val state by connectViewModel.uiState.collectAsStateWithLifecycle()
    val connectedExitLoad by connectViewModel.connectedExitLoad.collectAsStateWithLifecycle()
    // Time-to-fully-drawn ends at this screen's first frame: `am start -W`
    // stops at the splash, and every input of that first frame (tunnel state,
    // wallet, pin, cached labels) is a synchronous local read, so no later
    // frame adds content worth waiting for. The relay catalogue is a network
    // fetch and deliberately not a condition: an offline start would then
    // never report at all.
    ReportDrawn()

    val warrenScope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }
    // TOFU: when the exit's key changed since it was pinned, the use case
    // refuses to dispatch and returns ExitKeyMismatch; hold it here to raise the
    // warning dialog so the user explicitly trusts or rejects the new key.
    var pubkeyMismatch by remember { mutableStateOf<WarrenConnectResult.ExitKeyMismatch?>(null) }
    // The trust decision runs a pin write plus a re-dial, either of which can
    // fail; the dialog stays up and says so rather than vanishing after a
    // security decision that did not take effect.
    var pubkeyBusy by remember { mutableStateOf(false) }
    var pubkeyError by remember { mutableStateOf<String?>(null) }

    val walletNotReadyMessage = stringResource(R.string.connect_wallet_not_ready)
    val authDeniedMessage = stringResource(R.string.connect_authorization_denied)
    val genericErrorMessage = stringResource(R.string.error_occurred)
    val walletActionLabel = stringResource(R.string.wallet_settings_section)

    // Every connect outcome reaches the user: a dispatch that never happened is
    // otherwise indistinguishable from one that did. The engine's own message
    // is never shown, only a mapped one.
    suspend fun surfaceConnectFailure(result: WarrenConnectResult) {
        when (result) {
            WarrenConnectResult.Dispatched -> Unit
            is WarrenConnectResult.ExitKeyMismatch -> {
                pubkeyError = null
                pubkeyMismatch = result
            }
            WarrenConnectResult.WalletNotReady ->
                snackbarHostState.showSnackbarImmediately(
                    message = walletNotReadyMessage,
                    actionLabel = walletActionLabel,
                    withDismissAction = true,
                    onAction = { navigator.navigate(WarrenWalletSettingsNavKey) },
                )
            WarrenConnectResult.AuthorizationDenied ->
                snackbarHostState.showSnackbarImmediately(message = authDeniedMessage)
            is WarrenConnectResult.Failure ->
                snackbarHostState.showSnackbarImmediately(message = genericErrorMessage)
        }
    }

    // Route the user-initiated Connect button through the Warren Quinn
    // use-case. The Quinn path requires a FragmentActivity host for
    // BiometricPrompt; the app's MainActivity extends FragmentActivity.
    val onWarrenConnectClick: () -> Unit = {
        (context as? FragmentActivity)?.let { activity ->
            warrenScope.launch {
                runCatching { warrenConnect.connect(activity) }
                    .onSuccess { result -> surfaceConnectFailure(result) }
                    .onFailure { e ->
                        co.touchlab.kermit.Logger.e(throwable = e) { "warren connect failed" }
                        snackbarHostState.showSnackbarImmediately(message = genericErrorMessage)
                    }
            }
        }
    }

    // Account identity for the header second row (desktop AppMainHeader): the
    // public key is public, shown short with a copy action whether the wallet
    // is locked or ready.
    val walletRepository = koinInject<WalletRepository>()
    val walletState by walletRepository.state.collectAsStateWithLifecycle()
    val fullPubkey = when (val s = walletState) {
        is WalletState.Ready -> s.pubkey.value
        is WalletState.Locked -> s.pubkey.value
        WalletState.Absent -> null
    }
    // Proactively refresh the subscription expiry once the wallet is present, so
    // the header "Time left" and the account "Paid until" reflect the server
    // state without waiting for a purchase/voucher. fetch() caches the result;
    // it reads the mnemonic silently (no prompt) and is a no-op when Absent.
    // Mirrors the desktop daemon keeping account.expiry fresh.
    LaunchedEffect(fullPubkey) {
        if (fullPubkey != null) {
            (context as? FragmentActivity)?.let { activity ->
                runCatching { subscriptionInvoker.fetch(activity) }
                    .onFailure { e ->
                        co.touchlab.kermit.Logger.w(throwable = e) { "subscription fetch failed" }
                    }
            }
        }
    }
    val copyPubkey = createCopyToClipboardHandle(snackbarHostState, isSensitive = false)
    val pubkeyCopiedMsg = stringResource(R.string.wallet_settings_pubkey_copied)

    // The selection the location row names while no tunnel carries traffic
    // ("Automatic" when nothing is pinned), desktop getRelayName.
    val relayProvider = koinInject<WarrenRelayProvider>()
    // Seeding produceState from the cached snapshot keeps the "Automatic" flash
    // away without blocking: resolving the catalogue inline would run the
    // signed fetch during composition and freeze the home screen. The fetch
    // itself only happens once the snapshot is an hour old (the daemon's
    // cadence), not on every return to this screen.
    val relays: List<WarrenRelaySummary> by produceState(initialValue = relayProvider.list()) {
        value = relayProvider.refreshIfStale()
    }
    val exitPin by localSettings.exitPin.collectAsStateWithLifecycle()
    val cachedPinLabel by localSettings.exitPinLabel.collectAsStateWithLifecycle()
    val automaticLabel = stringResource(R.string.automatic)
    // The name the catalogue gives the pinned exit, once it has been fetched.
    val resolvedPinLabel =
        (exitPin as? ExitPin.Exit)?.let { pin ->
            relays.firstOrNull { it.exitId == pin.exitId }
                ?.let { it.city.ifBlank { countryDisplayName(it.country) } }
        }
    // Cached so the next cold start names the exit from the first frame.
    LaunchedEffect(resolvedPinLabel) {
        val pin = exitPin
        if (pin is ExitPin.Exit && resolvedPinLabel != null) {
            localSettings.setExitPinLabel(pin.exitId, resolvedPinLabel)
        }
    }
    // The pin can name any geographical depth, so the row shows the depth the
    // user actually chose rather than a resolved exit; once a tunnel carries
    // traffic the row names the exit instead (locationRowLine).
    val selectedLocationTitle =
        when (val pin = exitPin) {
            ExitPin.Automatic -> automaticLabel
            is ExitPin.Country -> countryDisplayName(pin.country)
            is ExitPin.City -> pin.city.ifBlank { countryDisplayName(pin.country) }
            // "Automatic" is the LAST resort, not the loading state: it
            // contradicts the user's own selection, and printing it before the
            // catalogue lands made every cold start flash the wrong answer.
            is ExitPin.Exit ->
                resolvedPinLabel
                    ?: cachedPinLabel?.takeIf { it.exitId == pin.exitId }?.label
                    ?: if (relays.isEmpty()) null else automaticLabel
        }
    val selectedCountryCode =
        when (val pin = exitPin) {
            ExitPin.Automatic -> null
            is ExitPin.Country -> pin.country
            is ExitPin.City -> pin.country
            is ExitPin.Exit -> relays.firstOrNull { it.exitId == pin.exitId }?.country
        }

    val createVpnProfile =
        rememberLauncherForActivityResult(CreateVpnProfile()) {
            connectViewModel.createVpnProfileResult(it)
        }

    // The Connect button must hold the system VPN consent before dispatching.
    // If it is missing, request it (the granted result re-dispatches the
    // connect via createVpnProfileResult -> RequestWarrenConnect); otherwise
    // connect straight away. Without this, VpnService.establish() returns null
    // and Connect silently does nothing.
    val onConnectButtonClick: () -> Unit = {
        context.prepareVpnSafe().fold(
            ifLeft = { error ->
                when (error) {
                    is PrepareError.NotPrepared -> createVpnProfile.launch(error.prepareIntent)
                    else ->
                        co.touchlab.kermit.Logger.w { "VPN prepare blocked: $error" }
                }
            },
            ifRight = { onWarrenConnectClick() },
        )
    }

    // The exit the shuffle must not land on: the live one when a tunnel exists,
    // the pinned one otherwise. Under Automatic the pin names none, so the live
    // endpoint is the only way to know what the user is already on.
    val activeEndpointHost =
        when (val tunnel = state.tunnelState) {
            is TunnelState.Connected -> tunnel.endpoint.endpoint.hostLiteral()
            is TunnelState.Connecting -> tunnel.endpoint?.endpoint?.hostLiteral()
            else -> null
        }
    val shufflePicks =
        shuffleCandidates(relays, currentExitId(relays, activeEndpointHost, exitPin))

    // Desktop "surprise me" shuffle: pin a random active exit other than the
    // current one and apply it. While a tunnel is up the new exit is applied by
    // reconnecting (reuses the cached mnemonic, no biometric re-prompt, like the
    // picker); while disconnected/errored it connects through the same
    // VPN-consent gate as the Connect button.
    val onShuffleClick: () -> Unit = {
        shufflePicks.randomOrNull()?.let { pick ->
            localSettings.setSelectedExitId(pick.exitId)
            when (state.tunnelState) {
                is TunnelState.Disconnected,
                is TunnelState.Error -> onConnectButtonClick()
                else -> warrenReconnect.reconnect()
            }
        }
    }

    val uriHandler = LocalUriHandler.current
    val resources = LocalResources.current
    CollectSideEffectWithLifecycle(
        connectViewModel.uiSideEffect,
        minActiveState = Lifecycle.State.RESUMED,
    ) { sideEffect ->
        when (sideEffect) {
            // The VM requests a Warren connect dispatch: invoke the
            // WarrenQuinnConnectInvoker on the current FragmentActivity (the
            // invoker needs the Activity host for the biometric prompt).
            ConnectViewModel.UiSideEffect.RequestWarrenConnect -> onWarrenConnectClick()

            is ConnectViewModel.UiSideEffect.NotPrepared ->
                when (sideEffect.prepareError) {
                    is PrepareError.OtherLegacyAlwaysOnVpn ->
                        launch {
                            snackbarHostState.showSnackbarImmediately(
                                message = sideEffect.prepareError.toMessage(resources)
                            )
                        }

                    is PrepareError.OtherAlwaysOnApp ->
                        launch {
                            snackbarHostState.showSnackbarImmediately(
                                message = sideEffect.prepareError.toMessage(resources)
                            )
                        }
                    is PrepareError.NotPrepared ->
                        createVpnProfile.launch(sideEffect.prepareError.prepareIntent)
                }

            ConnectViewModel.UiSideEffect.ConnectError.Generic ->
                snackbarHostState.showSnackbarImmediately(
                    message = resources.getString(R.string.error_occurred)
                )

            is ConnectViewModel.UiSideEffect.ConnectError.PermissionDenied -> {
                launch {
                    snackbarHostState.showSnackbarImmediately(
                        message =
                            resources.getString(
                                if (sideEffect.systemVpnSettingsAvailable) {
                                    R.string.vpn_permission_denied_error
                                } else {
                                    R.string.vpn_permission_denied_error_no_vpn_settings
                                }
                            ),
                        actionLabel =
                            if (sideEffect.systemVpnSettingsAvailable) {
                                resources.getString(R.string.go_to_vpn_settings)
                            } else {
                                null
                            },
                        withDismissAction = sideEffect.systemVpnSettingsAvailable,
                        onAction = {
                            context.openVpnSettings().onLeft {
                                launch {
                                    snackbarHostState.showSnackbarImmediately(
                                        message =
                                            resources.getString(R.string.vpn_settings_not_available)
                                    )
                                }
                            }
                        },
                    )
                }
            }

            is ConnectViewModel.UiSideEffect.OpenUri ->
                uriHandler.safeOpenUri(sideEffect.uri.toString()).onLeft {
                    snackbarHostState.showSnackbarImmediately(message = sideEffect.errorMessage)
                }
        }
    }

    // A pick made with no tunnel up comes back as a connect request: on desktop
    // choosing a location IS the connect gesture. Routing it through
    // onConnectButtonClick reuses this screen's VPN-consent gate rather than
    // duplicating it in the picker.
    LocalResultStore.current.consumeResult<ConnectAfterLocationPick> { onConnectButtonClick() }

    CompositionLocalProvider(LocalNavAnimatedVisibilityScope provides animatedVisibilityScope) {
        androidx.compose.foundation.layout.Box(modifier = Modifier.fillMaxSize()) {
            // Keyed, not copied on every recomposition: a fresh instance per
            // frame defeats skipping outright, since the state is compared by
            // identity once it reaches a composable that could skip.
            val uiState =
                remember(state, selectedLocationTitle) {
                    state.copy(selectedRelayItemTitle = selectedLocationTitle)
                }
            ConnectScreen(
                state = uiState,
                snackbarHostState = snackbarHostState,
                exitLoad =
                    connectedExitLoad?.let { load ->
                        { CompactExitLoad(load) }
                    },
                showBetaBadge = productFlags.isBeta,
                betaCapBps = betaCapBps,
                betaCapResolved = betaCapResolved,
                accountTimeLeft =
                    accountTimeLeftLabel(context, cachedExpiry, afterBeta = productFlags.isBeta),
                accountShortPubkey = fullPubkey?.shortWarrenAddress(),
                selectedCountryCode = selectedCountryCode,
                onCopyPubkey = fullPubkey?.let { full -> { copyPubkey(full, pubkeyCopiedMsg) } },
                onDisconnectClick = { warrenDisconnect.disconnect() },
                onConnectClick = onConnectButtonClick,
            // A shuffle with nothing to shuffle to is a button that silently
            // does nothing, so it stays disabled until the catalogue offers one.
            shuffleEnabled = shufflePicks.isNotEmpty(),
            onShuffleClick = onShuffleClick,
            onCancelClick = connectViewModel::onCancelClick,
            onSwitchLocationClick =
                // Switch location routes to the Warren picker (consumes
                // RelayCatalog).
                dropUnlessResumed {
                    navigator.navigate(WarrenLocationPickerNavKey(connectOnPick = true))
                },
            onOpenAppListing = connectViewModel::openAppListing,
            onManageAccountClick =
                // "Manage account" routes to the wallet settings; Warren
                // identity is the BIP39 wallet, not a server-side account.
                dropUnlessResumed { navigator.navigate(WarrenWalletSettingsNavKey) },
            onChangelogClick =
                dropUnlessResumed { navigator.navigate(ChangelogNavKey(isModal = true)) },
            onDismissChangelogClick = connectViewModel::dismissNewChangelogNotification,
            onSettingsClick =
                dropUnlessResumed {
                    if (navigator.screenIsListDetailTargetWidth) {
                        // Tablet detail-pane default for Settings is the
                        // unified Warren tunnel toggles screen.
                        navigator.navigate(SettingsNavKey, WarrenTunnelSettingsNavKey)
                    } else {
                        navigator.navigate(SettingsNavKey)
                    }
                },
            onAccountClick =
                dropUnlessResumed {
                    // The "account" surface is the wallet (BIP39 identity), so
                    // the account icon routes to the wallet settings.
                    navigator.navigate(WarrenWalletSettingsNavKey)
                },
            onNavigateToFeature =
                dropUnlessResumed { feature: FeatureIndicator ->
                    navigator.navigate(feature.navKey())
                },
            onClickDismissAndroid16UpgradeWarning =
                connectViewModel::dismissAndroid16UpgradeWarning,
            onClickShowAndroid16UpgradeInfo =
                dropUnlessResumed { navigator.navigate(Android16UpgradeInfoNavKey) },
            // The banner names the version it prompts for, so the dismissal is
            // recorded against that version and the next release prompts again.
            onClickDismissUpdateAvailable = {
                (uiState.inAppNotification as? InAppNotification.UpdateAvailable)?.let {
                    connectViewModel.dismissUpdateAvailable(it.version)
                }
            },
            onClickDismissExitSwitched = connectViewModel::acknowledgeExitSwitch,
            // The dismissal is recorded against the notice's own key, so a
            // later notice, or a rewrite of this one, raises the banner again.
            onClickDismissNotice = {
                when (val shown = uiState.inAppNotification) {
                    is InAppNotification.OperatorNotice ->
                        connectViewModel.dismissNotice(shown.notice.dismissalKey)
                    // Keyed on the strike, so the next one raises the banner.
                    is InAppNotification.AccountStrike ->
                        connectViewModel.dismissNotice(shown.notice.strike.dismissalKey)
                    else -> Unit
                }
            },
            // By announcement id, and for good: the card is an event rather
            // than a live statement, and it may already have handed over a
            // code.
            onClickDismissAnnouncement = {
                (uiState.inAppNotification as? InAppNotification.LaunchAnnouncement)?.let {
                    connectViewModel.dismissAnnouncement(it.announcement.id)
                }
            },
            onClickReEnableAfterStandDown = connectViewModel::reEnableAfterStandDown,
            onClickManageDevices = dropUnlessResumed { navigator.navigate(ManageDevicesNavKey) },
            forumSlot = forumSlot,
            onForumClick =
                dropUnlessResumed {
                    when (forumHeaderButton) {
                        ForumHeaderButton.ACTIVITY -> navigator.navigate(ForumActivityNavKey)
                        ForumHeaderButton.COMMUNITY -> uriHandler.openUri(forumUrl)
                        ForumHeaderButton.NONE -> Unit
                    }
                },
            )

            pubkeyMismatch?.let { mismatch ->
                val locationText =
                    relays.firstOrNull { it.exitId == mismatch.exitId }?.let { relay ->
                        val name = countryDisplayName(relay.country)
                        if (relay.city.isBlank()) name.ifBlank { null }
                        else stringResource(R.string.country_comma_city, name, relay.city)
                    }
                val saveFailedMessage = stringResource(R.string.warren_pubkey_warning_save_failed)
                val exitGoneMessage = stringResource(R.string.warren_pubkey_warning_exit_gone)
                val keyChangedAgainMessage =
                    stringResource(R.string.warren_pubkey_warning_key_changed_again)
                val trustFailedMessage = stringResource(R.string.warren_pubkey_warning_trust_failed)
                WarrenPubKeyWarningDialog(
                    exitIdHex = mismatch.exitId,
                    pinnedPubkeyHex = mismatch.pinnedPubkeyHex,
                    observedPubkeyHex = mismatch.observedPubkeyHex,
                    locationText = locationText,
                    busy = pubkeyBusy,
                    errorText = pubkeyError,
                    onTrust = {
                        val activity = context as? FragmentActivity
                        warrenScope.launch {
                            pubkeyBusy = true
                            pubkeyError = null
                            // Operator key rotation accepted: overwrite the pin
                            // with the newly observed key, then re-dispatch the
                            // connect. The dialog is dismissed only once the
                            // re-dispatch actually took.
                            val saved =
                                runCatching {
                                    localSettings.trustExitKey(
                                        mismatch.exitId,
                                        mismatch.observedPubkeyHex,
                                    )
                                }
                            val result =
                                if (saved.isSuccess && activity != null) {
                                    runCatching { warrenConnect.connect(activity) }.getOrNull()
                                } else {
                                    null
                                }
                            pubkeyBusy = false
                            when {
                                saved.isFailure -> pubkeyError = saveFailedMessage
                                result == WarrenConnectResult.Dispatched -> pubkeyMismatch = null
                                result is WarrenConnectResult.ExitKeyMismatch -> {
                                    pubkeyMismatch = result
                                    pubkeyError = keyChangedAgainMessage
                                }
                                relays.none { it.exitId == mismatch.exitId } ->
                                    pubkeyError = exitGoneMessage
                                else -> pubkeyError = trustFailedMessage
                            }
                        }
                    },
                    // Same outcome as rejecting (the connect was never
                    // dispatched, so there is no tunnel to drop), plus the
                    // signed report that tells the operator which key was
                    // served under this exit id. The dialog closes at once:
                    // the POST is best effort and the user has already
                    // decided, so making them wait on it would only turn a
                    // security answer into a spinner.
                    onReport = {
                        val location = relays.firstOrNull { it.exitId == mismatch.exitId }
                        warrenScope.launch {
                            incidentReporter.reportPubkeyMismatch(
                                exitIdHex = mismatch.exitId,
                                oldPubkeyHex = mismatch.pinnedPubkeyHex,
                                newPubkeyHex = mismatch.observedPubkeyHex,
                                countryCode = location?.country.orEmpty(),
                                city = location?.city.orEmpty(),
                            )
                        }
                        pubkeyMismatch = null
                        pubkeyError = null
                    },
                    // The connect was never dispatched on a mismatch, so rejecting
                    // just clears the pending prompt; there is no tunnel to drop.
                    onReject = {
                        pubkeyMismatch = null
                        pubkeyError = null
                    },
                )
            }
        }
    }
}

@Composable
@Suppress("LongParameterList", "LongMethod")
fun ConnectScreen(
    state: ConnectUiState,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    showBetaBadge: Boolean = false,
    betaCapBps: Long? = null,
    betaCapResolved: Boolean = false,
    accountTimeLeft: String? = null,
    accountShortPubkey: String? = null,
    onCopyPubkey: (() -> Unit)? = null,
    // The country the selection names, for the location row's flag while no tunnel is up.
    selectedCountryCode: String? = null,
    onDisconnectClick: () -> Unit,
    onConnectClick: () -> Unit,
    shuffleEnabled: Boolean = true,
    onShuffleClick: () -> Unit = {},
    onCancelClick: () -> Unit,
    onSwitchLocationClick: () -> Unit,
    onOpenAppListing: () -> Unit,
    onManageAccountClick: () -> Unit,
    onChangelogClick: () -> Unit,
    onDismissChangelogClick: () -> Unit,
    onSettingsClick: () -> Unit,
    onAccountClick: () -> Unit,
    onNavigateToFeature: (FeatureIndicator) -> Unit,
    onClickDismissAndroid16UpgradeWarning: () -> Unit,
    onClickShowAndroid16UpgradeInfo: () -> Unit,
    onClickDismissUpdateAvailable: () -> Unit = {},
    onClickDismissExitSwitched: () -> Unit = {},
    onClickDismissNotice: () -> Unit = {},
    onClickDismissAnnouncement: () -> Unit = {},
    onClickReEnableAfterStandDown: () -> Unit = {},
    onClickManageDevices: () -> Unit = {},
    // The desktop header's forum slot: the bell with its badge, the lifebuoy
    // for a wallet with no forum account, nothing when the setting is off.
    forumSlot: ForumHeaderSlot? = null,
    onForumClick: () -> Unit = {},
    // The load of the exit carrying the tunnel, closing the subtitle row; null while there is
    // none to show.
    exitLoad: (@Composable () -> Unit)? = null,
) {
    val contentFocusRequester = remember { FocusRequester() }
    val tv = isTv()

    val content =
        @Composable { showHeader: Boolean ->
            HomeContent(
                state = state,
                showHeader = showHeader,
                header = {
                    HomeHeader(
                        forumSlot = forumSlot,
                        onForumClick = onForumClick,
                        onAccountClick = onAccountClick,
                        onSettingsClick = onSettingsClick,
                    )
                },
                notifications = {
                    NotificationBanner(
                        modifier = Modifier.fillMaxWidth(),
                        notification = state.inAppNotification,
                        isPlayBuild = state.isPlayBuild,
                        contentFocusRequester = contentFocusRequester,
                        openAppListing = onOpenAppListing,
                        onClickShowAccount = onManageAccountClick,
                        onClickShowChangelog = onChangelogClick,
                        onClickShowAndroid16UpgradeInfo = onClickShowAndroid16UpgradeInfo,
                        onClickDismissChangelog = onDismissChangelogClick,
                        onClickDismissAndroid16UpgradeWarning =
                            onClickDismissAndroid16UpgradeWarning,
                        onClickDismissUpdateAvailable = onClickDismissUpdateAvailable,
                        onClickDismissExitSwitched = onClickDismissExitSwitched,
                        onClickDismissNotice = onClickDismissNotice,
                        onClickDismissAnnouncement = onClickDismissAnnouncement,
                        onClickReEnableAfterStandDown = onClickReEnableAfterStandDown,
                        onClickManageDevices = onClickManageDevices,
                    )
                },
                controls = {
                    val locationLine =
                        locationRowLine(
                            state.tunnelState,
                            state.location,
                            state.selectedRelayItemTitle,
                        )
                    HomeControls(
                        state = state,
                        locationName = locationLine?.text().orEmpty(),
                        flagCode = locationFlagCode(locationLine, selectedCountryCode),
                        focusRequester = contentFocusRequester,
                        shuffleEnabled = shuffleEnabled,
                        onSwitchLocationClick = onSwitchLocationClick,
                        onShuffleClick = onShuffleClick,
                        onConnectClick = onConnectClick,
                        onCancelClick = onCancelClick,
                        onDisconnectClick = onDisconnectClick,
                    )
                },
                chips = {
                    // Sorted by enum ordinal (desktop FeatureIndicators sorts by indicator
                    // value) so a feature always occupies the same slot.
                    val features = state.tunnelState.featureIndicators()?.sortedBy { it.ordinal }
                    if (!features.isNullOrEmpty()) {
                        AlwaysExpandedFeatureIndicators(
                            features = features,
                            multihopCircuit = state.tunnelState.isMultihopCircuit(),
                            onNavigateToFeature = onNavigateToFeature,
                        )
                    }
                },
                footer = {
                    HomeFooter(
                        shortPubkey = accountShortPubkey,
                        onCopyPubkey = onCopyPubkey,
                        timeLeft = accountTimeLeft,
                        beta =
                            if (showBetaBadge) {
                                {
                                    BetaBadge(
                                        capBps = betaCapBps,
                                        capResolved = betaCapResolved,
                                        variant = BetaBadgeVariant.Footer,
                                    )
                                }
                            } else {
                                null
                            },
                    )
                },
                exitLoad = exitLoad,
                onAppRoutingClick = { onNavigateToFeature(FeatureIndicator.SPLIT_TUNNELING) },
            )
        }

    if (tv) {
        Scaffold(
            snackbarHost = {
                SnackbarHost(
                    snackbarHostState,
                    snackbar = { snackbarData -> WarrenSnackbar(snackbarData = snackbarData) },
                )
            }
        ) {
            NavigationDrawerTv(
                onSettingsClick = onSettingsClick,
                onAccountClick = onAccountClick,
                forumItem =
                    when (forumSlot) {
                        is ForumHeaderSlot.Activity -> TvForumItem.ACTIVITY
                        ForumHeaderSlot.Community -> TvForumItem.COMMUNITY
                        null -> null
                    },
                onForumClick = onForumClick,
            ) {
                content(false)
            }
        }
        LaunchedEffect(Unit) { contentFocusRequester.requestFocus() }
    } else {
        Box(Modifier.fillMaxSize()) {
            content(true)
            SnackbarHost(
                snackbarHostState,
                modifier =
                    Modifier.align(Alignment.BottomCenter)
                        .navigationBarsPadding()
                        .padding(bottom = SNACKBAR_ABOVE_FOOTER),
                snackbar = { snackbarData -> WarrenSnackbar(snackbarData = snackbarData) },
            )
        }
    }
}

// Keeps a snackbar clear of the footer band it would otherwise cover.
private val SNACKBAR_ABOVE_FOOTER = 44.dp

// A column of controls wider than this reads as a form on a tablet; the hero keeps the width.
private val PANEL_MAX_WIDTH = 520.dp

// The side column of a landscape screen, where the panel cannot sit under the hero.
private val WIDE_PANEL_WIDTH = 400.dp

// The connection details open over the hero (desktop StyledDetailsSheet).
private val DETAILS_SHEET_MAX_HEIGHT = 300.dp
private val DETAILS_SHEET_RADIUS = 16.dp
private const val DETAILS_SHEET_MILLIS = 200
private val DETAILS_SHEET_SLIDE = 12.dp

// Notices float this far under the toolbar, on its right edge (mockup 8d).
private val NOTICES_GAP = 12.dp

/**
 * The home screen (desktop `MainView`): the scenery hero keeps the mockup's proportions, 300/360 of
 * its width tall under the status bar, with the header and the notices floating over it; the panel
 * takes the rest of the screen, one chip row reserved so the hero does not reframe when the tunnel
 * comes up, and the room left between the chips and the footer. A landscape screen puts the panel
 * in a side column instead, and the hero column's width sets its height.
 */
@Composable
@Suppress("LongParameterList", "LongMethod")
private fun HomeContent(
    state: ConnectUiState,
    showHeader: Boolean,
    header: @Composable () -> Unit,
    notifications: @Composable () -> Unit,
    controls: @Composable () -> Unit,
    chips: @Composable () -> Unit,
    footer: @Composable () -> Unit,
    exitLoad: (@Composable () -> Unit)?,
    onAppRoutingClick: () -> Unit,
) {
    val surfaces = LocalWarrenSurfaces.current
    val heroTokens = DesignTokens.Home.Hero
    val toolbar = DesignTokens.Home.Toolbar
    val panel = DesignTokens.Home.Panel
    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val headerTop = statusTop + heroTokens.HeaderInset
    val clearTop =
        if (showHeader) headerTop + toolbar.Height + heroTokens.ClearMargin
        else statusTop + heroTokens.ClearMargin
    val hasTunnel = state.tunnelState.isConnectingOrConnected()
    // Closed on every change of tunnel state, as on desktop: the sheet describes one connection.
    var detailsOpen by rememberSaveable(state.tunnelState::class) { mutableStateOf(false) }
    val sheetOpen = detailsOpen && hasTunnel
    BackHandler(enabled = sheetOpen) { detailsOpen = false }

    val phase = state.tunnelState.connectionPhase(state.hostOffline)
    val heroContent =
        @Composable {
            SceneryHero(
                phase = phase,
                exitCountry = state.location?.country,
                clearTop = clearTop,
                glowTop = statusTop,
                modifier = Modifier.fillMaxSize(),
            )
        }
    val statusBlock =
        @Composable {
            HomeStatusBlock(
                state = state,
                detailsOpen = sheetOpen,
                onToggleDetails = { detailsOpen = !detailsOpen },
                onAppRoutingClick = onAppRoutingClick,
                exitLoad = exitLoad,
            )
        }
    val panelContent =
        @Composable { modifier: Modifier ->
            Column(modifier.fillMaxWidth()) {
                Column(Modifier.padding(horizontal = panel.PaddingHorizontal)) {
                    statusBlock()
                    controls()
                    // One row is reserved even with no chip, so the hero keeps its frame when the
                    // tunnel comes up and the first chip appears.
                    Box(
                        Modifier.padding(top = DesignTokens.Home.Chips.MarginTop)
                            .heightIn(min = DesignTokens.Home.Chips.Height)
                    ) {
                        chips()
                    }
                }
            }
        }
    val sheet =
        @Composable { modifier: Modifier ->
            DetailsSheet(
                open = sheetOpen,
                state = state,
                modifier = modifier,
            )
        }

    BoxWithConstraints(Modifier.fillMaxSize().background(surfaces.homePage)) {
        val wide = maxWidth > maxHeight && !isTv()
        val sideWidth = minOf(WIDE_PANEL_WIDTH, maxWidth / 2)
        // A landscape screen puts the status bar's right half over the panel, which is dark in
        // the dark theme, so there the glyphs stay light.
        HomeSystemBarGlyphs(darkStatusGlyphs = !(wide && surfaces.isDark))
        if (wide) {
            Row(Modifier.fillMaxSize()) {
                BoxWithConstraints(Modifier.weight(1f).fillMaxHeight()) {
                    val heroHeight =
                        SceneryFraming.heroHeight(
                            width = maxWidth.value,
                            statusBar = statusTop.value,
                            available = maxHeight.value,
                        )
                    Box(Modifier.fillMaxWidth().height(heroHeight.dp)) { heroContent() }
                    sheet(Modifier.align(Alignment.BottomCenter))
                }
                Column(
                    Modifier.width(sideWidth)
                        .fillMaxHeight()
                        .windowInsetsPadding(
                            WindowInsets.safeDrawing.only(WindowInsetsSides.End)
                        )
                ) {
                    Box(
                        Modifier.weight(1f)
                            .verticalScroll(rememberScrollState())
                            .padding(top = clearTop)
                    ) {
                        panelContent(Modifier)
                    }
                    footer()
                }
            }
        } else {
            HomeColumnLayout(
                hero = heroContent,
                panel = {
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                        panelContent(Modifier.widthIn(max = PANEL_MAX_WIDTH))
                    }
                },
                sheet = {
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.BottomCenter) {
                        sheet(Modifier.widthIn(max = PANEL_MAX_WIDTH))
                    }
                },
                footer = footer,
                clearTop = clearTop,
                statusTop = statusTop,
            )
        }

        if (showHeader) {
            Box(
                Modifier.fillMaxWidth()
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
                    .padding(
                        top = headerTop,
                        start = heroTokens.HeaderInset,
                        end = heroTokens.HeaderInset,
                    )
            ) {
                header()
            }
        }
        // Transient notices float under the toolbar, over the art, so they never move the
        // controls; animateContentSize keeps a card arriving or leaving continuous.
        Box(
            Modifier.fillMaxWidth()
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
                .padding(
                    top =
                        if (showHeader) headerTop + toolbar.Height + NOTICES_GAP
                        else statusTop + NOTICES_GAP
                )
                .animateContentSize()
        ) {
            notifications()
        }
    }
}

/**
 * Lays the portrait home out: the hero at the mockup's ratio of the width under the status bar, the
 * panel riding the overlap up over its fade, the footer at the bottom, and the room left between
 * the panel and the footer, the mockup's spacer. Only a screen too short for all of it gives the
 * hero less, never the panel. The details sheet sits on the panel's top edge.
 */
@Composable
@Suppress("LongParameterList")
private fun HomeColumnLayout(
    hero: @Composable () -> Unit,
    panel: @Composable () -> Unit,
    sheet: @Composable () -> Unit,
    footer: @Composable () -> Unit,
    clearTop: Dp,
    statusTop: Dp,
) {
    Layout(contents = listOf(hero, panel, footer, sheet)) { measurables, constraints ->
        val (heroSlot, panelSlot, footerSlot, sheetSlot) = measurables
        val width = constraints.maxWidth
        val height = constraints.maxHeight
        val loose = constraints.copy(minWidth = width, maxWidth = width, minHeight = 0)
        val footerPlaceable = footerSlot.first().measure(loose)
        val panelPlaceable = panelSlot.first().measure(loose)
        val overlap = DesignTokens.Home.Panel.Overlap.roundToPx()
        val bottom = height - footerPlaceable.height
        val heroHeight =
            SceneryFraming.heroHeight(
                    width = width.toFloat(),
                    statusBar = statusTop.toPx(),
                    available = (bottom - panelPlaceable.height + overlap).toFloat(),
                )
                .roundToInt()
        val heroPlaceable = heroSlot.first().measure(Constraints.fixed(width, heroHeight))
        val sheetMax =
            minOf(
                DETAILS_SHEET_MAX_HEIGHT.roundToPx(),
                (heroHeight - overlap - clearTop.roundToPx()).coerceAtLeast(0),
            )
        val sheetPlaceable =
            sheetSlot.first().measure(loose.copy(maxHeight = sheetMax.coerceAtLeast(0)))
        layout(width, height) {
            heroPlaceable.place(0, 0)
            panelPlaceable.place(0, heroHeight - overlap)
            footerPlaceable.place(0, height - footerPlaceable.height)
            // On the panel's top edge, so the pill riding the overlap stays in view.
            sheetPlaceable.place(0, heroHeight - overlap - sheetPlaceable.height)
        }
    }
}

/**
 * The connection details, opened over the hero rather than pushing the controls (desktop
 * `HomeStatus` details sheet): the controls stay where the thumb expects them, the art gives way.
 */
@Composable
private fun DetailsSheet(open: Boolean, state: ConnectUiState, modifier: Modifier = Modifier) {
    val surfaces = LocalWarrenSurfaces.current
    val slide = with(LocalDensity.current) { DETAILS_SHEET_SLIDE.roundToPx() }
    AnimatedVisibility(
        visible = open,
        modifier = modifier,
        enter =
            fadeIn(tween(DETAILS_SHEET_MILLIS)) +
                slideInVertically(tween(DETAILS_SHEET_MILLIS)) { slide },
        exit =
            fadeOut(tween(DETAILS_SHEET_MILLIS)) +
                slideOutVertically(tween(DETAILS_SHEET_MILLIS)) { slide },
    ) {
        val shape =
            RoundedCornerShape(topStart = DETAILS_SHEET_RADIUS, topEnd = DETAILS_SHEET_RADIUS)
        val scrollState = rememberScrollState()
        Column(
            Modifier.fillMaxWidth()
                .dropShadow(
                    shape,
                    Shadow(
                        radius = DETAILS_SHEET_SHADOW_BLUR,
                        color = Color.Black.copy(alpha = DETAILS_SHEET_SHADOW_ALPHA),
                        spread = -DETAILS_SHEET_SHADOW_BLUR / 2,
                        offset = DpOffset(0.dp, -DETAILS_SHEET_SHADOW_BLUR / 2),
                    ),
                )
                .clip(shape)
                .background(surfaces.homePage)
                .padding(horizontal = DesignTokens.Home.Panel.PaddingHorizontal)
                .padding(top = DETAILS_SHEET_TOP_PADDING)
                .drawVerticalScrollbar(
                    scrollState,
                    color = surfaces.homeTextSecondary.copy(alpha = AlphaScrollbar),
                )
                .verticalScroll(scrollState)
        ) {
            ConnectionDetailPanel(
                state.tunnelState.toConnectionsDetails(state.location),
                enableSelectableText = !isTv(),
                autoRecoveryCount = state.autoRecoveryCount,
            )
        }
    }
}

private val DETAILS_SHEET_TOP_PADDING = 4.dp
private val DETAILS_SHEET_SHADOW_BLUR = 24.dp
private const val DETAILS_SHEET_SHADOW_ALPHA = 0.45f

/**
 * The status bar sits on the top of the sky in every landscape, which is pale (the scenery's top
 * fills are all near white) under a scrim of at most 40 %, so dark glyphs keep their contrast
 * there in both themes, as the inked logo beside them does. The navigation bar sits on the footer
 * band, which follows the theme. Both are restored to the app's light glyphs on leaving.
 */
@Composable
private fun HomeSystemBarGlyphs(darkStatusGlyphs: Boolean) {
    val view = LocalView.current
    val dark = LocalWarrenSurfaces.current.isDark
    DisposableEffect(view, dark, darkStatusGlyphs) {
        val window = (view.context as? Activity)?.window
        val controller = window?.let { WindowCompat.getInsetsController(it, view) }
        controller?.isAppearanceLightStatusBars = darkStatusGlyphs
        controller?.isAppearanceLightNavigationBars = !dark
        onDispose {
            controller?.isAppearanceLightStatusBars = false
            controller?.isAppearanceLightNavigationBars = false
        }
    }
}

/**
 * The remaining subscription time for the home footer ("29 days left"),
 * mirroring the desktop AppMainFooter time left. Hidden when there is no
 * subscription, when expired, and within the last 3 days (desktop closeToExpiry
 * threshold), where the in-app expiry banner surfaces the remaining time
 * instead.
 */
internal fun accountTimeLeftLabel(
    context: android.content.Context,
    expiryUnixSecs: Long,
    nowSecs: Long = System.currentTimeMillis() / 1000,
    afterBeta: Boolean = false,
): String? = when {
    expiryUnixSecs <= 0L -> null
    expiryUnixSecs - nowSecs <= 3L * 86_400 -> null
    else -> {
        val days = (expiryUnixSecs - nowSecs) / 86_400
        // Coarsest non-zero unit, like the desktop formatRemainingTime: a year
        // reads "1 year", not "400 days", and every unit goes through a plural
        // resource so ru/pl/ar can inflect it. Whole days and the same two
        // thresholds as the account card's RemainingTime, so the header and the
        // card never name different units for one expiry.
        val res = context.resources
        // After the beta cap the footer keeps the bare duration (mockup 8d), where "left" would
        // make the line run long.
        val (yearsRes, monthsRes, daysRes) =
            if (afterBeta) {
                Triple(
                    R.plurals.account_remaining_years,
                    R.plurals.account_remaining_months,
                    R.plurals.account_remaining_days,
                )
            } else {
                Triple(R.plurals.home_years_left, R.plurals.home_months_left, R.plurals.home_days_left)
            }
        when {
            days >= 365 -> {
                val years = (days / 365).toInt()
                res.getQuantityString(yearsRes, years, years)
            }
            days >= 30 -> {
                val months = (days / 30).toInt()
                res.getQuantityString(monthsRes, months, months)
            }
            else -> res.getQuantityString(daysRes, days.toInt(), days.toInt())
        }
    }
}

// A long value in the connection details does not fit a phone's width. The line
// scrolls its overflow instead of dying in an ellipsis, dwelling at each end like
// the desktop Marquee.
private const val MARQUEE_DWELL_MILLIS = 2000

internal fun Modifier.marqueeLine(): Modifier =
    this.basicMarquee(
        iterations = Int.MAX_VALUE,
        initialDelayMillis = MARQUEE_DWELL_MILLIS,
        repeatDelayMillis = MARQUEE_DWELL_MILLIS,
    )

/** The exit's load on the subtitle row: the ring, the percentage while live, the people. */
@Composable
private fun CompactExitLoad(load: ConnectedExitLoad) {
    val surfaces = LocalWarrenSurfaces.current
    ExitLoadBadge(
        badge = load.stats.loadBadgeOf(load.exit),
        
        showThroughput = false,
        stale = rememberSnapshotStale(load.stats),
        appearance =
            ExitLoadAppearance(
                textColor = surfaces.homeTextSecondary,
                textStyle = HomeTypography.exitLoad,
                ringPalette = homeLoadRingPalette(surfaces),
            ),
    )
}

// The exit country/city as a single line, used as the "Out" fallback when the
// egress IP is redacted (desktop `formatExitLocation`). Country is the raw code
// on the wire, rendered as its localized name.
@Composable
private fun GeoIpLocation?.exitLocationText(): String? {
    if (this == null) return null
    val name = countryDisplayName(country)
    if (name.isBlank()) return null
    val c = city
    return if (c.isNullOrBlank()) name else stringResource(R.string.country_comma_city, name, c)
}

data class ConnectionDetails(
    val multihop: Boolean,
    val inAddress: String,
    val outEndpoint: String?,
    val outIpv4Address: String?,
    val outIpv6Address: String?,
    val outFallback: String?,
)

@Composable
fun TunnelState.toConnectionsDetails(exitLocation: GeoIpLocation?): ConnectionDetails? {
    val endpoint =
        when (this) {
            is TunnelState.Connected -> endpoint
            is TunnelState.Connecting -> endpoint
            else -> null
        }

    if (endpoint == null) return null

    val outV4 = location()?.ipv4?.hostAddress
    val outV6 = location()?.ipv6?.hostAddress
    val multihop = isMultihopCircuit()
    // The exit socket the traffic egresses from, unless it is the unspecified
    // sentinel the engine publishes when it has no address to give.
    val exitEndpoint = endpoint.endpoint.takeUnless { it.isUnspecified() }?.toOutAddress()

    return ConnectionDetails(
        multihop = multihop,
        inAddress = endpoint.toInAddress(),
        outEndpoint = exitEndpoint,
        outIpv4Address = outV4,
        outIpv6Address = outV6,
        // Fallback only when no address of any kind is known (redacted multi-hop
        // exit); the exit country/city then identifies the Out, mirroring the
        // desktop `formatExitLocation` fallback so the row is never left blank.
        outFallback =
            if (outV4 == null && outV6 == null && exitEndpoint == null) {
                exitLocation.exitLocationText()
            } else {
                null
            },
    )
}

// The OS names no app for a legacy always-on VPN, so the unnamed variant is the
// only honest one: the named template would send the user hunting Android
// settings for an app called "Legacy app". Same string the system notification
// uses for the same cause.
private fun PrepareError.OtherLegacyAlwaysOnVpn.toMessage(resources: Resources) =
    resources.getString(R.string.legacy_always_on_vpn_error_notification_content).removeHtmlTags()

private fun PrepareError.OtherAlwaysOnApp.toMessage(resources: Resources) =
    resources.getString(R.string.always_on_vpn_error_notification_content, appName).removeHtmlTags()

private fun FeatureIndicator.navKey(): NavKey2 =
    when (this) {
        FeatureIndicator.DAITA,
        FeatureIndicator.DAITA_MULTIHOP -> WarrenDaitaSettingsNavKey
        FeatureIndicator.MULTIHOP,
        FeatureIndicator.MULTIHOP_CIRCUIT -> WarrenMultihopSettingsNavKey
        FeatureIndicator.SPLIT_TUNNELING -> SplitTunnelingNavKey(isModal = true)
        FeatureIndicator.APP_COUNTRIES ->
            SplitTunnelingNavKey(isModal = true, countryPerApp = true)
        FeatureIndicator.PORT_FORWARDING -> WarrenPortForwardingSettingsNavKey

        FeatureIndicator.SERVER_IP_OVERRIDE -> WarrenTunnelSettingsNavKey

        // Anti-censorship transport indicators route to the VPN settings
        // page; Warren uses a native Quinn obfuscation toggle.
        FeatureIndicator.UDP_2_TCP,
        FeatureIndicator.QUIC,
        FeatureIndicator.SHADOWSOCKS,
        FeatureIndicator.LWO -> WarrenTunnelSettingsNavKey

        FeatureIndicator.QUANTUM_RESISTANCE,
        FeatureIndicator.LAN_SHARING,
        FeatureIndicator.DNS_CONTENT_BLOCKERS,
        FeatureIndicator.CUSTOM_DNS,
        FeatureIndicator.REDUCED_MTU,
        FeatureIndicator.CUSTOM_MTU -> WarrenTunnelSettingsNavKey
    }

