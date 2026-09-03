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

    /**
     * Records that the OS is not blocking connections without this app's VPN,
     * so the UI can point the user at the system setting once. The desktop
     * apps arm their own protection across an update; on mobile the OS owns the
     * tunnel and this setting is the only equivalent.
     */
    suspend fun setShowAlwaysOnVpnAdvice(show: Boolean) =
        userPreferencesStore.updateData { prefs ->
            prefs.toBuilder().setShowAlwaysOnVpnAdvice(show).build()
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

    /** Keys of the operator notices the user has put away. */
    fun dismissedNotices(): Flow<List<String>> =
        userPreferencesStore.data.map { it.dismissedNoticesList }

    /**
     * Puts one notice away for good. Append-only and de-duplicated: the same
     * notice dismissed on two runs must not grow the file forever.
     */
    suspend fun dismissNotice(key: String) {
        userPreferencesStore.updateData { prefs ->
            if (prefs.dismissedNoticesList.contains(key)) {
                prefs
            } else {
                prefs.toBuilder().addDismissedNotices(key).build()
            }
        }
    }

    /** Ids of the launch announcements the reader has put away. */
    fun dismissedAnnouncements(): Flow<List<String>> =
        userPreferencesStore.data.map { it.dismissedAnnouncementsList }

    /**
     * Puts one announcement away for good. Append-only and de-duplicated, like
     * the notices: the same announcement dismissed on two runs must not grow
     * the file forever.
     */
    suspend fun dismissAnnouncement(id: String) {
        userPreferencesStore.updateData { prefs ->
            if (prefs.dismissedAnnouncementsList.contains(id)) {
                prefs
            } else {
                prefs.toBuilder().addDismissedAnnouncements(id).build()
            }
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
