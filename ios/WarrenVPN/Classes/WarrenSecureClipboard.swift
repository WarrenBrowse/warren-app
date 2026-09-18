//
//  WarrenSecureClipboard.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import UniformTypeIdentifiers

/// Copying a secret to the clipboard.
///
/// A plain `UIPasteboard.general.string = secret` is published to Universal
/// Clipboard, so the recovery phrase a user copies on their phone appears on
/// every Mac and iPad signed into the same Apple account, and survives there
/// until something else is copied. The recovery phrase IS the wallet and the
/// voucher code is a bearer token, so both are written local-only and with an
/// expiry the system enforces even if the app is killed before its own timer
/// fires.
///
/// The account address is deliberately NOT copied through here: it is a public
/// identifier, and a user copying it usually wants to paste it on another
/// device.
enum WarrenSecureClipboard {
    /// How long a copied secret stays on the clipboard.
    static let lifetime: TimeInterval = 60

    static func copy(
        _ secret: String,
        expiresIn seconds: TimeInterval = lifetime,
        to pasteboard: UIPasteboard = .general
    ) {
        pasteboard.setItems(
            [[UTType.utf8PlainText.identifier: secret]],
            options: [
                .localOnly: true,
                .expirationDate: Date().addingTimeInterval(seconds),
            ]
        )
    }

    /// Clears the clipboard if it still holds [secret], so a secret copied and
    /// never pasted does not outlive the screen that offered it. A no-op once
    /// the user has copied something else.
    static func clearIfStillHolding(_ secret: String, in pasteboard: UIPasteboard = .general) {
        if pasteboard.string == secret {
            pasteboard.items = []
        }
    }
}
