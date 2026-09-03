package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.core.view.WindowCompat
import androidx.compose.ui.platform.LocalView
import androidx.compose.runtime.DisposableEffect
import android.app.Activity
import android.content.res.Resources
import androidx.activity.compose.BackHandler
import androidx.activity.compose.ReportDrawn
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import kotlinx.coroutines.delay
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.foundation.background
import androidx.compose.material.icons.rounded.Check
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.common.compose.LocalNavAnimatedVisibilityScope
import com.warrenbrowse.vpn.common.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.isTv
import com.warrenbrowse.vpn.common.compose.safeOpenUri
import com.warrenbrowse.vpn.common.compose.showSnackbarImmediately
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.appinfo.api.ChangelogNavKey
import com.warrenbrowse.vpn.feature.home.api.Android16UpgradeInfoNavKey
import com.warrenbrowse.vpn.feature.home.api.DeviceRevokedNavKey
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.home.impl.connect.button.ConnectionButton
import com.warrenbrowse.vpn.feature.home.impl.connect.button.SwitchLocationButton
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.ConnectionDetailPanel
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.AlwaysExpandedFeatureIndicators
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.isUnspecified
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.toInAddress
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.toOutAddress
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.NotificationBanner
import com.warrenbrowse.vpn.feature.settings.api.ConnectAfterLocationPick
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenDaitaSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenMultihopSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenPortForwardingSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.splittunneling.api.SplitTunnelingNavKey
import androidx.fragment.app.FragmentActivity
import com.warrenbrowse.vpn.lib.common.util.CreateVpnProfile
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenConnectResult
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.repository.WarrenSubscriptionInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.common.util.openVpnSettings
import com.warrenbrowse.vpn.lib.common.util.removeHtmlTags
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.countryDisplayName
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.tv.NavigationDrawerTv
import com.warrenbrowse.vpn.lib.ui.component.BetaBadge
import com.warrenbrowse.vpn.lib.ui.component.BetaBadgeVariant
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.component.ForumHeaderSlot
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.model.forum.ForumHeaderButton
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.feature.settings.api.ForumActivityNavKey
import com.warrenbrowse.vpn.lib.tv.TvForumItem
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSnackbar
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_CARD_HEADER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SELECT_LOCATION_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SHUFFLE_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.util.visible
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

private const val CONNECT_BUTTON_THROTTLE_MILLIS = 1000

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
        when (forumHeaderButton) {
            ForumHeaderButton.ACTIVITY -> ForumHeaderSlot.Activity(forumUnread)
            ForumHeaderButton.COMMUNITY -> ForumHeaderSlot.Community
            ForumHeaderButton.NONE -> null
        }
    val forumUrl = stringResource(R.string.community_forum_url)

    val state by connectViewModel.uiState.collectAsStateWithLifecycle()
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

    // Selected-exit label for the Switch-location button (desktop UX): while
    // disconnected the button shows the chosen exit ("Automatic" when none is
    // pinned), and "Switch location" once connecting/connected.
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
    val selectedLocationTitle = if (state.tunnelState !is TunnelState.Disconnected) {
        null
    } else {
        // The pin can name any geographical depth, so the button shows the
        // depth the user actually chose rather than a resolved exit.
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
            ConnectViewModel.UiSideEffect.RevokedDevice ->
                navigator.navigate(DeviceRevokedNavKey, clearBackStack = true)

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
                showBetaBadge = productFlags.isBeta,
                betaCapBps = betaCapBps,
                betaCapResolved = betaCapResolved,
                accountTimeLeft = accountTimeLeftLabel(context, cachedExpiry),
                accountShortPubkey = fullPubkey?.shortWarrenAddress(),
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

@OptIn(ExperimentalComposeUiApi::class)
@Composable
@Suppress("LongParameterList")
fun ConnectScreen(
    state: ConnectUiState,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    showBetaBadge: Boolean = false,
    betaCapBps: Long? = null,
    betaCapResolved: Boolean = false,
    accountTimeLeft: String? = null,
    accountShortPubkey: String? = null,
    onCopyPubkey: (() -> Unit)? = null,
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
    // The desktop header's forum slot: the bell with its badge, the lifebuoy
    // for a wallet with no forum account, nothing when the setting is off.
    forumSlot: ForumHeaderSlot? = null,
    onForumClick: () -> Unit = {},
) {
    // The header paints its own glyphs black over the pale scenery sky; the
    // OS status bar right above it must follow (desktop header tone "dark"),
    // or a white clock sits on the same sky. Restored to the app-wide light
    // glyphs when the screen leaves composition.
    val view = LocalView.current
    DisposableEffect(view) {
        val window = (view.context as? Activity)?.window
        val controller = window?.let { WindowCompat.getInsetsController(it, view) }
        controller?.isAppearanceLightStatusBars = true
        onDispose { controller?.isAppearanceLightStatusBars = false }
    }
    val contentFocusRequester = remember { FocusRequester() }

    val content =
        @Composable { padding: PaddingValues ->
            Content(
                contentFocusRequester,
                padding,
                state,
                showBetaBadge,
                betaCapBps,
                betaCapResolved,
                accountShortPubkey,
                accountTimeLeft,
                onCopyPubkey,
                onDisconnectClick,
                onConnectClick,
                shuffleEnabled,
                onShuffleClick,
                onCancelClick,
                onSwitchLocationClick,
                onOpenAppListing,
                onManageAccountClick,
                onChangelogClick,
                onDismissChangelogClick,
                onNavigateToFeature,
                onClickDismissAndroid16UpgradeWarning,
                onClickShowAndroid16UpgradeInfo,
                onClickDismissUpdateAvailable,
                onClickDismissExitSwitched,
            )
        }

    if (isTv()) {
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
                content(it)
            }
        }
        LaunchedEffect(Unit) { contentFocusRequester.requestFocus() }
    } else {
        ScaffoldWithTopBar(
            // The header floats transparent over the scenery sky; identity and
            // time-left moved to the footer strip, matching the desktop
            // AppMainHeader variant="transparent" tone="dark".
            topBarColor = Color.Transparent,
            iconTintColor = Color.Black,
            onSettingsClicked = onSettingsClick,
            onAccountClicked = onAccountClick,
            forumSlot = forumSlot,
            onForumClicked = onForumClick,
            snackbarHostState = snackbarHostState,
        ) {
            content(it)
        }
    }
}

@Composable
@Suppress("LongParameterList")
private fun Content(
    focusRequester: FocusRequester,
    paddingValues: PaddingValues,
    state: ConnectUiState,
    showBetaBadge: Boolean,
    betaCapBps: Long?,
    betaCapResolved: Boolean,
    accountShortPubkey: String?,
    accountTimeLeft: String?,
    onCopyPubkey: (() -> Unit)?,
    onDisconnectClick: () -> Unit,
    onConnectClick: () -> Unit,
    shuffleEnabled: Boolean,
    onShuffleClick: () -> Unit,
    onCancelClick: () -> Unit,
    onSwitchLocationClick: () -> Unit,
    onOpenAppListing: () -> Unit,
    onManageAccountClick: () -> Unit,
    onChangelogClick: () -> Unit,
    onDismissChangelogClick: () -> Unit,
    onNavigateToFeature: (FeatureIndicator) -> Unit,
    onClickDismissAndroid16UpgradeWarning: () -> Unit,
    onClickShowAndroid16UpgradeInfo: () -> Unit,
    onClickDismissUpdateAvailable: () -> Unit,
    onClickDismissExitSwitched: () -> Unit,
) {
    // The card's top edge in root coordinates, fed to the backdrop at draw
    // time so the burrow foreground clears the card in every state, tracking
    // the card's own height animations frame by frame.
    var cardTopPx by remember { mutableFloatStateOf(Float.NaN) }
    Box(Modifier.fillMaxSize()) {
        // Full-bleed scenery behind the transparent header: the top padding is
        // deliberately not applied to the backdrop, only to the overlay UI.
        SceneryBackdrop(
            phase = state.tunnelState.connectionPhase(state.hostOffline),
            exitCountry = state.location?.country,
            modifier = Modifier.fillMaxSize(),
            cardTop = { cardTopPx },
        )

        Box(
            Modifier.padding(
                    top = paddingValues.calculateTopPadding(),
                    start = paddingValues.calculateStartPadding(LocalLayoutDirection.current),
                    end = paddingValues.calculateEndPadding(LocalLayoutDirection.current),
                )
                .fillMaxSize()
        ) {
            Box(
                modifier =
                    Modifier.fillMaxSize().padding(bottom = paddingValues.calculateBottomPadding())
            ) {
                // One top stack, in the desktop MainView order: the notification
                // card keeps the prime slot and the beta marker flows below it,
                // so a banner is never pushed down by the marker. The slot holds
                // exactly one card, expiry included, because a screen that can
                // stack strips is a screen where the most urgent one is not the
                // one at the top.
                // animateContentSize keeps the reflow continuous when a card
                // arrives or leaves, instead of shoving the rows below it.
                androidx.compose.foundation.layout.Column(
                    modifier = Modifier.align(Alignment.TopCenter).animateContentSize(),
                ) {
                    NotificationBanner(
                        modifier = Modifier.fillMaxWidth(),
                        notification = state.inAppNotification,
                        isPlayBuild = state.isPlayBuild,
                        contentFocusRequester = focusRequester,
                        openAppListing = onOpenAppListing,
                        onClickShowAccount = onManageAccountClick,
                        onClickShowChangelog = onChangelogClick,
                        onClickShowAndroid16UpgradeInfo = onClickShowAndroid16UpgradeInfo,
                        onClickDismissChangelog = onDismissChangelogClick,
                        onClickDismissAndroid16UpgradeWarning =
                            onClickDismissAndroid16UpgradeWarning,
                        onClickDismissUpdateAvailable = onClickDismissUpdateAvailable,
                        onClickDismissExitSwitched = onClickDismissExitSwitched,
                    )
                    if (showBetaBadge) {
                        // No vertical padding: the badge reserves its 48 dp touch
                        // row around a 32 dp pill, which already spaces it.
                        BetaBadge(
                            capBps = betaCapBps,
                            capResolved = betaCapResolved,
                            variant = BetaBadgeVariant.Overlay,
                            modifier = Modifier.padding(horizontal = Dimens.mediumPadding),
                        )
                    }
                }
                androidx.compose.foundation.layout.Column(
                    modifier = Modifier.align(Alignment.BottomCenter),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    // Active-feature chips float over the scenery ABOVE the
                    // card, like the desktop badge stack, not inside it. Sorted
                    // by enum ordinal (desktop FeatureIndicators sorts by
                    // indicator value) so a feature always occupies the same
                    // slot instead of shifting as others come and go.
                    val features =
                        state.tunnelState.featureIndicators()?.sortedBy { it.ordinal }
                    if (!features.isNullOrEmpty()) {
                        Box(
                            Modifier.widthIn(max = Dimens.connectionCardMaxWidth)
                                .fillMaxWidth()
                                .padding(horizontal = Dimens.mediumPadding)
                        ) {
                            AlwaysExpandedFeatureIndicators(
                                features = features,
                                onNavigateToFeature = onNavigateToFeature,
                            )
                        }
                    }
                    Box(
                        Modifier.onGloballyPositioned { cardTopPx = it.boundsInRoot().top }
                    ) {
                        ConnectionCard(
                            state = state,
                            focusRequester = focusRequester,
                            onSwitchLocationClick = onSwitchLocationClick,
                            onDisconnectClick = onDisconnectClick,
                            onCancelClick = onCancelClick,
                            onConnectClick = onConnectClick,
                            shuffleEnabled = shuffleEnabled,
                            onShuffleClick = onShuffleClick,
                        )
                    }
                    WarrenMainFooter(
                        shortPubkey = accountShortPubkey,
                        onCopyPubkey = onCopyPubkey,
                        timeLeft = accountTimeLeft,
                    )
                }
            }
        }
    }
}

/** Desktop AppMainHeaderPubKey: the check shows for two seconds after a copy. */
private const val FOOTER_COPIED_MILLIS = 2_000L

/**
 * Bottom footer strip over the scenery, mirroring the desktop AppMainFooter:
 * the short account pubkey (copyable, monospace) on the left and the remaining
 * subscription time on the right. Legibility comes from the backdrop's own
 * bottom scrim, which runs to the physical screen edge.
 */
@Composable
private fun WarrenMainFooter(
    shortPubkey: String?,
    onCopyPubkey: (() -> Unit)?,
    timeLeft: String?,
) {
    // Desktop AppMainFooter: a 60 % black band with a top hairline, 7 x 16
    // padding; the copy icon turns into a green check for two seconds. Keyed
    // on the tap count, not the flag, so a second tap inside the two seconds
    // restarts them the way the desktop reschedules its timer.
    var copyCount by remember { mutableIntStateOf(0) }
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copyCount) {
        if (copyCount > 0) {
            copied = true
            delay(FOOTER_COPIED_MILLIS)
            copied = false
        }
    }
    Column(modifier = Modifier.fillMaxWidth().background(Color.Black.copy(alpha = Alpha60))) {
        HorizontalDivider(thickness = 1.dp, color = Color.White.copy(alpha = Alpha20))
        Row(
            modifier =
                Modifier.fillMaxWidth()
                    .padding(
                        horizontal = Dimens.mediumPadding,
                        vertical = Dimens.footerVerticalPadding,
                    ),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (shortPubkey != null) {
                Text(
                    text = shortPubkey,
                    style = MaterialTheme.typography.labelMedium,
                    fontFamily = FontFamily.Monospace,
                    color = Color.White.copy(alpha = Alpha80),
                    maxLines = 1,
                )
                if (onCopyPubkey != null) {
                    IconButton(
                        onClick = {
                            onCopyPubkey()
                            copyCount++
                        },
                        modifier = Modifier.size(Dimens.mediumPadding * 2),
                    ) {
                        Icon(
                            imageVector =
                                if (copied) Icons.Rounded.Check else Icons.Rounded.ContentCopy,
                            // A screen reader gets the same confirmation the eye does.
                            contentDescription =
                                if (copied) stringResource(R.string.copied)
                                else stringResource(R.string.copy),
                            tint =
                                if (copied) MaterialTheme.colorScheme.positive
                                else Color.White.copy(alpha = Alpha80),
                            modifier = Modifier.size(Dimens.mediumPadding),
                        )
                    }
                }
            }
            Spacer(modifier = Modifier.weight(1f))
            if (timeLeft != null) {
                Text(
                    text = timeLeft,
                    style =
                        MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = Color.White.copy(alpha = Alpha80),
                    maxLines = 1,
                )
            }
        }
    }
}


/**
 * The remaining subscription time for the home header ("Time left: N days"),
 * mirroring the desktop AppMainHeaderTimeLeft. Hidden when there is no
 * subscription, when expired, and within the last 3 days (desktop closeToExpiry
 * threshold), where the in-app expiry banner surfaces the remaining time
 * instead.
 */
internal fun accountTimeLeftLabel(
    context: android.content.Context,
    expiryUnixSecs: Long,
    nowSecs: Long = System.currentTimeMillis() / 1000,
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
        val unit = when {
            days >= 365 -> {
                val years = (days / 365).toInt()
                res.getQuantityString(R.plurals.account_remaining_years, years, years)
            }
            days >= 30 -> {
                val months = (days / 30).toInt()
                res.getQuantityString(R.plurals.account_remaining_months, months, months)
            }
            else ->
                res.getQuantityString(R.plurals.account_remaining_days, days.toInt(), days.toInt())
        }
        context.getString(R.string.time_left_x, unit)
    }
}

// Desktop glass card: black at 50% (60% expanded) over the scenery with a
// hairline border, backdrop-blur approximated by the scrim alone.
private const val CARD_ALPHA_COLLAPSED = 0.5f
private const val CARD_ALPHA_EXPANDED = 0.6f

// Every geometry and tint change inside the card runs on the desktop
// ConnectionPanelAccordion clock (300ms ease-out), so the rule, the padding,
// the glass tint and the location block never settle on different curves.
private const val CARD_TRANSITION_MILLIS = 300

// The chevron's reserved slot, matching the Material icon's own 24dp box.
private val CHEVRON_SLOT_SIZE = 24.dp

// A long "Country, City" or an "<exit> via <entry>" hostname pair does not fit
// the card on a phone. The line scrolls its overflow instead of dying in an
// ellipsis, dwelling at each end like the desktop Marquee.
private const val MARQUEE_DWELL_MILLIS = 2000

internal fun Modifier.marqueeLine(): Modifier =
    this.basicMarquee(
        iterations = Int.MAX_VALUE,
        initialDelayMillis = MARQUEE_DWELL_MILLIS,
        repeatDelayMillis = MARQUEE_DWELL_MILLIS,
    )

@Composable
private fun ConnectionCard(
    state: ConnectUiState,
    modifier: Modifier = Modifier,
    focusRequester: FocusRequester,
    onSwitchLocationClick: () -> Unit,
    onDisconnectClick: () -> Unit,
    onCancelClick: () -> Unit,
    onConnectClick: () -> Unit,
    shuffleEnabled: Boolean,
    onShuffleClick: () -> Unit,
) {
    // The expansion survives connecting <-> connected (a reconnect must not
    // collapse the panel the user opened) and resets on the way out of them,
    // where the card has nothing left to expand and no chevron to collapse with.
    var expanded by
        rememberSaveable(state.tunnelState.isConnectingOrConnected()) { mutableStateOf(false) }
    // Back is the primary dismiss gesture on Android: it collapses the card
    // before it navigates away, and stays inert while the card is collapsed.
    BackHandler(enabled = expanded) { expanded = false }
    // Pinned to the same 300ms ease-out as the geometry around it (desktop
    // ConnectionPanel animates its tint on the accordion's own clock); the
    // default spring would settle the tint on a different curve.
    val containerColor =
        animateColorAsState(
            if (expanded) Color.Black.copy(alpha = CARD_ALPHA_EXPANDED)
            else Color.Black.copy(alpha = CARD_ALPHA_COLLAPSED),
            animationSpec = tween(CARD_TRANSITION_MILLIS, easing = LinearOutSlowInEasing),
            label = "connection_card_color",
        )

    Card(
        modifier =
            modifier.widthIn(max = Dimens.connectionCardMaxWidth).padding(Dimens.mediumPadding),
        shape = RoundedCornerShape(Dimens.mediumPadding),
        colors = CardDefaults.cardColors(containerColor = containerColor.value),
        border = BorderStroke(1.dp, Color.White.copy(alpha = Alpha20)),
    ) {
        Column(
            modifier =
                Modifier.padding(
                    vertical = Dimens.connectionCardVerticalPadding,
                    horizontal = Dimens.mediumPadding,
                )
        ) {
            ConnectionCardHeader(state, state.location, expanded) { expanded = !expanded }

            // The body is available in exactly the states that offer the
            // chevron, so the affordance can never open onto nothing.
            AnimatedContent(
                state.tunnelState.isConnectingOrConnected() to expanded,
                modifier = Modifier.weight(1f, fill = false),
                label = "connection_card_connection_details",
            ) { (hasTunnel, exp) ->
                if (hasTunnel) {
                    ConnectionInfo(
                        state.tunnelState.toConnectionsDetails(state.location),
                        exp,
                        autoRecoveryCount = state.autoRecoveryCount,
                    )
                } else {
                    Spacer(Modifier.height(Dimens.smallSpacer))
                }
            }

            Spacer(Modifier.height(Dimens.mediumPadding))

            ButtonPanel(
                state,
                focusRequester,
                onSwitchLocationClick,
                onDisconnectClick,
                onCancelClick,
                onConnectClick,
                shuffleEnabled,
                onShuffleClick,
            )
        }
    }
}

/**
 * The card's top row: status, the expand chevron and the country flag.
 *
 * The chevron keeps its slot in every state and only fades, so the flag owns
 * the top-right corner and never appears to move when the chevron comes and
 * goes (the desktop takes the chevron out of the flow for the same reason).
 */
@Composable
private fun ConnectionCardStatusRow(
    state: ConnectUiState,
    location: GeoIpLocation?,
    expanded: Boolean,
    hasTunnel: Boolean,
) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        ConnectionStatusText(
            state = state.tunnelState,
            hostOffline = state.hostOffline,
            modifier = Modifier.weight(1f),
        )
        val chevronAlpha by
            animateFloatAsState(
                targetValue = if (hasTunnel) 1f else 0f,
                animationSpec = tween(CARD_TRANSITION_MILLIS),
                label = "connection_card_chevron_alpha",
            )
        Box(Modifier.size(CHEVRON_SLOT_SIZE), contentAlignment = Alignment.Center) {
            if (chevronAlpha > 0f) {
                // The card grows upwards, so the arrow points up while
                // collapsed; the announcement still names what the tap does.
                ExpandChevron(
                    isExpanded = expanded,
                    pointsUpWhenCollapsed = true,
                    modifier = Modifier.alpha(chevronAlpha),
                )
            }
        }
        // While a tunnel exists the flag is the exit country; without one there
        // is no geoip source (Warren skips the conncheck), so the OS locale
        // region stands in for the user's own country, matching the desktop
        // CurrentCountryFlag (never the selected exit).
        val flagCountryCode =
            when (state.tunnelState) {
                is TunnelState.Connected,
                is TunnelState.Connecting,
                is TunnelState.Disconnecting -> location?.country
                else -> java.util.Locale.getDefault().country.ifBlank { null }
            }
        // The glyph changes at the same instant its source does, so it
        // cross-fades rather than popping into a different country.
        AnimatedContent(
            targetState = flagCountryCode,
            transitionSpec = {
                fadeIn(tween(CARD_TRANSITION_MILLIS)) togetherWith
                    fadeOut(tween(CARD_TRANSITION_MILLIS))
            },
            label = "connection_card_flag",
        ) { code ->
            CountryFlag(
                countryCode = code,
                modifier = Modifier.padding(start = Dimens.smallPadding),
            )
        }
    }
}

@Composable
private fun ConnectionCardHeader(
    state: ConnectUiState,
    location: GeoIpLocation?,
    expanded: Boolean,
    onToggleExpand: () -> Unit,
) {
    Column(
        modifier =
            Modifier.fillMaxWidth()
                .clickable(
                    enabled = state.tunnelState.isConnectingOrConnected(),
                    onClick = onToggleExpand,
                )
                .testTag(CONNECT_CARD_HEADER_TEST_TAG)
    ) {
        val hasTunnel = state.tunnelState.isConnectingOrConnected()
        ConnectionCardStatusRow(state, location, expanded, hasTunnel)

        // The exit location reads under the status only once a tunnel exists;
        // while disconnected the selector button already names the chosen exit
        // (desktop shows its Location line in the same states). The pair grows
        // and shrinks as one block on the desktop accordion's clock: the card is
        // bottom-anchored, so an unanimated appearance jumps the card, the chips
        // above it and the footer below it by two text lines at once.
        AnimatedVisibility(
            visible = hasTunnel,
            enter =
                expandVertically(tween(CARD_TRANSITION_MILLIS, easing = LinearOutSlowInEasing)) +
                    fadeIn(tween(CARD_TRANSITION_MILLIS)),
            exit =
                shrinkVertically(tween(CARD_TRANSITION_MILLIS, easing = LinearOutSlowInEasing)) +
                    fadeOut(tween(CARD_TRANSITION_MILLIS)),
        ) {
            Column {
                Text(
                    modifier =
                        Modifier.fillMaxWidth().padding(top = Dimens.tinyPadding).marqueeLine(),
                    text = location.asString(),
                    // Desktop Location: 18/24 semibold.
                    style =
                        MaterialTheme.typography.titleMedium.copy(
                            fontSize = 18.sp,
                            lineHeight = 24.sp,
                        ),
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                )
                val hostnameText = location.hostnameText()
                AnimatedContent(hostnameText, label = "hostname") {
                    if (it != null) {
                        Text(
                            modifier = Modifier.fillMaxWidth().marqueeLine(),
                            text = it,
                            // Desktop Hostname: 14/20 at 60 % white.
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                            maxLines = 1,
                        )
                    }
                }
            }
        }
    }
}

/**
 * Round country flag pinned to the right of the status row (desktop
 * CurrentCountryFlag). Rendered as the emoji flag of the ISO alpha-2 code so
 * no per-country assets are needed; hidden when the code is unknown.
 */
@Composable
private fun CountryFlag(countryCode: String?, modifier: Modifier = Modifier) {
    val emoji = countryCode?.toFlagEmoji() ?: return
    Text(text = emoji, fontSize = 22.sp, modifier = modifier)
}

private fun String.toFlagEmoji(): String? {
    val code = trim().uppercase()
    if (code.length != 2 || !code.all { it in 'A'..'Z' }) return null
    val base = 0x1F1E6
    return String(Character.toChars(base + (code[0] - 'A'))) +
        String(Character.toChars(base + (code[1] - 'A')))
}

@Composable
private fun GeoIpLocation?.asString(): String {
    val city = this?.city
    return when {
        this == null -> ""
        // country is the raw ISO code on the wire; show the localized name.
        city.isNullOrBlank() -> countryDisplayName(country)
        else -> stringResource(R.string.country_comma_city, countryDisplayName(country), city)
    }
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

@Composable
private fun GeoIpLocation?.hostnameText(): String? {
    val entryHostname = this?.entryHostname
    val exitHostname = this?.hostname
    return when {
        entryHostname != null && exitHostname != null ->
            stringResource(R.string.x_via_x, exitHostname, entryHostname)
        else -> exitHostname
    }
}

// The active-feature chips live ABOVE the card (desktop badge stack), so the
// card body only holds the expandable connection details.
@Composable
private fun ConnectionInfo(
    connectionDetails: ConnectionDetails?,
    expanded: Boolean,
    autoRecoveryCount: Int = 0,
) {
    val scrollState = rememberScrollState()
    Column(
        modifier =
            Modifier.fillMaxWidth()
                .drawVerticalScrollbar(
                    scrollState,
                    color = MaterialTheme.colorScheme.onPrimary.copy(alpha = AlphaScrollbar),
                )
                .verticalScroll(scrollState)
    ) {
        // The panel is drawn on expansion alone: its heading, protocol,
        // Reconnects and Obfuscation rows hold whatever the tunnel state, so
        // the chevron offered while connecting never opens onto nothing. The
        // In/Out grid stays blank until an endpoint is known.
        //
        // The rule and the padding it needs are INSIDE the animation: rendered
        // on a bare `if (expanded)` they snapped into existence while the panel
        // below them was still opening.
        AnimatedVisibility(
            visible = expanded,
            enter =
                expandVertically(tween(CARD_TRANSITION_MILLIS, easing = LinearOutSlowInEasing)) +
                    fadeIn(tween(CARD_TRANSITION_MILLIS)),
            exit =
                shrinkVertically(tween(CARD_TRANSITION_MILLIS, easing = LinearOutSlowInEasing)) +
                    fadeOut(tween(CARD_TRANSITION_MILLIS)),
        ) {
            Column {
                HorizontalDivider(
                    Modifier.padding(vertical = Dimens.smallPadding),
                    color = MaterialTheme.colorScheme.onPrimaryContainer.copy(Alpha20),
                )
                ConnectionDetailPanel(
                    connectionDetails,
                    enableSelectableText = !isTv(),
                    autoRecoveryCount = autoRecoveryCount,
                )
            }
        }
    }
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
    val multihop =
        featureIndicators()?.any {
            it == FeatureIndicator.MULTIHOP || it == FeatureIndicator.DAITA_MULTIHOP
        } ?: false
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

@Composable
@Suppress("LongParameterList")
private fun ButtonPanel(
    state: ConnectUiState,
    selectButtonFocusRequester: FocusRequester,
    onSwitchLocationClick: () -> Unit,
    onDisconnectClick: () -> Unit,
    onCancelClick: () -> Unit,
    onConnectClick: () -> Unit,
    shuffleEnabled: Boolean,
    onShuffleClick: () -> Unit,
) {
    var lastConnectionActionTimestamp by remember { mutableLongStateOf(0L) }

    fun handleThrottledAction(action: () -> Unit) {
        val currentTime = System.currentTimeMillis()
        if ((currentTime - lastConnectionActionTimestamp) > CONNECT_BUTTON_THROTTLE_MILLIS) {
            lastConnectionActionTimestamp = currentTime
            action.invoke()
        }
    }
    Column(modifier = Modifier.padding(top = Dimens.tinyPadding)) {
        SwitchLocationButton(
            text = state.selectedRelayItemTitle ?: stringResource(id = R.string.switch_location),
            onSwitchLocation = onSwitchLocationClick,
            onShuffle = { handleThrottledAction(onShuffleClick) },
            shuffleEnabled = shuffleEnabled,
            modifier =
                Modifier.testTag(SELECT_LOCATION_BUTTON_TEST_TAG)
                    .focusRequester(selectButtonFocusRequester),
            shuffleButtonTestTag = SHUFFLE_BUTTON_TEST_TAG,
        )
        Spacer(Modifier.height(Dimens.buttonSpacing))

        ConnectionButton(
            modifier = Modifier.fillMaxWidth().testTag(CONNECT_BUTTON_TEST_TAG),
            state = state.tunnelState,
            // Every tunnel action shares the throttle: a Disconnect-then-Connect
            // double tap would otherwise dispatch a connect into a teardown that
            // is still in flight.
            disconnectClick = { handleThrottledAction(onDisconnectClick) },
            cancelClick = { handleThrottledAction(onCancelClick) },
            connectClick = { handleThrottledAction(onConnectClick) },
        )
    }
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
        FeatureIndicator.MULTIHOP -> WarrenMultihopSettingsNavKey
        FeatureIndicator.SPLIT_TUNNELING -> SplitTunnelingNavKey(isModal = true)
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
