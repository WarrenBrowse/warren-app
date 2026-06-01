package com.warrenbrowse.vpn.feature.home.impl.connect

import android.content.res.Resources
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.common.compose.LocalNavAnimatedVisibilityScope
import com.warrenbrowse.vpn.common.compose.SECURE_ZOOM
import com.warrenbrowse.vpn.common.compose.SECURE_ZOOM_ANIMATION_MILLIS
import com.warrenbrowse.vpn.common.compose.UNSECURE_ZOOM
import com.warrenbrowse.vpn.common.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.fallbackLatLong
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
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.FeatureIndicatorsPanel
import com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo.toInAddress
import com.warrenbrowse.vpn.feature.home.impl.connect.notificationbanner.NotificationBanner
import com.warrenbrowse.vpn.feature.settings.api.WarrenLocationPickerNavKey
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.splittunneling.api.SplitTunnelingNavKey
import androidx.fragment.app.FragmentActivity
import com.warrenbrowse.vpn.lib.common.util.CreateVpnProfile
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnReconnectInvoker
import com.warrenbrowse.vpn.lib.common.util.openVpnSettings
import com.warrenbrowse.vpn.lib.common.util.removeHtmlTags
import com.warrenbrowse.vpn.lib.map.AnimatedMap
import com.warrenbrowse.vpn.lib.map.data.GlobeColors
import com.warrenbrowse.vpn.lib.map.data.LocationMarkerColors
import com.warrenbrowse.vpn.lib.map.data.Marker
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.LatLong
import com.warrenbrowse.vpn.lib.model.Latitude
import com.warrenbrowse.vpn.lib.model.Longitude
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.tv.NavigationDrawerTv
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSnackbar
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_CARD_HEADER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.RECONNECT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SELECT_LOCATION_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.Shapes
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.util.visible
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

private const val CONNECT_BUTTON_THROTTLE_MILLIS = 1000
private val SCREEN_HEIGHT_THRESHOLD = 700.dp
private const val SHORT_SCREEN_INDICATOR_BIAS = 0.2f
private const val TALL_SCREEN_INDICATOR_BIAS = 0.3f

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
            onReconnectClick = {},
            onConnectClick = {},
            onCancelClick = {},
            onSwitchLocationClick = {},
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
    val localSettings = koinInject<WarrenLocalSettingsRepository>()
    val cachedExpiry by localSettings.cachedSubscriptionExpiry.collectAsStateWithLifecycle()
    val subscriptionWarning = remember(cachedExpiry) { connectExpiryWarning(cachedExpiry) }

    val state by connectViewModel.uiState.collectAsStateWithLifecycle()

    val context = LocalContext.current

    val warrenScope = rememberCoroutineScope()
    // Route the user-initiated Connect button through the Warren Quinn
    // use-case. The Quinn path requires a FragmentActivity host for
    // BiometricPrompt; the app's MainActivity extends FragmentActivity.
    val onWarrenConnectClick: () -> Unit = {
        (context as? FragmentActivity)?.let { activity ->
            warrenScope.launch {
                runCatching { warrenConnect.connect(activity) }
                    .onFailure { e -> co.touchlab.kermit.Logger.e(throwable = e) { "warren connect failed" } }
            }
        }
    }

    val snackbarHostState = remember { SnackbarHostState() }

    val createVpnProfile =
        rememberLauncherForActivityResult(CreateVpnProfile()) {
            connectViewModel.createVpnProfileResult(it)
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

    // WarrenLocationPicker does not surface a "connect now" hint today; the
    // user taps Connect on the home screen after picking. A future iteration
    // can add a "select-and-connect" affordance via a Warren-native NavResult.


    CompositionLocalProvider(LocalNavAnimatedVisibilityScope provides animatedVisibilityScope) {
        androidx.compose.foundation.layout.Box(modifier = Modifier.fillMaxSize()) {
            ConnectScreen(
                state = state,
                snackbarHostState = snackbarHostState,
                subscriptionWarning = subscriptionWarning,
                onDisconnectClick = { warrenDisconnect.disconnect() },
                onReconnectClick = { warrenReconnect.reconnect() },
                onConnectClick = onWarrenConnectClick,
            onCancelClick = connectViewModel::onCancelClick,
            onSwitchLocationClick =
                // Switch location routes to the Warren picker (consumes
                // RelayCatalog).
                dropUnlessResumed { navigator.navigate(WarrenLocationPickerNavKey) },
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
            )
        }
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
@Suppress("LongParameterList")
fun ConnectScreen(
    state: ConnectUiState,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    subscriptionWarning: String? = null,
    onDisconnectClick: () -> Unit,
    onReconnectClick: () -> Unit,
    onConnectClick: () -> Unit,
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
) {
    val contentFocusRequester = remember { FocusRequester() }

    val content =
        @Composable { padding: PaddingValues ->
            Content(
                contentFocusRequester,
                padding,
                state,
                subscriptionWarning,
                onDisconnectClick,
                onReconnectClick,
                onConnectClick,
                onCancelClick,
                onSwitchLocationClick,
                onOpenAppListing,
                onManageAccountClick,
                onChangelogClick,
                onDismissChangelogClick,
                onNavigateToFeature,
                onClickDismissAndroid16UpgradeWarning,
                onClickShowAndroid16UpgradeInfo,
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
            ) {
                content(it)
            }
        }
        LaunchedEffect(Unit) { contentFocusRequester.requestFocus() }
    } else {
        ScaffoldWithTopBar(
            topBarColor = state.tunnelState.topBarColor(),
            iconTintColor = state.tunnelState.iconTintColor(),
            onSettingsClicked = onSettingsClick,
            onAccountClicked = onAccountClick,
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
    subscriptionWarning: String?,
    onDisconnectClick: () -> Unit,
    onReconnectClick: () -> Unit,
    onConnectClick: () -> Unit,
    onCancelClick: () -> Unit,
    onSwitchLocationClick: () -> Unit,
    onOpenAppListing: () -> Unit,
    onManageAccountClick: () -> Unit,
    onChangelogClick: () -> Unit,
    onDismissChangelogClick: () -> Unit,
    onNavigateToFeature: (FeatureIndicator) -> Unit,
    onClickDismissAndroid16UpgradeWarning: () -> Unit,
    onClickShowAndroid16UpgradeInfo: () -> Unit,
) {
    val screenHeight =
        with(LocalDensity.current) { LocalWindowInfo.current.containerSize.height.toDp() }

    val indicatorPercentOffset =
        if (screenHeight < SCREEN_HEIGHT_THRESHOLD) SHORT_SCREEN_INDICATOR_BIAS
        else TALL_SCREEN_INDICATOR_BIAS

    Box(
        Modifier.padding(
                top = paddingValues.calculateTopPadding(),
                start = paddingValues.calculateStartPadding(LocalLayoutDirection.current),
                end = paddingValues.calculateEndPadding(LocalLayoutDirection.current),
            )
            .fillMaxSize()
    ) {
        WarrenMap(state, indicatorPercentOffset)

        WarrenCircularProgressIndicatorLarge(
            color = MaterialTheme.colorScheme.onSurface,
            modifier =
                Modifier.layout { measurable, constraints ->
                        val placeable = measurable.measure(constraints)
                        layout(placeable.width, placeable.height) {
                            placeable.placeRelative(
                                x = (constraints.maxWidth * 0.5f - placeable.width / 2).toInt(),
                                y =
                                    (constraints.maxHeight * indicatorPercentOffset -
                                            placeable.height / 2)
                                        .toInt(),
                            )
                        }
                    }
                    .visible(state.showLoading),
        )

        Box(
            modifier =
                Modifier.fillMaxSize().padding(bottom = paddingValues.calculateBottomPadding())
        ) {
            androidx.compose.foundation.layout.Column(
                modifier = Modifier.align(Alignment.TopCenter),
            ) {
                subscriptionWarning?.let { warning ->
                    SubscriptionWarningBar(
                        text = warning,
                        onClick = onManageAccountClick,
                    )
                }
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
                    onClickDismissAndroid16UpgradeWarning = onClickDismissAndroid16UpgradeWarning,
                )
            }
            ConnectionCard(
                state = state,
                modifier = Modifier.align(Alignment.BottomCenter),
                focusRequester = focusRequester,
                onSwitchLocationClick = onSwitchLocationClick,
                onDisconnectClick = onDisconnectClick,
                onReconnectClick = onReconnectClick,
                onCancelClick = onCancelClick,
                onConnectClick = onConnectClick,
                onNavigateToFeature = onNavigateToFeature,
            )
        }
    }
}

/**
 * A tappable warning bar shown at the top of the connect screen when the
 * cached subscription is expired or close to expiring. Tapping opens the
 * wallet/account surface to renew. Null-safe: only rendered when
 * [connectExpiryWarning] returns a message.
 */
@Composable
private fun SubscriptionWarningBar(text: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.errorContainer)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onErrorContainer,
        )
    }
}

/**
 * Compute the connect-screen subscription warning from the cached expiry, or
 * null when there is nothing to warn about (unknown, or comfortably active).
 * Only nags within a week of expiry so the bar doesn't show all the time.
 */
internal fun connectExpiryWarning(
    expiryUnixSecs: Long,
    nowSecs: Long = System.currentTimeMillis() / 1000,
): String? = when {
    expiryUnixSecs <= 0L -> null
    expiryUnixSecs <= nowSecs -> "Your subscription has expired. Tap to renew."
    expiryUnixSecs - nowSecs <= 7L * 86_400 -> {
        val days = ((expiryUnixSecs - nowSecs) + 86_399) / 86_400
        "Your subscription expires in $days day${if (days == 1L) "" else "s"}. Tap to renew."
    }
    else -> null
}

@Composable
private fun WarrenMap(state: ConnectUiState, progressIndicatorBias: Float) {

    // Distance to marker when secure/unsecure
    val baseZoom =
        animateFloatAsState(
            targetValue =
                if (state.tunnelState is TunnelState.Connected) SECURE_ZOOM else UNSECURE_ZOOM,
            animationSpec = tween(SECURE_ZOOM_ANIMATION_MILLIS),
            label = "baseZoom",
        )

    val markers = state.tunnelState.toMarker(state.location)?.let { listOf(it) } ?: emptyList()

    AnimatedMap(
        modifier = Modifier,
        cameraLocation = state.location?.toLatLong() ?: fallbackLatLong,
        cameraBaseZoom = baseZoom.value,
        cameraVerticalBias = progressIndicatorBias,
        markers = markers,
        globeColors =
            GlobeColors(
                landColor = MaterialTheme.colorScheme.primary,
                oceanColor = MaterialTheme.colorScheme.surface,
            ),
    )
}

@Composable
private fun ConnectionCard(
    state: ConnectUiState,
    modifier: Modifier = Modifier,
    focusRequester: FocusRequester,
    onSwitchLocationClick: () -> Unit,
    onDisconnectClick: () -> Unit,
    onReconnectClick: () -> Unit,
    onCancelClick: () -> Unit,
    onConnectClick: () -> Unit,
    onNavigateToFeature: (FeatureIndicator) -> Unit,
) {
    var expanded by
        rememberSaveable(state.tunnelState is TunnelState.Disconnected) { mutableStateOf(false) }
    val containerColor =
        animateColorAsState(
            if (expanded) MaterialTheme.colorScheme.secondaryContainer
            else MaterialTheme.colorScheme.secondaryContainer.copy(alpha = Alpha80),
            label = "connection_card_color",
        )

    Card(
        modifier =
            modifier.widthIn(max = Dimens.connectionCardMaxWidth).padding(Dimens.mediumPadding),
        Shapes.large,
        colors = CardDefaults.cardColors(containerColor = containerColor.value),
    ) {
        Column(modifier = Modifier.padding(all = Dimens.mediumPadding)) {
            ConnectionCardHeader(state, state.location, expanded) { expanded = !expanded }

            AnimatedContent(
                state.tunnelState.featureIndicators() to expanded,
                modifier = Modifier.weight(1f, fill = false),
                label = "connection_card_connection_details",
            ) { (featureIndicators, exp) ->
                if (featureIndicators != null) {
                    ConnectionInfo(
                        featureIndicators,
                        state.tunnelState.toConnectionsDetails(),
                        exp,
                        onToggleExpand = { expanded = !exp },
                        onNavigateToFeature = onNavigateToFeature,
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
                onReconnectClick,
                onCancelClick,
                onConnectClick,
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
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            ConnectionStatusText(state = state.tunnelState)
            if (state.tunnelState.isConnectingOrConnected()) {
                ExpandChevron(isExpanded = !expanded)
            }
        }

        Text(
            modifier = Modifier.fillMaxWidth().padding(top = Dimens.tinyPadding),
            text = location.asString(),
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        val hostnameText = location.hostnameText()
        AnimatedContent(hostnameText, label = "hostname") {
            if (it != null) {
                Text(
                    modifier = Modifier.fillMaxWidth(),
                    text = it,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun GeoIpLocation?.asString(): String {
    val city = this?.city
    return when {
        this == null -> ""
        city.isNullOrBlank() -> country
        else -> stringResource(R.string.country_comma_city, country, city)
    }
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

@Composable
private fun ConnectionInfo(
    featureIndicators: List<FeatureIndicator>,
    connectionDetails: ConnectionDetails?,
    expanded: Boolean,
    onToggleExpand: () -> Unit,
    onNavigateToFeature: (FeatureIndicator) -> Unit,
) {
    val scrollState = rememberScrollState()
    Column {
        if (expanded) {
            HorizontalDivider(
                Modifier.padding(vertical = Dimens.smallPadding),
                color = MaterialTheme.colorScheme.onPrimaryContainer.copy(Alpha20),
            )
        }
        Column(
            modifier =
                Modifier.fillMaxWidth()
                    .drawVerticalScrollbar(
                        scrollState,
                        color = MaterialTheme.colorScheme.onPrimary.copy(alpha = AlphaScrollbar),
                    )
                    .verticalScroll(scrollState)
        ) {
            FeatureIndicatorsPanel(featureIndicators, expanded, onToggleExpand, onNavigateToFeature)

            AnimatedVisibility(expanded && connectionDetails != null) {
                ConnectionDetailPanel(connectionDetails, enableSelectableText = !isTv())
            }
        }
    }
}

data class ConnectionDetails(
    val inAddress: String,
    val outIpv4Address: String?,
    val outIpv6Address: String?,
)

@Composable
fun TunnelState.toConnectionsDetails(): ConnectionDetails? {
    val endpoint =
        when (this) {
            is TunnelState.Connected -> endpoint
            is TunnelState.Connecting -> endpoint
            else -> null
        }

    if (endpoint == null) return null

    return ConnectionDetails(
        endpoint.toInAddress(),
        location()?.ipv4?.hostAddress,
        location()?.ipv6?.hostAddress,
    )
}

@Composable
private fun ButtonPanel(
    state: ConnectUiState,
    selectButtonFocusRequester: FocusRequester,
    onSwitchLocationClick: () -> Unit,
    onDisconnectClick: () -> Unit,
    onReconnectClick: () -> Unit,
    onCancelClick: () -> Unit,
    onConnectClick: () -> Unit,
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
            reconnectClick = {
                handleThrottledAction {
                    onReconnectClick()
                    selectButtonFocusRequester.requestFocus()
                }
            },
            isReconnectButtonEnabled =
                state.tunnelState is TunnelState.Connected ||
                    state.tunnelState is TunnelState.Connecting,
            modifier =
                Modifier.testTag(SELECT_LOCATION_BUTTON_TEST_TAG)
                    .focusRequester(selectButtonFocusRequester),
            reconnectButtonTestTag = RECONNECT_BUTTON_TEST_TAG,
        )
        Spacer(Modifier.height(Dimens.buttonSpacing))

        ConnectionButton(
            modifier = Modifier.fillMaxWidth().testTag(CONNECT_BUTTON_TEST_TAG),
            state = state.tunnelState,
            disconnectClick = onDisconnectClick,
            cancelClick = onCancelClick,
            connectClick = { handleThrottledAction(onConnectClick) },
        )
    }
}

@Composable
fun TunnelState.toMarker(location: GeoIpLocation?): Marker? {
    if (location == null) return null
    return when (this) {
        is TunnelState.Connected ->
            Marker(
                location.toLatLong(),
                colors = LocationMarkerColors(centerColor = MaterialTheme.colorScheme.positive),
            )

        is TunnelState.Connecting -> null
        is TunnelState.Disconnected ->
            Marker(
                location.toLatLong(),
                colors = LocationMarkerColors(centerColor = MaterialTheme.colorScheme.error),
            )

        is TunnelState.Disconnecting -> null
        is TunnelState.Error -> null
    }
}

@Composable
fun TunnelState.topBarColor(): Color =
    if (isSecured()) MaterialTheme.colorScheme.positive else MaterialTheme.colorScheme.error

@Composable
fun TunnelState.iconTintColor(): Color =
    if (isSecured()) {
        MaterialTheme.colorScheme.onTertiary
    } else {
        MaterialTheme.colorScheme.onError
    }

fun GeoIpLocation.toLatLong() =
    LatLong(Latitude(latitude.toFloat()), Longitude(longitude.toFloat()))

private fun PrepareError.OtherLegacyAlwaysOnVpn.toMessage(resources: Resources) =
    resources
        .getString(R.string.always_on_vpn_error_notification_content, "Legacy app")
        .removeHtmlTags()

private fun PrepareError.OtherAlwaysOnApp.toMessage(resources: Resources) =
    resources.getString(R.string.always_on_vpn_error_notification_content, appName).removeHtmlTags()

private fun FeatureIndicator.navKey(): NavKey2 =
    when (this) {
        // DAITA + Multihop indicators route to the unified Warren tunnel
        // settings screen.
        FeatureIndicator.DAITA,
        FeatureIndicator.DAITA_MULTIHOP,
        FeatureIndicator.MULTIHOP -> WarrenTunnelSettingsNavKey
        FeatureIndicator.SPLIT_TUNNELING -> SplitTunnelingNavKey(isModal = true)

        FeatureIndicator.SERVER_IP_OVERRIDE -> WarrenTunnelSettingsNavKey

        // Anti-censorship transport indicators route to the unified
        // WarrenTunnelSettings; Warren uses a native Quinn + M4.0 obfuscation
        // toggle.
        FeatureIndicator.UDP_2_TCP,
        FeatureIndicator.QUIC,
        FeatureIndicator.SHADOWSOCKS,
        FeatureIndicator.LWO -> WarrenTunnelSettingsNavKey

        // These chips route to WarrenTunnelSettings until Warren-native
        // equivalents land.
        FeatureIndicator.QUANTUM_RESISTANCE,
        FeatureIndicator.LAN_SHARING,
        FeatureIndicator.DNS_CONTENT_BLOCKERS,
        FeatureIndicator.CUSTOM_DNS,
        FeatureIndicator.CUSTOM_MTU -> WarrenTunnelSettingsNavKey
    }
