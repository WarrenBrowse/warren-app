//
//  VPNSettingsViewController.swift
//  MullvadVPN
//
//  Created by pronebird on 19/05/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenSettings
import SwiftUI
import UIKit

protocol VPNSettingsViewControllerDelegate: AnyObject {
    func showIPOverrides()
}

class VPNSettingsViewController: UITableViewController {
    private let interactor: VPNSettingsInteractor
    private var dataSource: VPNSettingsDataSource?
    private let alertPresenter: AlertPresenter
    private let section: VPNSettingsSection?
    weak var delegate: VPNSettingsViewControllerDelegate?

    override var preferredStatusBarStyle: UIStatusBarStyle {
        .lightContent
    }

    init(
        interactor: VPNSettingsInteractor,
        alertPresenter: AlertPresenter,
        section: VPNSettingsSection?
    ) {
        self.interactor = interactor
        self.alertPresenter = alertPresenter
        self.section = section
        super.init(style: .grouped)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        tableView.setAccessibilityIdentifier(.vpnSettingsTableView)
        tableView.backgroundColor = .secondaryColor
        tableView.separatorColor = .secondaryColor
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 60
        tableView.estimatedSectionHeaderHeight = tableView.estimatedRowHeight
        tableView.allowsMultipleSelection = true

        // Warren tunnels use always-on M4.0 HTTP/3 mimicry; the legacy
        // WireGuard obfuscation methods do not apply. The dedicated
        // obfuscation screen therefore shows a read-only indicator instead
        // of the interactive method picker (mirrors the desktop view).
        if section == .obfuscation {
            navigationItem.title = NSLocalizedString("Obfuscation", comment: "")
            installObfuscationReadOnlyView()
            return
        }

        dataSource = VPNSettingsDataSource(
            tableView: tableView,
            section: section
        )

        dataSource?.delegate = self

        navigationItem.title = NSLocalizedString("VPN settings", comment: "")

        interactor.tunnelSettingsDidChange = { [weak self] newSettings in
            self?.dataSource?.reload(from: newSettings)
        }
        dataSource?.update(from: interactor.tunnelSettings)

        dataSource?.setAvailablePortRanges(interactor.cachedRelays?.relays.wireguard.portRanges ?? [])
        interactor.cachedRelaysDidChange = { [weak self] cachedRelays in
            self?.dataSource?.setAvailablePortRanges(cachedRelays.relays.wireguard.portRanges)
        }

        let showsSingleSection = section != nil
        tableView.tableHeaderView = UIView(
            frame: CGRect(
                origin: .zero,
                size: CGSize(width: 0, height: showsSingleSection ? 0 : UIMetrics.TableView.emptyHeaderHeight)
            ))
    }

    /// Hosts the read-only obfuscation indicator over the (empty) table view.
    /// Used for the dedicated obfuscation screen instead of the picker.
    private func installObfuscationReadOnlyView() {
        let hosting = UIHostingController(rootView: WarrenObfuscationSettingsReadOnlyView())
        hosting.view.backgroundColor = .clear
        addChild(hosting)
        hosting.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(hosting.view)
        NSLayoutConstraint.activate([
            hosting.view.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            hosting.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hosting.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            hosting.view.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
        ])
        hosting.didMove(toParent: self)
    }
}

extension VPNSettingsViewController: @preconcurrency VPNSettingsDataSourceDelegate {
    func humanReadablePortRepresentation() -> String {
        let ranges = interactor.cachedRelays?.relays.wireguard.portRanges ?? []
        return
            ranges
            .compactMap { range in
                if let minPort = range.first, let maxPort = range.last {
                    return minPort == maxPort ? String(minPort) : "\(minPort)-\(maxPort)"
                } else {
                    return nil
                }
            }
            .joined(separator: ", ")
    }

    func didUpdateTunnelSettings(_ update: TunnelSettingsUpdate) {
        interactor.updateSettings([update])
    }

    func showInfo(for item: VPNSettingsInfoButtonItem) {
        let presentation = AlertPresentation(
            id: "vpn-settings-content-blockers-alert",
            icon: .info,
            message: item.description,
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Got it!", comment: ""),
                    style: .default
                )
            ]
        )

        alertPresenter.showAlert(presentation: presentation, animated: true)
    }

    func showDetails(for item: VPNSettingsDetailsButtonItem) {
        switch item {
        case .udpOverTcp:
            showUDPOverTCPObfuscationSettings()
        case .wireguardOverShadowsocks:
            showShadowsocksObfuscationSettings()
        case .lwo:
            showLwoObfuscationSettings()
        }
    }

    func showDNSSettings() {
        let viewController = CustomDNSViewController(interactor: interactor, alertPresenter: alertPresenter)
        navigationController?.pushViewController(viewController, animated: true)
    }

    func showIPOverrides() {
        delegate?.showIPOverrides()
    }

    private func showUDPOverTCPObfuscationSettings() {
        let viewModel = TunnelUDPOverTCPObfuscationSettingsViewModel(tunnelManager: interactor.tunnelManager)
        let view = UDPOverTCPObfuscationSettingsView(viewModel: viewModel)
        let vc = UIHostingController(rootView: view)
        vc.title = NSLocalizedString("UDP-over-TCP", comment: "")
        navigationController?.pushViewController(vc, animated: true)
    }

    private func showShadowsocksObfuscationSettings() {
        let viewModel = TunnelShadowsocksObfuscationSettingsViewModel(tunnelManager: interactor.tunnelManager)
        let view = ShadowsocksObfuscationSettingsView(viewModel: viewModel)
        let vc = UIHostingController(rootView: view)
        vc.title = NSLocalizedString("Shadowsocks", comment: "")
        navigationController?.pushViewController(vc, animated: true)
    }

    private func showLwoObfuscationSettings() {
        let viewModel = TunnelLwoObfuscationSettingsViewModel(
            tunnelManager: interactor.tunnelManager,
            portRanges: interactor.cachedRelays?.relays.wireguard.portRanges ?? []
        )
        let view = LwoObfuscationSettingsView(viewModel: viewModel)
        let vc = UIHostingController(rootView: view)
        vc.title = NSLocalizedString("LWO", comment: "")
        navigationController?.pushViewController(vc, animated: true)
    }

    func didSelectWireGuardPort(_ port: UInt16?) {
        interactor.setPort(port)
    }
}
