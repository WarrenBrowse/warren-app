//
//  WarrenPinTrustOutcome.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  What the user is told after they trusted an exit's new key and the
//  reconnect did not take.
//
//  Two of these outcomes are worth naming apart, and neither was. A key that
//  changes a SECOND time between the question and the answer is the shape an
//  attack takes, and it used to come back as the same alert with no hint that
//  anything had happened twice. An exit that is simply gone from the roster is
//  the opposite, an ordinary fleet change, and saying "could not trust the key"
//  there sends the user looking for a security problem that is not one.
//
//  Android decides the same four ways in `ConnectScreen`'s trust handler; this
//  is that decision, pure, so it is tested without a tunnel.
//

import Foundation

public enum WarrenPinTrustOutcome: Equatable, Sendable {
    /// The pin was written and the reconnect went out. Nothing to say.
    case dispatched
    /// The pin could not be written at all.
    case saveFailed
    /// The exit served a different key again before the reconnect took.
    case keyChangedAgain
    /// That exit is no longer in the roster.
    case exitGone
    /// The reconnect did not take, and none of the above explains it.
    case trustFailed
}

public enum WarrenPinTrust {
    /// The verdict, in the order the causes exclude each other.
    ///
    /// - Parameters:
    ///   - saved: whether the new pin was written.
    ///   - observedAgain: the key the exit served after the trust, when a
    ///     fresh mismatch arrived for the same exit; nil when none did.
    ///   - trusted: the key the user just trusted.
    ///   - exitStillKnown: whether that exit is still in the roster.
    ///   - reconnected: whether the reconnect took.
    public static func outcome(
        saved: Bool,
        observedAgain: String?,
        trusted: String,
        exitStillKnown: Bool,
        reconnected: Bool
    ) -> WarrenPinTrustOutcome {
        guard saved else { return .saveFailed }
        // A second key outranks everything else: it is the finding, and it is
        // true whether or not the reconnect went out afterwards.
        if let observedAgain, !observedAgain.isEmpty,
            observedAgain.caseInsensitiveCompare(trusted) != .orderedSame
        {
            return .keyChangedAgain
        }
        if reconnected { return .dispatched }
        return exitStillKnown ? .trustFailed : .exitGone
    }

    /// What the alert says for an outcome, or nil when there is nothing to
    /// say because the reconnect went out.
    public static func message(for outcome: WarrenPinTrustOutcome) -> String? {
        switch outcome {
        case .dispatched:
            return nil
        case .saveFailed:
            return NSLocalizedString(
                "Could not save the new key. Please try again.",
                tableName: "Settings",
                comment: ""
            )
        case .keyChangedAgain:
            return NSLocalizedString(
                "The key changed again before it could be trusted. Review the new key and try again.",
                tableName: "Settings",
                comment: ""
            )
        case .exitGone:
            return NSLocalizedString(
                "Could not find that exit server anymore. Try reconnecting.",
                tableName: "Settings",
                comment: ""
            )
        case .trustFailed:
            return NSLocalizedString(
                "Could not trust the new key.",
                tableName: "Settings",
                comment: ""
            )
        }
    }
}
