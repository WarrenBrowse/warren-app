//
//  WarrenAccountStanding.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The wallet's port-forward abuse standing (warren-core doc 105 §5.4), as the
//  Rust store answers it, and the signed poll that fetches it. The store, the
//  envelope and which strikes were already announced live in the shared
//  `warren-standing` crate, so iOS and Android cannot drift on either; this
//  only reads the envelope.
//

import Foundation
import WarrenRustRuntimeProxy

/// One strike on the account.
///
/// It names the port that was closed and the case reference to quote when
/// contesting it, both of which belong on the account's own screens and
/// nowhere else: `description` renders neither, so a log line that prints a
/// strike prints no case.
public struct WarrenAccountStrike: Equatable, Sendable, CustomStringConvertible {
    /// The day of the strike, midnight UTC.
    public let day: Date
    /// The category of the report, as the API names it (`copyright`,
    /// `malware_c2`, `spam`, `scanning`, `phishing`, `csam`, `other`).
    public let category: String
    public let exitCountry: String?
    /// The forwarded public port that was closed.
    public let port: Int
    public let caseReference: String

    public init(day: Date, category: String, exitCountry: String?, port: Int, caseReference: String) {
        self.day = day
        self.category = category
        self.exitCountry = exitCountry
        self.port = port
        self.caseReference = caseReference
    }

    /// The key a dismissed banner is remembered by: a digest of the case
    /// reference (FNV-1a), so the preferences name no case.
    public var dismissalKey: String {
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in caseReference.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x0000_0100_0000_01b3
        }
        return "strike:" + String(hash, radix: 16)
    }

    public var description: String {
        "WarrenAccountStrike(day: \(day), category: \(category))"
    }
}

/// A ban on the wallet.
public struct WarrenAccountBan: Equatable, Sendable {
    /// Banned for port-forwarding abuse, rather than any other reason.
    public let portForwarding: Bool
    /// When it lapses on its own, `nil` when the source did not say.
    public let lapsesAt: Date?
    /// Whether it held when the standing was read.
    public let inForce: Bool

    public init(portForwarding: Bool, lapsesAt: Date?, inForce: Bool) {
        self.portForwarding = portForwarding
        self.lapsesAt = lapsesAt
        self.inForce = inForce
    }
}

/// A strike to warn about: "warning `ordinal` of `threshold`".
public struct WarrenStrikeNotice: Equatable, Sendable {
    public let strike: WarrenAccountStrike
    /// Its rank among the live strikes, from 1.
    public let ordinal: Int
    /// The live strikes that ban the account, `0` while unknown.
    public let threshold: Int

    public init(strike: WarrenAccountStrike, ordinal: Int, threshold: Int) {
        self.strike = strike
        self.ordinal = ordinal
        self.threshold = threshold
    }
}

/// The standing: live strikes oldest first, the threshold, the window and the
/// ban.
public struct WarrenAccountStanding: Equatable, Sendable {
    public let strikes: [WarrenAccountStrike]
    public let threshold: Int
    public let windowDays: Int
    public let ban: WarrenAccountBan?

    public init(strikes: [WarrenAccountStrike], threshold: Int, windowDays: Int, ban: WarrenAccountBan?) {
        self.strikes = strikes
        self.threshold = threshold
        self.windowDays = windowDays
        self.ban = ban
    }

    /// The newest strike with its rank, `nil` without one.
    public var latestStrike: WarrenStrikeNotice? {
        strikes.last.map { WarrenStrikeNotice(strike: $0, ordinal: strikes.count, threshold: threshold) }
    }
}

/// One poll, as the envelope of `warren_account_standing` answers it.
public struct WarrenStandingPoll: Equatable, Sendable {
    /// False on a failure other than a `404`: nothing is published.
    public let ok: Bool
    /// False on a `404`, an API that does not report the standing yet.
    public let reported: Bool
    public let standing: WarrenAccountStanding?
    /// The strikes this device has not warned about yet, each handed over
    /// exactly once: Rust remembers across launches which it already did.
    public let newStrikes: [WarrenStrikeNotice]

    /// Strikes read from one answer, at most. Three ban the account, so a
    /// longer list is a broken or hostile answer, and neither the screen nor
    /// the notifications should grow with it.
    public static let maxStrikes = 10

    /// Reads the `{"ok","reported","standing","new_strikes"}` envelope, `nil`
    /// when it is not one. A malformed strike is dropped rather than failing
    /// the whole standing.
    public static func parse(envelope: String?) -> WarrenStandingPoll? {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return nil
        }
        let newStrikes = (root["new_strikes"] as? [[String: Any]] ?? []).prefix(maxStrikes).compactMap {
            row -> WarrenStrikeNotice? in
            guard let strike = (row["strike"] as? [String: Any]).flatMap(strike(from:)),
                let ordinal = row["ordinal"] as? Int
            else {
                return nil
            }
            return WarrenStrikeNotice(strike: strike, ordinal: ordinal, threshold: row["threshold"] as? Int ?? 0)
        }
        return WarrenStandingPoll(
            ok: root["ok"] as? Bool ?? false,
            reported: root["reported"] as? Bool ?? true,
            standing: (root["standing"] as? [String: Any]).map(standing(from:)),
            newStrikes: newStrikes
        )
    }

    private static func standing(from object: [String: Any]) -> WarrenAccountStanding {
        let ban = (object["ban"] as? [String: Any]).map { ban in
            WarrenAccountBan(
                portForwarding: ban["reason"] as? String == "port_forwarding_abuse",
                lapsesAt: (ban["lapses_at_unix_secs"] as? NSNumber).map {
                    Date(timeIntervalSince1970: $0.doubleValue)
                },
                inForce: ban["in_force"] as? Bool ?? true
            )
        }
        return WarrenAccountStanding(
            strikes: (object["strikes"] as? [[String: Any]] ?? []).prefix(maxStrikes).compactMap(strike(from:)),
            threshold: object["threshold"] as? Int ?? 0,
            windowDays: object["window_days"] as? Int ?? 0,
            ban: ban
        )
    }

    private static func strike(from object: [String: Any]) -> WarrenAccountStrike? {
        guard let reference = object["case_reference"] as? String, !reference.isEmpty,
            let port = object["port"] as? Int,
            let day = object["day_unix_secs"] as? NSNumber
        else {
            return nil
        }
        return WarrenAccountStrike(
            day: Date(timeIntervalSince1970: day.doubleValue),
            category: object["category"] as? String ?? "other",
            exitCountry: object["exit_country"] as? String,
            port: port,
            caseReference: reference
        )
    }
}

extension WarrenAccountClient {
    /// Signed `GET /v1/account/standing`, with the strike ledger kept in
    /// `ledgerDirectory`, the app's own container. `nil` when the call could
    /// not even be made. Blocking: run off the main thread. The seed is never
    /// logged, nor are the case references and ports the answer carries.
    public static func accountStanding(seed: Data, ledgerDirectory: URL) -> WarrenStandingPoll? {
        guard seed.count == 32 else { return nil }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return ledgerDirectory.path.withCString { dirPtr in
                warren_account_standing(base, dirPtr)
            }
        }
        guard let raw else { return nil }
        defer { warren_wallet_free_mnemonic(raw) }
        return WarrenStandingPoll.parse(envelope: String(cString: raw))
    }

    /// The wallet left this device: its standing and which of its strikes
    /// were announced are forgotten, the ledger in `ledgerDirectory` too.
    public static func forgetAccountStanding(ledgerDirectory: URL) {
        ledgerDirectory.path.withCString { warren_account_standing_forget($0) }
    }
}
