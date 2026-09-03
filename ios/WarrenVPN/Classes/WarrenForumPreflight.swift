//
//  WarrenForumPreflight.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Whether a wallet-signed forum request may leave the device now, from the
//  tunnel's state alone. The rule is the Android `ForumPreflight`, mirrored
//  here and pinned on both sides by
//  `fixtures/client-rules/forum_preflight.json`.
//

import Foundation

/// The verdict on a forum request that is about to leave.
enum ForumPreflight: Equatable {
    /// Nothing in the tunnel's way: sign and send.
    case proceed

    /// Not now. `tunnelClass` names the state class for the log and the
    /// report (`connecting`, `reconnecting`, `disconnecting`, `blocking`),
    /// never a relay, an endpoint or an engine reason.
    case deferred(tunnelClass: String)
}

/// The tunnel states a forum request may leave in.
///
/// The request goes to the connect broker over the ordinary network stack,
/// but the broker's host name is resolved by the system resolver, which no
/// protector covers. While the tunnel is coming up or going down the resolver
/// points at the tunnel's DNS and the lookup times out; while the packet
/// tunnel holds the blocked state there is no reachable resolver at all and
/// it fails at once. Those are the states that defer, and a deferred attempt
/// signs nothing, so no nonce and no session is spent on a request that
/// cannot arrive.
enum WarrenForumPreflight {
    static func verdict(for state: TunnelState) -> ForumPreflight {
        switch state {
        case .disconnected, .connected:
            // Settled, in either direction: the resolver in force answers.
            .proceed
        case .connecting, .negotiatingEphemeralPeer:
            .deferred(tunnelClass: "connecting")
        case .reconnecting, .pendingReconnect:
            .deferred(tunnelClass: "reconnecting")
        case let .disconnecting(actionAfterDisconnect):
            // A teardown that is the first half of a reconnect is named for
            // where it is going, as on Android.
            switch actionAfterDisconnect {
            case .nothing: .deferred(tunnelClass: "disconnecting")
            case .reconnect: .deferred(tunnelClass: "reconnecting")
            }
        case .error:
            // The blocked state: the packet tunnel is installed and dropping
            // everything, which is Android's `blocking`.
            .deferred(tunnelClass: "blocking")
        case let .waitingForConnectivity(reason):
            switch reason {
            case .noConnection:
                // The tunnel is up and secured while its connection is down,
                // so the lookup still goes to the tunnel's DNS.
                .deferred(tunnelClass: "blocking")
            case .noNetwork:
                // The device has no network at all. Deferring would tell the
                // person to wait for a tunnel; letting it go gives them the
                // transport failure, which is what is actually wrong.
                .proceed
            }
        }
    }
}
