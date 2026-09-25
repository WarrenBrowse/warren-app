//
//  WarrenPortForwarding.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  What the port-forwarding screen shows, decided from the mapping snapshot
//  the tunnel extension broadcasts. Pure, so every verdict is tested off any
//  device and off any network; the Android twins are `natPmpStatusLabel`,
//  `natPmpIsPortConflict` and `natPmpPortBlock`.
//

import Foundation
import WarrenSettings

/// The state of the mapping, as the screen reads it.
public enum WarrenPortForwardingState: Equatable, Sendable {
    /// The tunnel is down, so there is nothing to ask an exit for.
    case noTunnel
    /// A request is in flight, or none has resolved yet.
    case requesting
    case mapped(port: Int, renewsIn: TimeInterval?)
    /// The exit refused. `portConflict` is the one refusal the user can act
    /// on, so it is named apart from the rest.
    case failed(portConflict: Bool)
    /// The exit is refusing new allocations for a while longer.
    case rateLimited(remaining: TimeInterval)
    /// The exit refused the mapping as not authorized (warren-core doc 105):
    /// for want of an entitlement, or refusing the one presented. The tunnel
    /// asks again on its own in `retryIn`.
    case refused(noEntitlement: Bool, retryIn: TimeInterval)
}

/// The category the engine reports when the port the user pinned is already
/// taken on this exit. The one refusal with a remedy, hence the only one
/// spelled out here.
public let warrenPortInUseReason = "SuggestedPortInUse"

public enum WarrenPortForwarding {
    /// What the screen shows for `snapshot` at `now`.
    ///
    /// A refusal window outranks a refusal: the request will work again on
    /// its own, so offering the remedies during it would spend the user's
    /// only recovery on a wait.
    static func state(
        snapshot: WarrenNatPmpSnapshot,
        tunnelIsSecured: Bool,
        now: Date
    ) -> WarrenPortForwardingState {
        guard tunnelIsSecured else { return .noTunnel }
        if let remaining = rateLimitRemaining(snapshot: snapshot, now: now) {
            return .rateLimited(remaining: remaining)
        }
        switch snapshot.status {
        case "open":
            guard let port = snapshot.externalPort else { return .requesting }
            return .mapped(port: port, renewsIn: renewCountdown(snapshot: snapshot, now: now))
        case "failed":
            return .failed(portConflict: snapshot.failureReason == warrenPortInUseReason)
        case "refused":
            return .refused(
                noEntitlement: snapshot.refusal != "entitlement_refused",
                retryIn: refusalRetryIn(snapshot: snapshot, now: now)
            )
        default:
            return .requesting
        }
    }

    /// Seconds until the refresh loop renews the mapping. The client renews
    /// at half the granted lifetime (RFC 6886 practice), clamped at zero
    /// while a renewal is in flight.
    static func renewCountdown(snapshot: WarrenNatPmpSnapshot, now: Date) -> TimeInterval? {
        guard let mappedAt = snapshot.mappedAt, let lifetime = snapshot.lifetimeSeconds else {
            return nil
        }
        let renewAt = mappedAt.addingTimeInterval(TimeInterval(lifetime) / 2)
        return max(0, renewAt.timeIntervalSince(now))
    }

    /// Seconds left of the exit's refusal window, or nil once it has run out.
    ///
    /// The window is anchored on when the snapshot arrived, not on when the
    /// screen opened: the exit only says so on a request, so anchoring on
    /// display would keep a spent window alive and strand the controls.
    static func rateLimitRemaining(
        snapshot: WarrenNatPmpSnapshot,
        now: Date
    ) -> TimeInterval? {
        guard snapshot.status == "rate-limited",
            let at = snapshot.rateLimitedAt,
            let seconds = snapshot.retryAfterSeconds
        else {
            return nil
        }
        let remaining = at.addingTimeInterval(TimeInterval(seconds)).timeIntervalSince(now)
        return remaining > 0 ? remaining : nil
    }

    /// Seconds until the tunnel asks again after a refusal, clamped at zero
    /// while the new request is in flight.
    static func refusalRetryIn(snapshot: WarrenNatPmpSnapshot, now: Date) -> TimeInterval {
        guard let at = snapshot.refusedAt, let seconds = snapshot.retryAfterSeconds else { return 0 }
        return max(0, at.addingTimeInterval(TimeInterval(seconds)).timeIntervalSince(now))
    }

    /// Whether the two recoveries from a port conflict may be offered. Both
    /// ask for a new allocation, so they are withheld while the exit is
    /// refusing allocations: the countdown says why.
    public static func offersRecovery(_ state: WarrenPortForwardingState) -> Bool {
        state == .failed(portConflict: true)
    }

    /// The port as typed, or nil when it is not one an exit will consider.
    /// An empty field is a deliberate "let the exit pick", which is 0.
    public static func port(fromInput input: String) -> UInt16? {
        let trimmed = input.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty { return 0 }
        guard let value = UInt16(trimmed),
            WarrenNatPmpSettings.portRange.contains(value)
        else {
            return nil
        }
        return value
    }

    /// The field's text for a stored port. A pin of 0 is "let the exit pick",
    /// which reads as an empty field rather than as a port named zero.
    public static func input(forPort port: UInt16) -> String {
        port == 0 ? "" : "\(port)"
    }

    /// A countdown as "mm:ss", or "hh:mm:ss" past an hour.
    public static func countdown(_ seconds: TimeInterval) -> String {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = seconds >= 3600 ? [.hour, .minute, .second] : [.minute, .second]
        formatter.zeroFormattingBehavior = .pad
        return formatter.string(from: max(0, seconds)) ?? "--:--"
    }
}
