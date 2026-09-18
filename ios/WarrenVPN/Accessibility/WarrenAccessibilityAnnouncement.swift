//
//  WarrenAccessibilityAnnouncement.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import SwiftUI
import UIKit

/// Speaks a status or error line that appears without focus moving.
///
/// SwiftUI has no live region. `.accessibilityAddTraits(.updatesFrequently)` is
/// often reached for and does not announce anything: it tells assistive
/// technology a value changes often, which is a hint for polling, not a request
/// to speak. Screens carrying progress and error text under a button therefore
/// read as silent to VoiceOver while a sighted user watches them change.
///
/// `assertive` interrupts whatever is being spoken, for something that went
/// wrong; the polite tier waits its turn, for progress.
private struct WarrenAnnouncementModifier: ViewModifier {
    let message: String?
    let assertive: Bool

    func body(content: Content) -> some View {
        content.onChange(of: message) { _, newValue in
            guard let newValue, !newValue.isEmpty else { return }
            // The same guard `TunnelStateAccessibilityAnnouncer` uses: posting
            // with VoiceOver off is wasted work.
            guard UIAccessibility.isVoiceOverRunning else { return }
            UIAccessibility.post(notification: .announcement, argument: argument(for: newValue))
        }
    }

    private func argument(for message: String) -> Any {
        guard assertive else { return message }
        return NSAttributedString(
            string: message,
            attributes: [
                .accessibilitySpeechAnnouncementPriority: UIAccessibilityPriority.high.rawValue
            ]
        )
    }
}

extension View {
    /// Announce `message` when it becomes non-nil. See the modifier's note on
    /// why `.updatesFrequently` does not do this.
    func announce(_ message: String?, assertive: Bool = false) -> some View {
        modifier(WarrenAnnouncementModifier(message: message, assertive: assertive))
    }
}
