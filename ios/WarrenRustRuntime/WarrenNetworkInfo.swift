//
//  WarrenNetworkInfo.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  What the network this build talks to says about itself: its label, whether
//  it is deliberately degraded, the default bandwidth cap and whether payments
//  are offered here.
//
//  Display data, and unauthenticated: the enforcement lives on the exits. It
//  is what lets the app tell a user that the speed they are getting is the
//  beta's cap rather than their own line.
//

import Foundation

public struct WarrenNetworkInfo: Equatable, Sendable {
    /// Environment name the server reports, e.g. `beta` or `production`.
    public let environment: String
    /// True when the service is deliberately degraded, which is what the beta
    /// is.
    public let degraded: Bool
    /// Default per-subscriber bandwidth cap in bits per second, absent when
    /// there is no cap.
    public let defaultRateBps: UInt64?
    /// False where payment flows must not be offered.
    public let paymentsEnabled: Bool

    public init(
        environment: String,
        degraded: Bool,
        defaultRateBps: UInt64?,
        paymentsEnabled: Bool
    ) {
        self.environment = environment
        self.degraded = degraded
        self.defaultRateBps = defaultRateBps
        self.paymentsEnabled = paymentsEnabled
    }

    /// The cap as the badge states it, rounded to whole megabits per second.
    /// Nil when the server named no cap, which reads as "limited bandwidth"
    /// rather than as a number nobody can check.
    public var capMbps: Int? {
        guard let defaultRateBps, defaultRateBps > 0 else { return nil }
        return Int((Double(defaultRateBps) / 1_000_000).rounded())
    }
}

public enum WarrenNetworkInfoClient {
    /// Fetches `GET /v1/network`. Blocking (the GET runs in Rust): call off
    /// the main thread. Nil for any failure, including an API that predates
    /// the endpoint.
    public static func fetch() -> WarrenNetworkInfo? {
        guard let raw = warren_fetch_network_info() else { return nil }
        defer { warren_wallet_free_mnemonic(raw) }
        return info(fromEnvelope: String(cString: raw))
    }

    /// `{"ok":true,...}` or `{"ok":false}`. Anything that is not an explicit
    /// `ok` is no info, so a half-read envelope can never be shown as a fact
    /// about the network. Pure, so it is tested off-device.
    static func info(fromEnvelope envelope: String?) -> WarrenNetworkInfo? {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            object["ok"] as? Bool == true,
            let environment = object["environment"] as? String
        else {
            return nil
        }
        return WarrenNetworkInfo(
            environment: environment,
            degraded: object["degraded"] as? Bool == true,
            defaultRateBps: (object["default_rate_bps"] as? NSNumber)?.uint64Value,
            paymentsEnabled: object["payments_enabled"] as? Bool == true
        )
    }
}
