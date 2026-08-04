//
//  WarrenAppGroupEvents.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Reads transient Warren tunnel events broadcast from the
//  `PacketTunnelProvider` extension via the shared App Group
//  UserDefaults container. Surfacing happens here only - the producer
//  side lives in the tunnel extension.
//
//  Until the tunnel extension actually writes these
//  keys, this observer simply emits no events.
//

import Combine
import Foundation
import WarrenRustRuntime

// `WarrenAppGroupKey` lives in `Shared/WarrenAppGroupKey.swift` so the
// PacketTunnel extension (producer) and the main app (consumer) share
// a single source of truth. Keys cannot drift silently any more.

public struct WarrenFailoverEvent: Equatable {
    public let country: String
    public let occurredAt: Date

    /// Considered fresh if it happened within the last 30 seconds.
    /// Older events are kept in UserDefaults for diagnostics but should
    /// not re-trigger the UI banner on subsequent app launches.
    public var isFresh: Bool {
        Date().timeIntervalSince(occurredAt) < 30
    }
}

/// An exit-pubkey TOFU mismatch surfaced from the tunnel extension. The
/// connection already failed closed; this drives the Trust / Report /
/// Reject alert in the main app.
public struct WarrenPinMismatchEvent: Equatable {
    public let mismatch: WarrenPinMismatch
    public let occurredAt: Date

    /// Fresh within the last 2 minutes. A pin mismatch is a security
    /// decision the user should make promptly, but the window is wider
    /// than the failover banner's because the alert is modal (the user may
    /// need a moment to read it) and we do not want a stale mismatch from a
    /// previous session re-surfacing on launch.
    public var isFresh: Bool {
        Date().timeIntervalSince(occurredAt) < 120
    }
}

/// Bridge between the tunnel extension's App Group UserDefaults writes
/// and SwiftUI/UIKit consumers in the main app. Owns no UI state - just
/// surfaces decoded events.
@MainActor
public final class WarrenAppGroupEvents: ObservableObject {
    @Published public private(set) var lastFailover: WarrenFailoverEvent?
    @Published public private(set) var obfuscationActive: Bool = false
    @Published public private(set) var lastPinMismatch: WarrenPinMismatchEvent?

    private let defaults: UserDefaults?
    private var observer: NSObjectProtocol?

    public init(suiteName: String) {
        defaults = UserDefaults(suiteName: suiteName)
        refresh()
        observer = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: defaults,
            queue: .main
        ) { [weak self] _ in
            // Hop back onto the main actor; the observer's queue is .main
            // but Swift Concurrency still requires the explicit isolation.
            Task { @MainActor [weak self] in
                self?.refresh()
            }
        }
    }

    deinit {
        // NotificationCenter automatically drops weak observer references
        // when the observed object is deallocated. The explicit unregister
        // would require accessing the @MainActor-isolated `observer`
        // ivar from a nonisolated deinit - disallowed under Swift 6
        // strict concurrency. The block-based observer holds `self`
        // weakly, so leaking is bounded.
    }

    private func refresh() {
        guard let defaults else { return }
        if let country = defaults.string(forKey: WarrenAppGroupKey.lastFailoverExit.rawValue),
            let date = defaults.object(forKey: WarrenAppGroupKey.lastFailoverAt.rawValue) as? Date
        {
            let event = WarrenFailoverEvent(country: country, occurredAt: date)
            if event != lastFailover {
                lastFailover = event
            }
        } else if lastFailover != nil {
            lastFailover = nil
        }

        let obfActive = defaults.bool(forKey: WarrenAppGroupKey.obfuscationActive.rawValue)
        if obfActive != obfuscationActive {
            obfuscationActive = obfActive
        }

        if let json = defaults.string(forKey: WarrenAppGroupKey.pinMismatch.rawValue),
            let date = defaults.object(forKey: WarrenAppGroupKey.pinMismatchAt.rawValue) as? Date,
            let data = json.data(using: .utf8),
            let mismatch = try? JSONDecoder().decode(WarrenPinMismatch.self, from: data)
        {
            let event = WarrenPinMismatchEvent(mismatch: mismatch, occurredAt: date)
            if event != lastPinMismatch {
                lastPinMismatch = event
            }
        } else if lastPinMismatch != nil {
            lastPinMismatch = nil
        }
    }

    /// Clear the persisted pin-mismatch payload so the alert does not
    /// re-surface (called after the user resolves it: Trust / Report /
    /// Reject). Leaves the failover/obfuscation keys untouched.
    public func clearPinMismatch() {
        defaults?.removeObject(forKey: WarrenAppGroupKey.pinMismatch.rawValue)
        defaults?.removeObject(forKey: WarrenAppGroupKey.pinMismatchAt.rawValue)
        if lastPinMismatch != nil {
            lastPinMismatch = nil
        }
    }
}
