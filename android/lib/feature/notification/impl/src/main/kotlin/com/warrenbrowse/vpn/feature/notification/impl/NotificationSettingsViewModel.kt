package com.warrenbrowse.vpn.feature.notification.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

sealed interface NotificationSettingsSideEffect {
    data object OpenSystemNotificationsSettings : NotificationSettingsSideEffect
}

class NotificationSettingsViewModel(
    private val userPreferencesRepository: UserPreferencesRepository,
    private val localSettings: WarrenLocalSettingsRepository,
    forumIdentity: ForumIdentityRepository,
) : ViewModel() {

    private val _uiSideEffect = Channel<NotificationSettingsSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    val uiState =
        combine(
                userPreferencesRepository.preferencesFlow(),
                localSettings.forumNotificationsEnabled,
                forumIdentity.identity,
            ) { settings, forumEnabled, identity ->
                NotificationSettingsUiState(
                        locationInNotificationEnabled = settings.showLocationInSystemNotification,
                        forumNotificationsEnabled = if (identity != null) forumEnabled else null,
                    )
                    .toLc<Unit, NotificationSettingsUiState>()
            }
            .stateIn(
                scope = viewModelScope,
                started = SharingStarted.Companion.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                initialValue = Lc.Loading(Unit),
            )

    fun onToggleLocationInNotifications(enabled: Boolean) {
        viewModelScope.launch {
            userPreferencesRepository.setLocationInNotificationEnabled(enabled)
        }
    }

    fun onToggleForumNotifications(enabled: Boolean) =
        localSettings.setForumNotificationsEnabled(enabled)

    fun openSystemNotificationsSettings() = viewModelScope.launch {
        _uiSideEffect.send(NotificationSettingsSideEffect.OpenSystemNotificationsSettings)
    }
}
