//
//  AlertPresentation.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2023-08-23.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import Routing

struct AlertMetadata {
    let presentation: AlertPresentation
    let context: Presenting
}

struct AlertAction {
    let title: String
    let style: AlertActionStyle
    var accessibilityId: AccessibilityIdentifier?
    /// When true this button stays disabled until the alert's `checkbox` is
    /// ticked. Used to gate a destructive confirmation (desktop parity: the
    /// logout button unlocks only once the user confirms they backed up).
    var isGatedByCheckbox: Bool = false
    var handler: (() -> Void)?
    var interactiveHandler: ((AlertViewController, AppButton) -> Void)?
}

/// A mandatory-acknowledgement checkbox rendered above the buttons. While
/// unticked, every `AlertAction` flagged `isGatedByCheckbox` stays disabled.
struct AlertCheckbox {
    let title: String
    var accessibilityId: AccessibilityIdentifier?
}

struct AlertPresentation: Identifiable, CustomDebugStringConvertible {
    let id: String

    var accessibilityIdentifier: AccessibilityIdentifier?
    var header: String?
    var icon: AlertIcon?
    var title: String?
    var message: String?
    var attributedMessage: NSAttributedString?
    var checkbox: AlertCheckbox?
    let buttons: [AlertAction]

    var debugDescription: String {
        return id
    }
}

extension AlertPresentation: Equatable, Hashable {
    func hash(into hasher: inout Hasher) {
        hasher.combine(id)
    }

    static func == (lhs: AlertPresentation, rhs: AlertPresentation) -> Bool {
        return lhs.id == rhs.id
    }
}
