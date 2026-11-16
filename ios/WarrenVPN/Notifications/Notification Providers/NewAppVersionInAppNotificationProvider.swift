//
//  NewAppVersionInAppNotificationProvider.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2026-02-06.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenREST
import WarrenSettings
import WarrenTypes
import UIKit

final class NewAppVersionInAppNotificationProvider:
    NotificationProvider, InAppNotificationProvider, @unchecked Sendable
{
    private var appVersionService: AppVersionService
    private var tunnelObserver: TunnelBlockObserver?

    private var hasEnabledIncludeAllNetworksAtLeastOnce = false
    private var newAppVersionIsAvailable = false
    private var tunnelIsSecured = false
    private var includeAllNetworksIsEnabled = false {
        didSet {
            if tunnelIsSecured && includeAllNetworksIsEnabled {
                hasEnabledIncludeAllNetworksAtLeastOnce = true
            }
        }
    }

    init(tunnelManager: TunnelManager, appVersionService: AppVersionService) {
        self.appVersionService = appVersionService

        super.init()

        self.appVersionService.onNewAppVersion = { [weak self] in
            guard let self else { return }

            newAppVersionIsAvailable = true
            invalidate()
        }

        let tunnelObserver = TunnelBlockObserver(
            didLoadConfiguration: { [weak self] tunnelManager in
                guard let self else { return }

                tunnelIsSecured = tunnelManager.tunnelStatus.state.isSecured
                includeAllNetworksIsEnabled = tunnelManager.settings.includeAllNetworks.includeAllNetworksIsEnabled
                invalidate()
            },
            didUpdateTunnelStatus: { [weak self] tunnelManager, _ in
                guard let self else { return }

                tunnelIsSecured = tunnelManager.tunnelStatus.state.isSecured
                includeAllNetworksIsEnabled = tunnelManager.settings.includeAllNetworks.includeAllNetworksIsEnabled
                invalidate()
            }
        )
        self.tunnelObserver = tunnelObserver

        tunnelManager.addObserver(tunnelObserver)
        self.appVersionService.scheduleTimer(deadline: .now)
    }

    override var identifier: NotificationProviderIdentifier {
        .newAppVersionInAppNotification
    }

    override var priority: NotificationPriority {
        .critical
    }

    // MARK: - InAppNotificationProvider

    var notificationDescriptor: InAppNotificationDescriptor? {
        guard newAppVersionIsAvailable && tunnelIsSecured else {
            return nil
        }

        // The logic here is that if tunnel has been connected and IAN has been enabled at the same time at least once,
        // we should show the "safe" banner whenever the tunnel is up and IAN is disabled. We keep this state until
        // the app is restarted. If neither of the above is true, we shouldn't show a banner at all since we don't
        // want to urge people to update while we're using phased releases on AppStore.
        guard includeAllNetworksIsEnabled || hasEnabledIncludeAllNetworksAtLeastOnce else {
            return nil
        }

        return Self.descriptor(
            version: appVersionService.availableVersion,
            includeAllNetworksIsEnabled: includeAllNetworksIsEnabled,
            install: { [weak self] in
                guard let self else { return }
                NotificationManager.shared.notificationProvider(self, didReceiveAction: "\(identifier)")
            }
        )
    }

    /// Mockup 8d's update banner: "Update available", the version, and an Install button that
    /// takes the update path, as a tap on the card does. With "Force all apps" on, updating would
    /// cut the network, so the body says that instead of the version.
    static func descriptor(
        version: String?,
        includeAllNetworksIsEnabled: Bool,
        install: @escaping () -> Void
    ) -> InAppNotificationDescriptor {
        let body: String
        if includeAllNetworksIsEnabled {
            body = String(
                format:
                    NSLocalizedString(
                        "“%@” is enabled, please disable it or disconnect before updating or you will "
                            + "lose network connectivity.", comment: ""),
                NSLocalizedString("Force all apps", comment: "")
            )
        } else if let version {
            body = String(format: NSLocalizedString("Version %@", comment: ""), version)
        } else {
            body = NSLocalizedString("Install the latest app version to stay up to date.", comment: "")
        }

        return InAppNotificationDescriptor(
            identifier: .newAppVersionInAppNotification,
            style: .warning,
            title: NSLocalizedString("Update available", comment: ""),
            body: NSAttributedString(string: body),
            button: InAppNotificationAction(
                title: NSLocalizedString("Install", comment: ""),
                handler: install
            ),
            tapAction: InAppNotificationAction(handler: install),
            icon: .update
        )
    }
}
