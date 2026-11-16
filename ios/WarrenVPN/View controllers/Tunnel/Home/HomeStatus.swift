//
//  HomeStatus.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  What the home status block says: the pill, the title and the line under it.
//  The desktop copy is `views/main/home/home-status-text.ts`; the phase is
//  `ConnectionPhase`, shared with the scenery.
//

import Foundation
import WarrenREST

struct HomeStatus: Equatable {
    let pill: String
    let title: String
    let subtitle: String
    /// The line under a protected title is the exit host, set in the mono face.
    let subtitleIsHostname: Bool

    init(pill: String, title: String, subtitle: String, subtitleIsHostname: Bool = false) {
        self.pill = pill
        self.title = title
        self.subtitle = subtitle
        self.subtitleIsHostname = subtitleIsHostname
    }

    init(tunnelState: TunnelState) {
        let phase = ConnectionPhase(tunnelState: tunnelState)
        let pill = Self.pillLabel(phase)
        let relays = tunnelState.relays

        switch phase {
        case .exposed:
            self.init(pill: pill, title: Self.visible, subtitle: Self.notEncrypted)
        case .blocked:
            self.init(pill: pill, title: NSLocalizedString("Connection blocked", comment: ""), subtitle: "")
        case .interrupted:
            self.init(
                pill: pill,
                title: NSLocalizedString("Connection interrupted", comment: ""),
                subtitle: Self.youAreProtected
            )
        case .connecting:
            if case .disconnecting = tunnelState {
                self.init(pill: pill, title: Self.visible, subtitle: NSLocalizedString("Disconnecting...", comment: ""))
                return
            }
            if let place = relays.map({ Self.placeName(country: $0.exit.location.country, city: $0.exit.location.city) }
            ) {
                self.init(pill: pill, title: place, subtitle: Self.inProgress)
            } else {
                self.init(pill: pill, title: Self.inProgress, subtitle: "")
            }
        case .protected:
            guard let relays else {
                self.init(pill: pill, title: Self.youAreProtected, subtitle: "")
                return
            }
            let place = Self.placeName(country: relays.exit.location.country, city: relays.exit.location.city)
            self.init(pill: pill, title: place, subtitle: Self.hostname(relays), subtitleIsHostname: true)
        }
    }

    static func pillLabel(_ phase: ConnectionPhase) -> String {
        switch phase {
        case .exposed: NSLocalizedString("Unprotected", comment: "Home status badge: traffic leaves unencrypted.")
        case .connecting: NSLocalizedString("Connecting", comment: "Home status badge: the tunnel is coming up.")
        case .protected: NSLocalizedString("Protected", comment: "Home status badge: the tunnel is up.")
        case .interrupted:
            NSLocalizedString("Interrupted", comment: "Home status badge: tunnel up, no network.")
        case .blocked: NSLocalizedString("Blocked", comment: "Home status badge: the kill switch holds all traffic.")
        }
    }

    /// City first, the way a place is named on a ticket: "Helsinki, Finland". A city-state is named
    /// once.
    static func placeName(country: String, city: String) -> String {
        let localCountry = NSLocalizedString(country, comment: "")
        guard !city.isEmpty, city.caseInsensitiveCompare(country) != .orderedSame else {
            return localCountry
        }
        return String(
            format: NSLocalizedString("%@, %@", comment: ""),
            NSLocalizedString(city, comment: ""),
            localCountry
        )
    }

    /// The exit host, and the entry it is reached through on a two-hop circuit.
    static func hostname(_ relays: SelectedRelays) -> String {
        guard let entry = relays.entry?.hostname else { return relays.exit.hostname }
        return String(format: NSLocalizedString("%@ via %@", comment: ""), relays.exit.hostname, entry)
    }

    private static var visible: String { NSLocalizedString("You are visible", comment: "") }
    private static var notEncrypted: String { NSLocalizedString("Your connection is not encrypted", comment: "") }
    private static var inProgress: String { NSLocalizedString("Connection in progress", comment: "") }
    private static var youAreProtected: String { NSLocalizedString("You are protected", comment: "") }
}
