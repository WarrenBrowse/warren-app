//
//  WarrenAppGroupEvents.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Reads transient Warren tunnel events broadcast from the
//  `PacketTunnelProvider` extension via the shared App Group
//  UserDefaults container. Surfacing happens here only — the producer
//  side lives in the tunnel extension (cf.
//  `.planning/c4-packet-tunnel-provider-quinn-design.md` §2.3).
//
//  Until C.4 lands and the tunnel extension actually writes these
//  keys, this observer simply emits no events.
//

import Combine
import Foundation

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

/// Bridge between the tunnel extension's App Group UserDefaults writes
/// and SwiftUI/UIKit consumers in the main app. Owns no UI state — just
/// surfaces decoded events.
@MainActor
public final class WarrenAppGroupEvents: ObservableObject {
    @Published public private(set) var lastFailover: WarrenFailoverEvent?
    @Published public private(set) var obfuscationActive: Bool = false

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
        if let observer {
            NotificationCenter.default.removeObserver(observer)
        }
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
    }
}
