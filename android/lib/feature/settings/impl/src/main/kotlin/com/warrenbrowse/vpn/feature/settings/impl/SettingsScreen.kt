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
import com.warrenbrowse.vpn.feature.problemreport.api.ProblemReportNavKey
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenTunnelSettingsNavKey
import com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey
import com.warrenbrowse.vpn.feature.splittunneling.api.SplitTunnelingNavKey
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.util.appendHideNavOnPlayBuild
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateCloseIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.ExternalLinkListItem
import com.warrenbrowse.vpn.lib.ui.component.listitem.NavigationListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.DAITA_CELL_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.LAZY_LIST_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.MULTIHOP_CELL_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import org.koin.androidx.compose.koinViewModel

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
            onReportProblemCellClick = {},
            onApiAccessClick = {},
            onMultihopClick = {},
            onDaitaClick = {},
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

    BackHandler(enabled = navigator.screenIsListDetailTargetWidth) {
        navigator.goBackUntil(SettingsNavKey, inclusive = true)
    }

    // Tablet detail-pane default is the Warren tunnel toggles screen.
    navigator.assureHasDetailPane<SettingsNavKey>(WarrenTunnelSettingsNavKey)

    SettingsScreen(
        state = state,
        onSplitTunnelingCellClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(SplitTunnelingNavKey()) },
        onAppInfoClick = dropUnlessResumed { navigator.navigateReplaceIfDetailPane(AppInfoNavKey) },
        // The Warren API endpoint is hardcoded, so the API access cell does
        // nothing.
        onApiAccessClick = {},
        onReportProblemCellClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(ProblemReportNavKey) },
        // Multihop + DAITA cells route to the unified Warren tunnel settings
        // screen (4-toggle view with picker).
        onMultihopClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenTunnelSettingsNavKey) },
        onDaitaClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenTunnelSettingsNavKey) },
        onNotificationSettingsCellClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(NotificationSettingsNavKey) },
        // Language is the only user-interface setting that applies to Android
        // (desktop's "User interface settings" group). Surfaced as a direct
        // cell instead of a single-item "Appearance" passthrough; per-app
        // language requires API 33, so it is hidden below that.
        onLanguageClick =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                dropUnlessResumed { navigator.navigateReplaceIfDetailPane(LanguageNavKey) }
            } else {
                null
            },
        onWalletClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenWalletSettingsNavKey) },
        onWarrenTunnelClick =
            dropUnlessResumed { navigator.navigateReplaceIfDetailPane(WarrenTunnelSettingsNavKey) },
        onBackClick = dropUnlessResumed { navigator.goBackUntil(SettingsNavKey, inclusive = true) },
    )
}

@Composable
fun SettingsScreen(
    state: Lc<Unit, SettingsUiState>,
    onSplitTunnelingCellClick: () -> Unit,
    onAppInfoClick: () -> Unit,
    onReportProblemCellClick: () -> Unit,
    onApiAccessClick: () -> Unit,
    onMultihopClick: () -> Unit,
    onDaitaClick: () -> Unit,
    onBackClick: () -> Unit,
    onNotificationSettingsCellClick: () -> Unit,
    onLanguageClick: (() -> Unit)? = null,
    onWalletClick: () -> Unit = {},
    onWarrenTunnelClick: () -> Unit = {},
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
                        onSplitTunnelingCellClick = onSplitTunnelingCellClick,
                        onAppInfoClick = onAppInfoClick,
                        onReportProblemCellClick = onReportProblemCellClick,
                        onApiAccessClick = onApiAccessClick,
                        onMultihopClick = onMultihopClick,
                        onDaitaClick = onDaitaClick,
                        onNotificationSettingsCellClick = onNotificationSettingsCellClick,
                        onLanguageClick = onLanguageClick,
                        onWalletClick = onWalletClick,
                        onWarrenTunnelClick = onWarrenTunnelClick,
                    )
                }
            }
        }
    }
}

private fun LazyListScope.content(
    state: SettingsUiState,
    onSplitTunnelingCellClick: () -> Unit,
    onAppInfoClick: () -> Unit,
    onReportProblemCellClick: () -> Unit,
    onApiAccessClick: () -> Unit,
    onMultihopClick: () -> Unit,
    onDaitaClick: () -> Unit,
    onNotificationSettingsCellClick: () -> Unit,
    onLanguageClick: (() -> Unit)? = null,
    onWalletClick: () -> Unit = {},
    onWarrenTunnelClick: () -> Unit = {},
) {
    // D.5 wallet entry - shown at the very top so it's prominent
    // (Warren's identity model = the wallet, not a Mullvad account).
    itemWithDivider {
        NavigationListItem(
            title = stringResource(id = R.string.wallet_settings_section),
            onClick = onWalletClick,
            position = Position.Top,
        )
    }
    // Warren tunnel toggles (DAITA / NAT-PMP / multi-hop / M4.0).
    itemWithDivider {
        NavigationListItem(
            title = stringResource(id = R.string.settings_warren_tunnel),
            onClick = onWarrenTunnelClick,
            position = Position.Bottom,
        )
    }
    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }

    if (state.isLoggedIn) {
        itemWithDivider {
            DaitaListItem(isDaitaEnabled = state.isDaitaEnabled, onDaitaClick = onDaitaClick)
        }
        itemWithDivider {
            MultihopCell(onMultihopClick = onMultihopClick)
        }
        item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }
        item { SplitTunneling(onSplitTunnelingCellClick) }
        item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }
    }

    if (onLanguageClick != null) {
        itemWithDivider {
            NavigationListItem(
                title = stringResource(id = R.string.language),
                onClick = onLanguageClick,
                position = Position.Top,
            )
        }
    }

    itemWithDivider {
        NavigationListItem(
            title = stringResource(id = R.string.settings_notifications),
            onClick = onNotificationSettingsCellClick,
            position = if (onLanguageClick != null) Position.Middle else Position.Top,
        )
    }

    item { AppInfo(onAppInfoClick, state) }

    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }

    itemWithDivider { ReportProblem(onReportProblemCellClick) }

    if (!state.isPlayBuild) {
        itemWithDivider { FaqAndGuides() }
    }

    itemWithDivider { PrivacyPolicy(state) }

    item { Spacer(modifier = Modifier.height(Dimens.cellVerticalSpacing)) }
}

@Composable
private fun SplitTunneling(onSplitTunnelingCellClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.split_tunneling),
        onClick = onSplitTunnelingCellClick,
    )
}

@Composable
private fun AppInfo(navigateToAppInfo: () -> Unit, state: SettingsUiState) {
    NavigationListItem(
        title = stringResource(id = R.string.app_info),
        subtitle = state.appVersion,
        subTitleTextDirection = TextDirection.Ltr,
        showWarning = !state.isSupportedVersion,
        position = Position.Bottom,
        onClick = navigateToAppInfo,
    )
}

@Composable
private fun ReportProblem(onReportProblemCellClick: () -> Unit) {
    NavigationListItem(
        title = stringResource(id = R.string.report_a_problem),
        onClick = { onReportProblemCellClick() },
        position = Position.Top,
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
private fun DaitaListItem(isDaitaEnabled: Boolean, onDaitaClick: () -> Unit) {
    val title = stringResource(id = R.string.daita)
    NavigationListItem(
        title = title,
        subtitle =
            stringResource(
                if (isDaitaEnabled) {
                    R.string.on
                } else {
                    R.string.off
                }
            ),
        onClick = onDaitaClick,
        position = Position.Top,
        testTag = DAITA_CELL_TEST_TAG,
    )
}

@Composable
private fun MultihopCell(onMultihopClick: () -> Unit) {
    // The Warren tunnel always routes through an entry hop before the exit
    // (see WarrenTunnelConfigBuilder + WarrenQuinnAdapter, multiHop = true), so
    // the cell must read "on" to match reality and the home feature indicators.
    // Do NOT revert this to "off": it previously contradicted the active tunnel
    // and the connect screen's multi-hop indicator.
    NavigationListItem(
        title = stringResource(id = R.string.multihop),
        subtitle = stringResource(R.string.on),
        onClick = onMultihopClick,
        position = Position.Bottom,
        testTag = MULTIHOP_CELL_TEST_TAG,
    )
}

private fun LazyListScope.loading() {
    item { WarrenCircularProgressIndicatorLarge() }
}
