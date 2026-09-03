//
//  WarrenEnvStandDown.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime
import WarrenSettings

/// Where the coexistence record is kept. Behind a protocol so the stand-down
/// can be exercised without UserDefaults.
protocol WarrenEnvStandDownStoring: AnyObject {
    var warrenEnvStandDown: WarrenEnvStandDownRecord { get set }
}

extension AppPreferences: WarrenEnvStandDownStoring {}

/// A connect request refused because this build has stood down for a
/// higher-priority product environment. Typed, like the desktop daemon's
/// refusal, so a caller cannot read it as a transport failure worth retrying:
/// the banner's re-enable is the only way back, as `ClearEnvYield` is there.
///
/// Deliberately not localized. While the record stands, the stand-down banner
/// is the exclusive one on the connect screen, so no other banner is rendered
/// and this text reaches the log alone.
struct WarrenEnvStandDownRefusal: LocalizedError {
    /// The environment this build stood down for (`prod`, `staging`). An
    /// environment name, never an account, an address or a key.
    let environment: String

    var errorDescription: String? {
        "This build stood down for \(environment) and will not connect until it is re-enabled."
    }
}

/// Coexistence with a higher-priority product environment: prod outranks
/// staging, staging outranks beta, and the outranked build is the one that
/// stands down. Nothing here ever commands the other install, and the
/// production build is not modified.
///
/// iOS shows far less than the desktop, and the gap is not closable. The
/// desktop daemon dials the higher environment's socket and yields
/// continuously on its live tunnel state; iOS exposes no API for that.
/// `NETunnelProviderManager.loadAllFromPreferences` returns only the CALLING
/// app's configurations, and there is no API to stop another app's tunnel,
/// clear its on-demand rules or turn off its `includeAllNetworks`. So the rule
/// here is the other app's PRESENCE, observed once per launch, on the
/// transition. A continuous rule would undo the user's manual re-enable at the
/// next start, since the other install is still there.
///
/// That presence is read from `UIApplication.canOpenURL`, and a URL scheme is
/// not an identity: any app on the device can claim `warren://` in its own
/// Info.plist, and the platform offers no signed alternative a build can check
/// on its own. So an observation only ever OFFERS the stand-down. Applying it
/// takes an explicit tap ([confirmStandDown]), which is what stops the design
/// from being a documented way for any local app to disconnect someone's
/// tunnel and disarm their kill switch.
///
/// [refresh] is the whole observation. The banner it raises is cleared by
/// [confirmStandDown] (accept), by [reEnable] (turn it down, or ask for this
/// build back afterwards), and by the other install going away.
final class WarrenEnvStandDown {
    /// The device this stand-down observes and the levers it pulls, injected
    /// so the teardown ORDER can be asserted without NetworkExtension.
    struct Device {
        /// Whether an app registering `scheme` is installed, from
        /// `UIApplication.canOpenURL`. It answers only for schemes the
        /// Info.plist declares in `LSApplicationQueriesSchemes`, so a missing
        /// declaration reads exactly like an absent app.
        var isInstalled: (String) -> Bool

        /// Stops the tunnel and clears the on-demand rules, calling back once
        /// the tunnel is down. The two are one call because iOS ties them
        /// together: `StopTunnelOperation` writes `isOnDemandEnabled` into the
        /// manager before it stops the tunnel, and stopping while an
        /// `NEOnDemandRuleConnect` still stands would just bring the tunnel
        /// straight back up.
        var stopTunnelAndClearOnDemand: (@escaping () -> Void) -> Void

        /// The "Force all apps" state, iOS's kill switch.
        var readKillSwitch: () -> Bool
        var writeKillSwitch: (Bool) -> Void
    }

    /// The record changed: the banner has to be re-rendered.
    var didChange: (() -> Void)?

    private let store: WarrenEnvStandDownStoring
    private let higherPriority: [WarrenProductAnchors]
    private let device: Device

    init(
        store: WarrenEnvStandDownStoring,
        higherPriority: [WarrenProductAnchors] = WarrenProductAnchors.higherPriority,
        device: Device
    ) {
        self.store = store
        self.higherPriority = higherPriority
        self.device = device
    }

    var record: WarrenEnvStandDownRecord { store.warrenEnvStandDown }

    var isStandingDown: Bool { record.isStandingDown }

    /// The strongest environment that outranks this build and whose app is on
    /// the device, `nil` when none is. Pure, so the detection is testable
    /// without a device.
    static func assertingEnvironment(
        higherPriority: [WarrenProductAnchors],
        isInstalled: (String) -> Bool
    ) -> WarrenProductAnchors? {
        higherPriority.first { isInstalled($0.deepLinkScheme) }
    }

    /// Observe the device once. A presence that has not changed since the last
    /// observation does nothing at all, so an answered offer holds and a build
    /// that already stood down is not torn down again on every start.
    func refresh() {
        let seen = Self.assertingEnvironment(
            higherPriority: higherPriority,
            isInstalled: device.isInstalled
        )?.name
        guard seen != record.higherEnvironmentSeen else { return }
        if let seen {
            offerStandDown(to: seen)
        } else {
            forget()
        }
    }

    /// The user accepted the stand-down.
    ///
    /// The order is the safety, and it is the desktop daemon's order
    /// (`warren_env_arbitration::stand_down_plan`): record what is about to be
    /// given up, take the tunnel down while the block is still armed, and lift
    /// the block only once the tunnel is actually down. Lifting it first would
    /// leave the device with no tunnel and no block for the whole teardown.
    ///
    /// The block is released even if the stop reported an error: the operation
    /// has run its course, and leaving "Force all apps" armed behind a tunnel
    /// that will not come up is how a user loses the network entirely.
    func confirmStandDown() {
        var record = store.warrenEnvStandDown
        guard record.isOfferingStandDown else { return }
        record.standDownApplied = true
        record.restoreIncludeAllNetworks = device.readKillSwitch()
        store.warrenEnvStandDown = record
        announceChange()
        device.stopTunnelAndClearOnDemand { [weak self, device] in
            device.writeKillSwitch(false)
            self?.announceChange()
        }
    }

    /// The user turned the offer down, or asked for this build back after
    /// accepting it. Sticky: the record keeps the higher install marked as
    /// seen, so no later start raises the offer for it again. Only a fresh
    /// install of it is a new transition.
    ///
    /// The kill switch is put back only when the teardown actually ran. After
    /// a mere offer nothing was ever turned off, and writing the recorded
    /// value then would overwrite whatever the user has set since.
    func reEnable() {
        var record = store.warrenEnvStandDown
        guard record.isStandingDown || record.isOfferingStandDown else { return }
        let restore = record.standDownApplied ? record.restoreIncludeAllNetworks : nil
        record.reEnabled = true
        store.warrenEnvStandDown = record
        if let restore {
            device.writeKillSwitch(restore)
        }
        announceChange()
    }

    /// A higher environment appeared, or a stronger one took over from the one
    /// already recorded. Nothing on the device is touched here.
    ///
    /// A stand-down already in force keeps the two values it recorded and only
    /// moves the environment it names: the kill switch is off by then, so
    /// reading it again would save the neutralised value as the one to put
    /// back and quietly destroy the user's setting. That is the defect the
    /// desktop fixed by threading the held record through `stand_down_plan`.
    private func offerStandDown(to environment: String) {
        var record = store.warrenEnvStandDown
        if record.isStandingDown {
            record.higherEnvironmentSeen = environment
            store.warrenEnvStandDown = record
        } else {
            store.warrenEnvStandDown = WarrenEnvStandDownRecord(
                higherEnvironmentSeen: environment
            )
        }
        announceChange()
    }

    /// The higher install is gone. What this build gave up for it goes back
    /// before anything else, because the record about to be dropped is the
    /// only place the value to restore is written down. The record is then
    /// dropped whole rather than merely cleared of its banner, so a reinstall
    /// later reads as the new transition it is and offers the stand-down
    /// again.
    private func forget() {
        let record = store.warrenEnvStandDown
        if record.isStandingDown {
            device.writeKillSwitch(record.restoreIncludeAllNetworks)
        }
        store.warrenEnvStandDown = WarrenEnvStandDownRecord()
        announceChange()
    }

    /// The record moved: re-render the banner, and tell the connect screen.
    ///
    /// The screen cannot infer this from a settings write, because both the
    /// stand-down and the re-enable write the kill switch and a write that
    /// changes nothing never reaches the settings observer. The post is
    /// hopped to the main queue: the tunnel-stop completion arrives on the
    /// tunnel manager's own queue, and the observer touches the view model.
    private func announceChange() {
        didChange?()
        DispatchQueue.main.async {
            NotificationCenter.default.post(
                name: .warrenEnvStandDownDidChange,
                object: nil
            )
        }
    }
}

extension Notification.Name {
    /// The coexistence record moved. Posted by [WarrenEnvStandDown] and
    /// observed by the connect screen, which greys its buttons out exactly
    /// while the stand-down is in force.
    static let warrenEnvStandDownDidChange = Notification.Name(
        "WarrenEnvStandDownDidChange"
    )
}
