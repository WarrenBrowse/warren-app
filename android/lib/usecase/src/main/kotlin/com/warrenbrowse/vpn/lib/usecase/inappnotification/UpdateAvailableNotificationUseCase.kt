package com.warrenbrowse.vpn.lib.usecase.inappnotification

import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.VersionInfo
import com.warrenbrowse.vpn.lib.repository.AppVersionInfoRepository

/**
 * Emits [InAppNotification.UpdateAvailable] for sideload installs only.
 *
 * Google Play already notifies store installs about updates, so showing an
 * in-app prompt there would be redundant. The notification therefore fires only
 * when ALL of the following hold:
 *  - in-app version notifications are enabled ([isVersionInfoNotificationEnabled]),
 *  - the signed manifest reported a newer stable version
 *    ([VersionInfo.availableUpgrade] is non-null), and
 *  - the app was NOT installed from a store ([isInstalledFromStore] returns false).
 *
 * The forced-update gate ([UnsupportedVersion]) is a separate, install-source
 * agnostic concern handled by [VersionNotificationUseCase] and is unaffected.
 *
 * [isInstalledFromStore] is injected as a lambda rather than the concrete
 * `InstallSourceProvider` so this module does not depend on the app-listing
 * feature module; the wiring in `:app` supplies the provider's method reference.
 */
class UpdateAvailableNotificationUseCase(
    private val appVersionInfoRepository: AppVersionInfoRepository,
    private val isVersionInfoNotificationEnabled: Boolean,
    private val isInstalledFromStore: () -> Boolean,
) : InAppNotificationUseCase {

    override operator fun invoke() =
        appVersionInfoRepository.versionInfo
            .map { versionInfo -> updateAvailableNotification(versionInfo) }
            .distinctUntilChanged()

    private fun updateAvailableNotification(versionInfo: VersionInfo): InAppNotification? {
        if (!isVersionInfoNotificationEnabled) {
            return null
        }
        val upgrade = versionInfo.availableUpgrade ?: return null
        // Store installs are notified by the store itself; suppress here.
        return if (isInstalledFromStore()) {
            null
        } else {
            InAppNotification.UpdateAvailable(upgrade)
        }
    }
}
