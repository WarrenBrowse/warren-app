//
//  WarrenIncidentReport.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation

/// Whether an incident report reached the operator feed, and if not, the coarse
/// class the shared crate named.
///
/// The class never carries a value the report would have held: a gap in the
/// feed is diagnosed from the class alone, which is the no-log rule the Rust
/// side states for the same enum.
public enum WarrenIncidentOutcome: Equatable, Sendable {
    case sent
    case notSent(reason: String)

    public var didSend: Bool {
        if case .sent = self { return true }
        return false
    }
}

/// The signed reports a client files so an incident it suffered is visible to
/// the operator.
///
/// Only the pubkey mismatch for now, the one a user decides on: the exit
/// key-change alert's "Report to Warren" button used to open a static FAQ page,
/// so the report it names never left the device.
public enum WarrenIncidentReport {
    /// Files the pubkey-mismatch report for an exit that served a key other
    /// than the pinned one.
    ///
    /// Blocking: the Rust side POSTs on the shared runtime, so call this off
    /// the main thread. `seed` is the 32-byte wallet signing seed, which the
    /// FFI copies and zeroes; it is never logged on either side.
    public static func pubkeyMismatch(
        seed: Data,
        exitIdHex: String,
        oldPubkeyHex: String,
        newPubkeyHex: String,
        countryCode: String,
        city: String
    ) -> WarrenIncidentOutcome {
        guard seed.count == 32 else { return .notSent(reason: "identity") }
        let raw: UnsafeMutablePointer<CChar>? = seed.withUnsafeBytes { seedBytes in
            exitIdHex.withCString { exitPtr in
                oldPubkeyHex.withCString { oldPtr in
                    newPubkeyHex.withCString { newPtr in
                        countryCode.withCString { countryPtr in
                            city.withCString { cityPtr in
                                warren_report_pubkey_mismatch(
                                    seedBytes.bindMemory(to: UInt8.self).baseAddress,
                                    exitPtr,
                                    oldPtr,
                                    newPtr,
                                    countryPtr,
                                    cityPtr
                                )
                            }
                        }
                    }
                }
            }
        }
        guard let raw else { return .notSent(reason: "unknown") }
        // The type-agnostic free routine every CString this crate produces
        // takes, as the FFI's own doc comment states.
        defer { warren_wallet_free_mnemonic(raw) }
        return outcome(fromEnvelope: String(cString: raw))
    }

    /// Maps the shared envelope to an outcome. `{"ok":true}` is a send;
    /// anything else, including an envelope that cannot be read, is a failure,
    /// so a broken envelope can never read as success. Pure, so it is tested
    /// off-device.
    static func outcome(fromEnvelope envelope: String?) -> WarrenIncidentOutcome {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return .notSent(reason: "unknown")
        }
        if object["ok"] as? Bool == true {
            return .sent
        }
        return .notSent(reason: object["reason"] as? String ?? "unknown")
    }
}
