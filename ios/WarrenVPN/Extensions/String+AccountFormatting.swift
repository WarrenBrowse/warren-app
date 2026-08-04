//
//  String+AccountFormatting.swift
//  MullvadVPN
//
//  Created by Andreas Lif on 2022-06-10.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

extension String {
    /// Legacy Mullvad numeric account-number formatting: groups the
    /// digits into space-separated blocks of four. Still used by the
    /// legacy account-number surfaces (`AccountNumberRow`,
    /// `AccountInputGroupView`, deletion / welcome screens). NOT used
    /// for the Warren wallet address, whose canonical short form is
    /// `shortWarrenAddress`.
    var formattedAccountNumber: String {
        split(every: 4).joined(separator: " ")
    }

    /// Compact display form of a Warren SS58 wallet address: the first 6
    /// characters, an ellipsis (`…`, U+2026), then the last 6
    /// characters (e.g. `wb7kgy…hP9DnB`). Returns the string unchanged
    /// when it is 13 characters or shorter (too short to abbreviate
    /// without losing information). The full address - never this short
    /// form - must be used for copy / share.
    var shortWarrenAddress: String {
        guard count > 13 else { return self }
        return "\(prefix(6))\u{2026}\(suffix(6))"
    }

    /// Lightweight, non-authoritative check that `self` looks like a
    /// Warren SS58 wallet address. This is a fast client-side guard for
    /// the login / import field only - it does NOT validate the SS58
    /// checksum. Authoritative validation happens in the Rust layer
    /// (`warren_identity::ss58::decode`, reached via
    /// `WarrenWallet.fromMnemonic` / address import).
    ///
    /// The check: non-empty, `wb` prefix, length 47…49, and every
    /// character in the base58 alphabet `[1-9A-HJ-NP-Za-km-z]`
    /// (excludes `0`, `O`, `I`, `l`).
    var isWarrenAddress: Bool {
        guard hasPrefix("wb"), (47...49).contains(count) else { return false }
        let base58 = CharacterSet(charactersIn:
            "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")
        return unicodeScalars.allSatisfy { base58.contains($0) }
    }
}
