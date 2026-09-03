package com.warrenbrowse.vpn.feature.notification.impl

import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.common.compose.isTv
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.util.openAppInfoNotificationSettings
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.SwitchListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import org.koin.androidx.compose.koinViewModel

@Preview("Loading|Normal")
@Composable
private fun PreviewNotificationSettingsScreen(
    @PreviewParameter(NotificationSettingsUiStatePreviewParameterProvider::class)
    state: Lc<Unit, NotificationSettingsUiState>
) {
    AppTheme {
        NotificationSettingsScreen(
            state = state,
            onBackClick = {},
            onToggleLocationInNotifications = {},
            onToggleForumNotifications = {},
            onOpenSystemNotificationsSettings = {},
        )
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun NotificationSettings(navigator: Navigator) {
    val vm = koinViewModel<NotificationSettingsViewModel>()
    val state by vm.uiState.collectAsStateWithLifecycle()

    val context = LocalContext.current
    CollectSideEffectWithLifecycle(vm.uiSideEffect) {
        when (it) {
            NotificationSettingsSideEffect.OpenSystemNotificationsSettings -> {
                context.openAppInfoNotificationSettings()
            }
        }
    }

    NotificationSettingsScreen(
        state = state,
        onBackClick = dropUnlessResumed { navigator.goBack() },
        onToggleLocationInNotifications = vm::onToggleLocationInNotifications,
        onToggleForumNotifications = vm::onToggleForumNotifications,
        onOpenSystemNotificationsSettings = vm::openSystemNotificationsSettings,
    )
}

@Composable
fun NotificationSettingsScreen(
    state: Lc<Unit, NotificationSettingsUiState>,
    onBackClick: () -> Unit,
    onToggleLocationInNotifications: (Boolean) -> Unit,
    onToggleForumNotifications: (Boolean) -> Unit,
    onOpenSystemNotificationsSettings: () -> Unit,
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.settings_notifications),
        navigationIcon = {
            unlessIsDetail { NavigateBackIconButton(onNavigateBack = onBackClick) }
        },
        bottomBar = {
            if (!isTv()) {
                PrimaryButton(
                    modifier =
                        Modifier.windowInsetsPadding(
                                WindowInsets.systemBars.only(WindowInsetsSides.Bottom)
                            )
                            .padding(
                                horizontal = Dimens.sideMargin,
                                vertical = Dimens.screenBottomMargin,
                            ),
                    text = stringResource(R.string.notification_settings),
                    onClick = onOpenSystemNotificationsSettings,
                    trailingIcon = {
                        Icon(
                            imageVector = Icons.AutoMirrored.Rounded.OpenInNew,
                            tint = MaterialTheme.colorScheme.onPrimary,
                            contentDescription = null,
                        )
                    },
                )
            }
        },
    ) { modifier ->
        val scrollState = rememberScrollState()
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier =
                modifier
                    .drawVerticalScrollbar(
                        state = scrollState,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                    )
                    .verticalScroll(state = scrollState)
                    .padding(horizontal = Dimens.sideMarginNew),
        ) {
            when (state) {
                is Lc.Loading -> Loading()
                is Lc.Content -> {
                    NotificationSettingsContent(
                        state = state.value,
                        onToggleLocationInNotifications = onToggleLocationInNotifications,
                        onToggleForumNotifications = onToggleForumNotifications,
                    )
                }
            }
        }
    }
}

@Composable
private fun NotificationSettingsContent(
    state: NotificationSettingsUiState,
    onToggleLocationInNotifications: (Boolean) -> Unit,
    onToggleForumNotifications: (Boolean) -> Unit,
) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        SwitchListItem(
            title = stringResource(R.string.enable_location_in_notification),
            isToggled = state.locationInNotificationEnabled,
            onCellClicked = onToggleLocationInNotifications,
        )
        // Desktop `ForumNotificationsSetting`: shown only to a wallet that has
        // a forum account; off removes the bell, the badge and the
        // notification alike.
        state.forumNotificationsEnabled?.let { enabled ->
            Spacer(modifier = Modifier.height(Dimens.mediumPadding))
            SwitchListItem(
                title = stringResource(R.string.forum_notifications_setting),
                isToggled = enabled,
                onCellClicked = onToggleForumNotifications,
            )
            Text(
                text = stringResource(R.string.forum_notifications_setting_description),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier =
                    Modifier.fillMaxWidth()
                        .padding(horizontal = Dimens.mediumPadding, vertical = Dimens.smallPadding),
            )
        }
    }
}

@Composable
private fun Loading() {
    WarrenCircularProgressIndicatorLarge()
}
