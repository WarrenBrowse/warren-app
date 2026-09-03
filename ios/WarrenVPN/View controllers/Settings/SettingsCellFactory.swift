//
//  SettingsCellFactory.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2023-03-09.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenSettings
import UIKit

@MainActor
final class SettingsCellFactory: @preconcurrency CellFactoryProtocol {
    let tableView: UITableView
    var viewModel: SettingsViewModel
    var breadcrumbs: Set<Breadcrumb>
    private let interactor: SettingsInteractor
    private var contentSizeCategory = UIApplication.shared.preferredContentSizeCategory

    init(tableView: UITableView, interactor: SettingsInteractor, breadcrumbs: Set<Breadcrumb>) {
        self.tableView = tableView
        self.interactor = interactor
        self.breadcrumbs = breadcrumbs

        viewModel = SettingsViewModel(from: interactor.tunnelSettings)

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(preferredContentSizeChanged(_:)),
            name: UIContentSizeCategory.didChangeNotification,
            object: nil
        )
    }

    func makeCell(for item: SettingsDataSource.Item, indexPath: IndexPath) -> UITableViewCell {
        let cell: UITableViewCell

        cell =
            tableView
            .dequeueReusableCell(
                withIdentifier: item.reuseIdentifier.rawValue
            )
            ?? SettingsCell(
                style: contentSizeCategory.isLarge ? .subtitle : item.reuseIdentifier.cellStyle,
                reuseIdentifier: item.reuseIdentifier.rawValue
            )

        // Configure the cell with the common logic
        configureCell(cell, item: item, indexPath: indexPath)

        return cell
    }

    func configureCell(_ cell: UITableViewCell, item: SettingsDataSource.Item, indexPath: IndexPath) {
        switch item {
        case .vpnSettings:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("VPN settings", comment: "")
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .vpnSettings }

        case .changelog:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = NSLocalizedString("What’s new", comment: "")
            // Soft update path: when the verified manifest lists a newer
            // (non-mandatory) version, say so instead of only the version.
            if WarrenAppVersionGate.shared.updateAvailableVersion != nil {
                cell.detailTitleLabel.text = NSLocalizedString("Update available", comment: "")
            } else {
                cell.detailTitleLabel.text = Bundle.main.productVersion
            }
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .changelog }

        case .faq:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("FAQs & Guides", comment: "")
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .externalLink
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .faq }

        case .apiAccess:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = NSLocalizedString("API access", comment: "")
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .apiAccess }

        case .daita:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("DAITA", comment: "")

            cell.detailTitleLabel.text =
                viewModel.daitaSettings.isEnabled
                ? NSLocalizedString("On", comment: "")
                : NSLocalizedString("Off", comment: "")

            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .daita }

        case .multihop:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("Multihop", comment: "")

            cell.detailTitleLabel.text = viewModel.multihopState.description

            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .multihop }

        case .language:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("Language", comment: "")
            cell.detailTitleLabel.text = viewModel.currentLanguage
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .externalLink
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .language }

        case .notificationSettings:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("Notifications", comment: "")
            cell.detailTitleLabel.text = nil
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .notificationSettings }

        case .includeAllNetworks:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = NSLocalizedString("Force all apps", comment: "")

            cell.detailTitleLabel.text =
                viewModel.includeAllNetworksState.isEnabled
                ? NSLocalizedString("On", comment: "")
                : NSLocalizedString("Off", comment: "")

            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .includeAllNetworks }

        case .warrenWalletBackup:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = String(
                localized: "Recovery phrase",
                table: "Wallet",
                comment: "Settings row that opens the Face ID gated wallet backup view"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenWalletBackup }

        case .warrenWalletErase:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = String(
                localized: "Erase wallet",
                table: "Wallet",
                comment: "Destructive Settings row that opens the wallet wipe confirmation flow"
            )
            cell.titleLabel.textColor = .Warren.error
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenWalletErase }

        case .warrenWalletIdentity:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = String(
                localized: "Wallet identity",
                table: "Wallet",
                comment: "Settings row that opens the read-only pubkey display"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenWalletIdentity }

        case .warrenTunnelStatistics:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = String(
                localized: "Tunnel statistics",
                table: "Settings",
                comment: "Settings row that opens the live tunnel counters view"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenTunnelStatistics }

        case .warrenDiagnosticInfo:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = String(
                localized: "Diagnostic info",
                table: "Settings",
                comment: "Settings row that opens the screenshot-friendly support payload view"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenDiagnosticInfo }

        case .warrenAbout:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = String(
                localized: "About Warren",
                table: "Settings",
                comment: "Settings row that opens the marketing/privacy/source-code links view"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenAbout }

        case .warrenForumSignInCode:
            guard let cell = cell as? SettingsCell else { return }
            cell.titleLabel.text = String(
                localized: "Sign in to the forum with a code",
                table: "Settings",
                comment: "Settings row that opens the forum sign-in code entry"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenForumSignInCode }

        case .warrenPortForwarding:
            guard let cell = cell as? SettingsCell else { return }

            cell.titleLabel.text = String(
                localized: "Port forwarding",
                table: "Settings",
                comment: "Settings row that opens the NAT-PMP port forwarding configuration"
            )
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = breadcrumbs.first { $0.navigationRoute == .warrenPortForwarding }

        case .debugOptions:
            guard let cell = cell as? SettingsCell else { return }

            // Developer-only row (DEBUG builds); deliberately not localized.
            cell.titleLabel.text = "Debug options"
            cell.detailTitleLabel.text = nil
            cell.setAccessibilityIdentifier(item.accessibilityIdentifier)
            cell.disclosureType = .chevron
            cell.breadcrumb = nil
        }
    }

    @objc private func preferredContentSizeChanged(_ notification: Notification) {
        if let newContentSizeCategory = notification.userInfo?[UIContentSizeCategory.newValueUserInfoKey]
            as? UIContentSizeCategory
        {
            contentSizeCategory = newContentSizeCategory
        }
    }
}

private extension UIContentSizeCategory {
    var isLarge: Bool {
        (self > .extraExtraExtraLarge) || (self > .accessibilityLarge)
    }
}
