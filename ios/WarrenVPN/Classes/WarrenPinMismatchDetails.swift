//
//  WarrenPinMismatchDetails.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The evidence behind an exit-key change, as the alert renders it.
//

import Foundation

enum WarrenPinMismatchDetails {
    /// A long hex fingerprint as "aabbccdd...11223344", head and tail, so the
    /// alert can show it without wrapping. Returns the input unchanged when it
    /// already fits. The desktop twin is `truncatePubkeyHex`.
    static func truncate(_ hex: String, chars: Int = 8) -> String {
        guard hex.count > 2 * chars else { return hex }
        return "\(hex.prefix(chars))...\(hex.suffix(chars))"
    }

    /// The labelled rows of the details block, in the order the desktop and
    /// Android dialogs show them. `Location` is omitted when the exit reported
    /// no country, matching the `pending.countryCode &&` guard on desktop: an
    /// empty row reads as missing data rather than as unknown.
    static func rows(exitId: String, pinned: String, observed: String, country: String)
        -> [(label: String, value: String)]
    {
        var rows: [(String, String)] = [
            (
                NSLocalizedString("Exit ID", tableName: "Settings", comment: ""),
                truncate(exitId)
            ),
            (
                NSLocalizedString("Previously pinned key", tableName: "Settings", comment: ""),
                truncate(pinned)
            ),
            (
                NSLocalizedString("Newly observed key", tableName: "Settings", comment: ""),
                truncate(observed)
            ),
        ]
        let trimmedCountry = country.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedCountry.isEmpty {
            rows.append((
                NSLocalizedString("Location", tableName: "Settings", comment: ""),
                trimmedCountry
            ))
        }
        return rows
    }
}
