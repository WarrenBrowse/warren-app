package com.warrenbrowse.vpn.lib.repository

import androidx.datastore.core.DataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.repository.UserPreferences

class UserPreferencesRepository(
    private val userPreferencesStore: DataStore<UserPreferences>,
    private val buildVersion: BuildVersion,
) {
    fun preferencesFlow(): Flow<UserPreferences> = userPreferencesStore.data

    fun showAndroid16ConnectWarning(): Flow<Boolean> =
        userPreferencesStore.data.map { it.showAndroid16ConnectWarning }

    suspend fun preferences(): UserPreferences = userPreferencesStore.data.first()

    suspend fun setPrivacyDisclosureAccepted() {
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setIsPrivacyDisclosureAccepted(true).build()
        }
    }

    suspend fun setHasDisplayedChangelogNotification() {
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setLastShownChangelogVersionCode(buildVersion.code).build()
        }
    }

    suspend fun setLocationInNotificationEnabled(enable: Boolean) {
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setShowLocationInSystemNotification(enable).build()
        }
    }

    suspend fun setShowAndroid16ConnectWarning(show: Boolean) =
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setShowAndroid16ConnectWarning(show).build()
        }

    /**
     * Version of the update prompt the user dismissed, empty when none. Gating
     * on the version rather than a boolean is what makes the dismissal apply to
     * that release only: the next upgrade raises the banner again.
     */
    fun dismissedUpgradeVersion(): Flow<String> =
        userPreferencesStore.data.map { it.dismissedUpgradeVersion }

    suspend fun setDismissedUpgradeVersion(version: String) {
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setDismissedUpgradeVersion(version).build()
        }
    }

    fun showSystemAppsSplitTunneling(): Flow<Boolean> =
        userPreferencesStore.data.map { it.showSystemAppsSplitTunneling }

    suspend fun setShowSystemAppsSplitTunneling(show: Boolean) {
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setShowSystemAppsSplitTunneling(show).build()
        }
    }
}
