//
//  WarrenPathHealthNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import UIKit
import WarrenRustRuntime
import WarrenTypes

/// "SERVER NOT FORWARDING TRAFFIC", and its milder sibling for a path that
/// only drops the large frames.
///
/// A drained or half-swapped exit keeps answering QUIC keep-alives, so the
/// tunnel state stays `connected` while nothing reaches the internet. Every
/// other guard on this client looks at the client half of the circuit and is
/// satisfied by exactly that, which is how the app came to say "You are
/// protected" over a tunnel carrying nothing. The goodput prober is the one
/// thing that can see it, and until now iOS computed its verdict and showed it
/// to nobody; desktop has `WarrenExitEgressNotificationProvider` and Android
/// `InAppNotification.ExitEgressDead`.
///
/// Informational: recovery is automatic (the verdict clears on the next
/// successful probe, and session liveness or a drain migration handles the
/// switch), so there is no action button.
final class WarrenPathHealthNotificationProvider: NotificationProvider,
    InAppNotificationProvider
{
    /// How often the verdict is re-read. It rides the extension's own 2 s
    /// stats tick into the App Group, so a faster poll here would only read
    /// the same value twice.
    static let pollInterval: TimeInterval = 2

    /// What the banner says, from the verdict and whether the tunnel claims
    /// to be up. Pure, so the decision is tested without a tunnel: a verdict
    /// only means something while the tunnel claims `connected`, because in
    /// every other state the screen already tells the truth.
    static func verdict(
        pathHealth: WarrenPathHealth,
        tunnelState: TunnelState
    ) -> WarrenPathHealth? {
        guard case .connected = tunnelState else { return nil }
        return pathHealth == .healthy ? nil : pathHealth
    }

    private let suiteName: String?
    private let pollInterval: TimeInterval
    private var tunnelObserver: TunnelBlockObserver?
    private var timer: DispatchSourceTimer?
    private var tunnelState: TunnelState = .disconnected
    private var shown: WarrenPathHealth?

    init(
        tunnelManager: TunnelManager,
        suiteName: String? = ApplicationConfiguration.securityGroupIdentifier,
        pollInterval: TimeInterval = WarrenPathHealthNotificationProvider.pollInterval
    ) {
        self.suiteName = suiteName
        self.pollInterval = pollInterval
        super.init()

        let tunnelObserver = TunnelBlockObserver(
            didLoadConfiguration: { [weak self] tunnelManager in
                self?.handle(tunnelManager.tunnelStatus.state)
            },
            didUpdateTunnelStatus: { [weak self] _, tunnelStatus in
                self?.handle(tunnelStatus.state)
            }
        )
        self.tunnelObserver = tunnelObserver
        tunnelManager.addObserver(tunnelObserver)
    }

    deinit {
        timer?.cancel()
    }

    override var identifier: NotificationProviderIdentifier {
        .warrenPathHealthNotificationProvider
    }

    /// Below the tunnel-status provider, which owns the blocked and
    /// no-network cases; this one speaks while the tunnel believes itself up.
    override var priority: NotificationPriority {
        .high
    }

    var notificationDescriptor: InAppNotificationDescriptor? {
        guard let shown else { return nil }
        return InAppNotificationDescriptor(
            identifier: identifier,
            style: .error,
            title: Self.title(for: shown),
            body: NSAttributedString(string: Self.body(for: shown))
        )
    }

    static func title(for health: WarrenPathHealth) -> String {
        switch health {
        case .degradedBoth:
            return NSLocalizedString(
                "SERVER NOT FORWARDING TRAFFIC",
                tableName: "Settings",
                comment: ""
            )
        case .degradedLarge:
            return NSLocalizedString(
                "CONNECTION CARRYING LITTLE",
                tableName: "Settings",
                comment: ""
            )
        case .healthy:
            return ""
        }
    }

    static func body(for health: WarrenPathHealth) -> String {
        switch health {
        case .degradedBoth:
            return NSLocalizedString(
                "The server stopped forwarding your traffic. Warren will switch or reconnect automatically.",
                tableName: "Settings",
                comment: ""
            )
        case .degradedLarge:
            return NSLocalizedString(
                "Something on the way is dropping the larger packets, so downloads and calls will struggle. Warren keeps measuring and will move you if it does not clear.",
                tableName: "Settings",
                comment: ""
            )
        case .healthy:
            return ""
        }
    }

    private func handle(_ state: TunnelState) {
        tunnelState = state
        // The verdict only means something while the tunnel claims to be up,
        // so the poll runs exactly then and stops otherwise. A poll left
        // running over a disconnected tunnel would keep reporting the session
        // that ended.
        guard case .connected = state else {
            timer?.cancel()
            timer = nil
            publish(nil)
            return
        }
        guard timer == nil else { return }

        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now(), repeating: pollInterval)
        timer.setEventHandler { [weak self] in
            self?.poll()
        }
        timer.resume()
        self.timer = timer
    }

    private func poll() {
        publish(Self.verdict(pathHealth: read(), tunnelState: tunnelState))
    }

    /// The verdict the extension last broadcast. Absent means healthy: no
    /// session has published one, which is not a degradation.
    private func read() -> WarrenPathHealth {
        guard let suiteName,
            let defaults = UserDefaults(suiteName: suiteName),
            let raw = defaults.object(forKey: WarrenAppGroupKey.pathHealth.rawValue) as? Int,
            let health = WarrenPathHealth(rawValue: Int32(raw))
        else {
            return .healthy
        }
        return health
    }

    private func publish(_ health: WarrenPathHealth?) {
        guard health != shown else { return }
        shown = health
        invalidate()
    }
}
