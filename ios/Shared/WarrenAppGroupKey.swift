//
//  WarrenAppGroupKey.swift
//  Shared
//
//  Created by Warren on 2026-05-22 (C.4.3.X follow-up).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Single source of truth for the App Group `UserDefaults` keys used
//  by the PacketTunnel extension to broadcast tunnel events to the
//  main app. Previously duplicated as raw string literals in both
//  `PacketTunnelCore/Actor/WarrenQuinnTunnelImplementation.swift` (writer)
//  and `WarrenVPN/View controllers/Tunnel/WarrenAppGroupEvents.swift`
//  (reader) ; centralising prevents silent drift when a key is renamed
//  on one side only.
//

import Foundation

/// App Group `UserDefaults` keys consumed cross-process by the main
/// app's `WarrenAppGroupEvents` observer. Producer side lives in
/// `WarrenQuinnTunnelImplementation.broadcastEvent(_:into:)`.
///
/// Key naming convention : `WarrenTunnel.<eventField>` so they all
/// cluster together in the Settings-app App Group inspector and don't
/// collide with Mullvad-fork keys.
public enum WarrenAppGroupKey: String, CaseIterable {
    /// Country name of the exit reached after a multi-exit failover.
    /// Cf. `WarrenTunnelEvent.failover(toExit:)`.
    case lastFailoverExit = "WarrenTunnel.lastFailoverExit"

    /// `Date` the failover transition fired. Combined with
    /// [`lastFailoverExit`] to drive the 30 s freshness window in
    /// `WarrenFailoverEvent.isFresh`.
    case lastFailoverAt = "WarrenTunnel.lastFailoverAt"

    /// `Bool`. `true` when the M4.0 HTTP/3 mimicry obfuscation layer
    /// is active on the current tunnel. Drives
    /// `WarrenObfuscationIndicatorView` visibility (M4.0 always-on,
    /// but reserve the flag for future toggle).
    case obfuscationActive = "WarrenTunnel.obfuscationActive"

    /// `Int`. External port mapped through NAT-PMP after a successful
    /// `MAP` request. Cf. `WarrenTunnelEvent.natPmpMapped/.natPmpRenewed`.
    case natPmpExternalPort = "WarrenTunnel.natPmpExternalPort"

    /// `Int`. Cumulative bytes received over the tunnel since the
    /// PacketTunnel extension started. Surfaced by
    /// `WarrenTunnelStatisticsView`.
    case bytesIn = "WarrenTunnel.bytesIn"

    /// `Int`. Cumulative bytes sent over the tunnel since the
    /// PacketTunnel extension started.
    case bytesOut = "WarrenTunnel.bytesOut"

    /// `Int`. Seconds since the active connection was established.
    /// 0 when the tunnel is not currently connected.
    case connectedDurationSeconds = "WarrenTunnel.connectedDurationSeconds"

    /// `Int`. Number of multi-exit failover transitions this session.
    case failoverCount = "WarrenTunnel.failoverCount"

    /// `String`. Localized label for the current tunnel state
    /// ("Connected" / "Reconnecting" / "Disconnected"). Populated by
    /// `WarrenQuinnTunnelImplementation` on every state transition.
    case stateLabel = "WarrenTunnel.stateLabel"

    /// `String`. JSON payload of the last exit-pubkey TOFU mismatch
    /// (`{"exitId","observed","pinned","country"}`) the tunnel extension
    /// recorded on a fail-closed connect. Drives the main app's
    /// `WarrenPubKeyWarningPresenter` alert. Cf.
    /// `WarrenQuinnAdapter.takePinMismatch`.
    case pinMismatch = "WarrenTunnel.pinMismatch"

    /// `Date`. Time the [`pinMismatch`] payload was recorded. Combined
    /// with the payload to drive a freshness window so a stale mismatch
    /// from a previous session does not re-trigger the alert on launch.
    case pinMismatchAt = "WarrenTunnel.pinMismatchAt"
}
