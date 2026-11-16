//
//  IncreasedHitButton.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-05-16.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI
import UIKit

final class IncreasedHitButton: UIButton {
    private let defaultSize = UIMetrics.Button.minimumTappableAreaSize.width

    /// When set, the button fills its bounds with this colour while pressed or focused (the home
    /// toolbar's squares); nil leaves the background alone.
    var pressedTint: UIColor? {
        didSet {
            if pressedTint == nil, oldValue != nil {
                backgroundColor = nil
            }
            applyPressedTint()
        }
    }

    override var isHighlighted: Bool {
        didSet { applyPressedTint() }
    }

    override func didUpdateFocus(in context: UIFocusUpdateContext, with coordinator: UIFocusAnimationCoordinator) {
        super.didUpdateFocus(in: context, with: coordinator)
        applyPressedTint()
    }

    private func applyPressedTint() {
        guard let pressedTint else { return }
        backgroundColor = isHighlighted || isFocused ? pressedTint : nil
    }

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        let width = bounds.width
        let height = bounds.height
        let dx = (max(defaultSize, width) - width) * 0.5
        let dy = (max(defaultSize, height) - height) * 0.5
        return bounds.insetBy(dx: -dx, dy: -dy).contains(point)
    }
}
