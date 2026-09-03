package com.warrenbrowse.vpn.feature.settings.impl

import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.assureHasDetailPane
import com.warrenbrowse.vpn.common.compose.createUriHook
import com.warrenbrowse.vpn.common.compose.itemWithDivider
import com.warrenbrowse.vpn.common.compose.navigateReplaceIfDetailPane
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.appinfo.api.AppInfoNavKey
import com.warrenbrowse.vpn.feature.language.api.LanguageNavKey
import com.warrenbrowse.vpn.feature.notification.api.NotificationSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.ForumSignInCodeNavKey
import com.warrenbrowse.vpn.feature.settings.api.ReportProblemNavKey
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenDaitaSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenMultihopSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenPortForwardingSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.splittunneling.api.SplitTunnelingNavKey
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.util.appendHideNavOnPlayBuild
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNetworkInfoProvider
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.ui.component.BetaBadge
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateCloseIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.ExternalLinkListItem
import com.warrenbrowse.vpn.lib.ui.component.listitem.NavigationListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.DAITA_CELL_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.LAZY_LIST_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.MULTIHOP_CELL_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.VPN_SETTINGS_CELL_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

@OptIn(ExperimentalMaterial3Api::class)
@Preview("Loading|Supported|+")
@Composable
private fun PreviewSettingsScreen(
    @PreviewParameter(SettingsUiStatePreviewParameterProvider::class)
    state: Lc<Unit, SettingsUiState>
) {
    AppTheme {
        SettingsScreen(
            state = state,
            onSplitTunnelingCellClick = {},
            onAppInfoClick = {},
            onMultihopClick = {},
            onDaitaClick = {},
            onPortForwardingClick = {},
            onVpnSettingsClick = {},
            onBackClick = {},
            onNotificationSettingsCellClick = {},
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Settings(navigator: Navigator) {
    val vm = koinViewModel<SettingsViewModel>()
    val state by vm.uiState.collectAsStateWithLifecycle()
    // Beta program surfaces: badge gated on the COMPILED flavor; the
    // network-info feed supplies the live cap figure.
    val productFlags = koinInject<WarrenProductFlags>()
    val networkInfoProvider = koinInject<WarrenNetworkInfoProvider>()
    val networkInfo by networkInfoProvider.networkInfo.collectAsStateWithLifecycle()
    // The badge holds its note until the cap is known (live feed or the cached
    // last answer), so a cold start never shows one wording and swaps it.
    val localSettings = koinInject<WarrenLocalSettingsRepository>()
    val cachedRateBps by localSettings.cachedNetworkRateBps.collectAsStateWithLifecycle()

    BackHandler(enabled = navigator.screenIsListDetailTargetWidth) {
        navigator.goBackUntil(SettingsNavKey, inclusive = true)
    }

    // Tablet detail-pane default is the VPN settings page.
    navigator.assureHasDetailPane<SettingsNavKey>(WarrenTunnelSettingsNavKey)

    SettingsScreen(
        state = state,
        showBetaBadge = productFlags.isBeta,
        betaCapBps = if (networkInfo != null) networkInfo?.defaultRateBps else cachedRateBps,
        betaCapResolved = networkInfo != null || cachedRateBps != null,
        onSplitTunnelingCellClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(SplitTunnelingNavKey()) },
        onAppInfoClick = dropUnlessResumed { navigator.navigateReplaceIfDetailPane(AppInfoNavKey) },
        // Each tunnel responsibility has its own dedicated page, mirroring the
        // desktop settings root (DAITA, Multihop, Port forwarding, VPN settings).
        onDaitaClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenDaitaSettingsNavKey) },
        onMultihopClick =
            dropUnlessResumed {
                navigator.navigateReplaceIfDetailPane(WarrenMultihopSettingsNavKey)
            },
        onPortForwardingClick =
            dropUnlessResumed {
                navigator.navigateReplaceIfDetailPane(WarrenPortForwardingSettingsNavKey)
            },
        onVpnSettingsClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenTunnelSettingsNavKey) },
        onNotificationSettingsCellClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(NotificationSettingsNavKey) },
        // Language and Notifications form desktop's "User interface settings"
        // group, kept as a flat group here instead of a container page (the
        // two rows do not earn a passthrough on mobile). Per-app language
        // requires API 33, so the row is hidden below that.
        onLanguageClick =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                dropUnlessResumed { navigator.navigateReplaceIfDetailPane(LanguageNavKey) }
            } else {
                null
            },
        onWalletClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenWalletSettingsNavKey) },
        onReportProblemClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(ReportProblemNavKey) },
        onForumSignInCodeClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(ForumSignInCodeNavKey) },
        onReplayOnboardingClick = dropUnlessResumed { replayOnboarding(localSettings, navigator) },
        onBackClick = dropUnlessResumed { navigator.goBackUntil(SettingsNavKey, inclusive = true) },
    )
}

@Composable
fun SettingsScreen(
    state: Lc<Unit, SettingsUiState>,
    showBetaBadge: Boolean = false,
    betaCapBps: Long? = null,
    betaCapResolved: Boolean = false,
    onSplitTunnelingCellClick: () -> Unit,
    onAppInfoClick: () -> Unit,
    onMultihopClick: () -> Unit,
    onDaitaClick: () -> Unit,
    onPortForwardingClick: () -> Unit,
    onVpnSettingsClick: () -> Unit,
    onBackClick: () -> Unit,
    onNotificationSettingsCellClick: () -> Unit,
    onLanguageClick: (() -> Unit)? = null,
    onWalletClick: () -> Unit = {},
    onReportProblemClick: () -> Unit = {},
    onForumSignInCodeClick: () -> Unit = {},
    onReplayOnboardingClick: () -> Unit = {},
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.settings),
        navigationIcon = { NavigateCloseIconButton(onBackClick) },
    ) { modifier ->
        val lazyListState = rememberLazyListState()
        LazyColumn(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier =
                modifier
                    .drawVerticalScrollbar(
                        state = lazyListState,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                    )
                    .testTag(LAZY_LIST_TEST_TAG)
                    .padding(horizontal = Dimens.sideMarginNew)
                    .animateContentSize(),
            state = lazyListState,
        ) {
            when (state) {
                is Lc.Loading -> loading()
                is Lc.Content -> {
                    content(
                        state = state.value,
                        showBetaBadge = showBetaBadge,
                        betaCapBps = betaCapBps,
                        betaCapResolved = betaCapResolved,
                        onSplitTunnelingCellClick = onSplitTunnelingCellClick,
                        onAppInfoClick = onAppInfoClick,
                        onMultihopClick = onMultihopClick,
                        onDaitaClick = onDaitaClick,
                        onPortForwardingClick = onPortForwardingClick,
                        onVpnSettingsClick = onVpnSettingsClick,
                        onNotificationSettingsCellClick = onNotificationSettingsCellClick,
                        onLanguageClick = onLanguageClick,
                        onWalletClick = onWalletClick,
                        onReportProblemClick = onReportProblemClick,
                        onForumSignInCodeClick = onForumSignInCodeClick,
                        onReplayOnboardingClick = onReplayOnboardingClick,
                    )
                }
            }
        }
    }
}

private fun LazyListScope.content(
    state: SettingsUiState,
    showBetaBadge: Boolean = false,
    betaCapBps: Long? = null,
    betaCapResolved: Boolean = false,
    onSplitTunnelingCellClick: () -> Unit,
    onAppInfoClick: () -> Unit,
    onMultihopClick: () -> Unit,
    onDaitaClick: () -> Unit,
    onPortForwardingClick: () -> Unit,
    onVpnSettingsClick: () -> Unit,
    onNotificationSettingsCellClick: () -> Unit,
    onLanguageClick: (() -> Unit)? = null,
    onWalletClick: () -> Unit = {},
    onReportProblemClick: () -> Unit = {},
    onForumSignInCodeClick: () -> Unit = {},
    onReplayOnboardingClick: () -> Unit = {},
) {
    if (showBetaBadge) {
        item {
            BetaBadge(
                capBps = betaCapBps,
                capResolved = betaCapResolved,
                modifier = Modifier.padding(bottom = Dimens.smallPadding),
            )
        }
    }

    if (state.isLoggedIn) {
        // Tunnel pages, in the desktop settings-root order: DAITA, Multihop,
        // Port forwarding, VPN settings. Each opens its own dedicated page,
        // and the list opens on this block as it does on desktop.
        itemWithDivider {
            DaitaListItem(isDaitaEnabled = state.isDaitaEnabled, onDaitaClick = onDaitaClick)
        }
        itemWithDivider {
            MultihopCell(isEnabled = state.isMultiHopEnabled, onMultihopClick = onMultihopClick)
        }
        itemWithDivider {
            PortForwardingCell(
                isEnabled = state.isPortForwardingEnabled,
                onPortForwardingClick = onPortForwardingClick,
            )
        }
        itemWithDivider { VpnSettingsCell(onVpnSettingsClick = onVpnSettingsClick) }
        item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }
        // Account keeps a settings-root entry (a prominent account row is a
        // mobile convention and the home screen carries less chrome than the
        // desktop main view), but grouped and below the tunnel block so the
        // page still opens on DAITA. Split tunneling shares the group: both
        // scope the tunnel to this device rather than configuring it.
        itemWithDivider { Account(onWalletClick) }
        item { SplitTunneling(onSplitTunnelingCellClick) }
        item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }
    }

    // Desktop's "User interface settings" container, kept as its own group
    // directly after the tunnel block rather than dissolved into App info.
    if (onLanguageClick != null) {
        itemWithDivider {
            NavigationListItem(
                title = stringResource(id = R.string.language),
                onClick = onLanguageClick,
                position = Position.Top,
            )
        }
    }

    item {
        NavigationListItem(
            title = stringResource(id = R.string.settings_notifications),
            onClick = onNotificationSettingsCellClick,
            position = if (onLanguageClick != null) Position.Bottom else Position.Single,
        )
    }

    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }

    item { AppInfo(onAppInfoClick, state) }

    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }

    // Desktop's Support page order: Community forum, FAQs & Guides, Terms of
    // use, Refund policy. Privacy policy is a Warren-specific extra and stays
    // last so the shared rows keep their desktop positions.
    itemWithDivider { CommunityForum() }

    // The two doors that need no browser round trip: file a report with the
    // logs straight from the app, and finish a forum sign-in from the code
    // the approval page shows when the deep link went nowhere.
    itemWithDivider { ReportProblem(onReportProblemClick) }

    itemWithDivider { ForumSignInCode(onForumSignInCodeClick) }

    if (!state.isPlayBuild) {
        itemWithDivider { FaqAndGuides() }
    }

    itemWithDivider { TermsOfUse() }

    itemWithDivider { RefundPolicy() }

    itemWithDivider { PrivacyPolicy(state) }

    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }

    // Desktop settings root: the wizard replay sits on its own after the
    // support and app-info block.
    item { ReplayOnboarding(onReplayOnboardingClick) }

    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }
}

@Composable
private fun ReplayOnboarding(onClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.settings_replay_onboarding),
        onClick = onClick,
        position = Position.Single,
    )
}

@Composable
private fun Account(onWalletClick: () -> Unit) {
    // The row and the page it opens carry the same word: the destination's app
    // bar reads "Account", so labelling the row "Wallet" left the two
    // contradicting each other.
    NavigationListItem(
        title = stringResource(id = R.string.settings_account),
        onClick = onWalletClick,
        position = Position.Top,
    )
}

@Composable
private fun SplitTunneling(onSplitTunnelingCellClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.split_tunneling),
        onClick = onSplitTunnelingCellClick,
        position = Position.Bottom,
    )
}

@Composable
private fun AppInfo(navigateToAppInfo: () -> Unit, state: SettingsUiState) {
    // A pending update is visible from the root, as on desktop: the version
    // stays the subtitle and the upgrade is called out with its own line plus
    // the warning dot, so an outdated app no longer looks up to date.
    val upgrade = state.availableUpgrade
    NavigationListItem(
        title = stringResource(id = R.string.app_info),
        subtitle =
            if (upgrade != null) {
                state.appVersion +
                    "\n" +
                    stringResource(R.string.app_info_update_available, upgrade)
            } else {
                state.appVersion
            },
        // The version alone is forced LTR so its dotted number never reorders.
        // Once a translated sentence joins it the paragraph must follow the
        // locale instead, or an RTL update line would be laid out backwards.
        subTitleTextDirection =
            if (upgrade != null) TextDirection.Unspecified else TextDirection.Ltr,
        showWarning = !state.isSupportedVersion || upgrade != null,
        position = Position.Single,
        onClick = navigateToAppInfo,
    )
}

@Composable
private fun FaqAndGuides() {
    val faqGuideLabel = stringResource(id = R.string.faqs_and_guides)
    val openFaqAndGuides =
        LocalUriHandler.current.createUriHook(stringResource(R.string.faqs_and_guides_url))

    ExternalLinkListItem(
        title = faqGuideLabel,
        onClick = openFaqAndGuides,
        position = Position.Middle,
    )
}

@Composable
private fun TermsOfUse() {
    // The one URL that always resolves to the contract the user is actually
    // under; without this row Android had no in-app path to it at all.
    val label = stringResource(id = R.string.terms_of_use)
    val openTerms = LocalUriHandler.current.createUriHook(stringResource(R.string.terms_of_use_url))

    ExternalLinkListItem(title = label, onClick = openTerms, position = Position.Middle)
}

@Composable
private fun PrivacyPolicy(state: SettingsUiState) {
    val privacyPolicyLabel = stringResource(id = R.string.privacy_policy_label)

    val openPrivacyPolicy =
        LocalUriHandler.current.createUriHook(
            stringResource(R.string.privacy_policy_url).appendHideNavOnPlayBuild(state.isPlayBuild)
        )

    ExternalLinkListItem(
        title = privacyPolicyLabel,
        onClick = openPrivacyPolicy,
        position = Position.Bottom,
    )
}

@Composable
private fun RefundPolicy() {
    val label = stringResource(id = R.string.refund_policy_label)
    val openRefundPolicy =
        LocalUriHandler.current.createUriHook(stringResource(R.string.refund_policy_url))

    ExternalLinkListItem(title = label, onClick = openRefundPolicy, position = Position.Middle)
}

@Composable
private fun CommunityForum() {
    val label = stringResource(id = R.string.community_forum)
    val openForum =
        LocalUriHandler.current.createUriHook(stringResource(R.string.community_forum_url))

    ExternalLinkListItem(title = label, onClick = openForum, position = Position.Top)
}

@Composable
private fun ReportProblem(onClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.report_problem),
        subtitle = stringResource(id = R.string.report_problem_row_subtitle),
        onClick = onClick,
        position = Position.Middle,
    )
}

@Composable
private fun ForumSignInCode(onClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.forum_sign_in_code),
        subtitle = stringResource(id = R.string.forum_sign_in_code_row_subtitle),
        onClick = onClick,
        position = Position.Middle,
    )
}

@Composable
private fun DaitaListItem(isDaitaEnabled: Boolean, onDaitaClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.daita),
        trailingText = stringResource(if (isDaitaEnabled) R.string.on else R.string.off),
        onClick = onDaitaClick,
        position = Position.Top,
        testTag = DAITA_CELL_TEST_TAG,
    )
}

@Composable
private fun MultihopCell(isEnabled: Boolean, onMultihopClick: () -> Unit) {
    // Multi-hop is a real toggle now (2-hop entry+exit vs the single-hop 1-hop
    // circuit), so the subtitle tracks the actual setting to stay consistent
    // with the connect screen's multi-hop indicator. The tunnel still always
    // rides the multi-hop wire; ON/OFF only picks whether a distinct entry
    // relay fronts the exit (see WarrenTunnelConfigBuilder + WarrenQuinnAdapter).
    NavigationListItem(
        title = stringResource(id = R.string.multihop),
        trailingText = stringResource(if (isEnabled) R.string.on else R.string.off),
        onClick = onMultihopClick,
        position = Position.Middle,
        testTag = MULTIHOP_CELL_TEST_TAG,
    )
}

@Composable
private fun PortForwardingCell(isEnabled: Boolean, onPortForwardingClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.tunnel_natpmp_title),
        trailingText = stringResource(if (isEnabled) R.string.on else R.string.off),
        onClick = onPortForwardingClick,
        position = Position.Middle,
    )
}

@Composable
private fun VpnSettingsCell(onVpnSettingsClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.settings_warren_tunnel),
        onClick = onVpnSettingsClick,
        position = Position.Bottom,
        testTag = VPN_SETTINGS_CELL_TEST_TAG,
    )
}

private fun LazyListScope.loading() {
    item { WarrenCircularProgressIndicatorLarge() }
}
