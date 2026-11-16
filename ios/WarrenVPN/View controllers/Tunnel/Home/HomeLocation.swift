//
//  HomeLocation.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The location row of the home: the name it shows and the country whose round
//  flag it shows. The desktop copies are `views/main/home/location-name.ts` and
//  `LocationFlag.tsx`.
//

import CoreGraphics
import Foundation
import WarrenTypes

struct HomeLocation: Equatable {
    let name: String
    /// The ISO code of the country the row pictures with its flag: the exit once a tunnel is up or
    /// coming up, the picked country before that, nil for a pick that names none (automatic, a
    /// custom list), which shows only its name.
    let flagCode: String?

    /// `selectedName` is what the picked location is called while no tunnel is up (the describer's
    /// answer, resolved by the caller from the relay list).
    init(tunnelState: TunnelState, exitLocations: RelayConstraint<UserSelectedRelays>, selectedName: String?) {
        // Once a tunnel is up or coming up the row names the exit, country first; before that, the
        // location the user picked.
        if let exit = tunnelState.relays?.exit.location {
            name = Self.exitPlaceName(country: exit.country, city: exit.city)
            flagCode = exit.countryCode.lowercased()
            return
        }
        switch exitLocations {
        case .any:
            name = NSLocalizedString("Automatic", comment: "")
            flagCode = nil
        case let .only(selection):
            // The describer answers in the relay list's English; a country or
            // city name has its translation in the catalog.
            name =
                selectedName.map { NSLocalizedString($0, comment: "") }
                ?? NSLocalizedString("Select location", comment: "")
            flagCode = Self.countryCode(of: selection)?.lowercased()
        }
    }

    /// The exit, country first, as the location row shows it once a tunnel is up.
    static func exitPlaceName(country: String, city: String) -> String {
        let localCountry = NSLocalizedString(country, comment: "")
        guard !city.isEmpty, city.caseInsensitiveCompare(country) != .orderedSame else {
            return localCountry
        }
        return String(
            format: NSLocalizedString("%@, %@", comment: ""),
            localCountry,
            NSLocalizedString(city, comment: "")
        )
    }

    /// The ISO code of the country the user picked, when the pick names one.
    static func countryCode(of selection: UserSelectedRelays) -> String? {
        guard selection.customListSelection?.isList != true else { return nil }
        switch selection.locations.first {
        case let .country(code), let .city(code, _), let .hostname(code, _, _):
            return code
        case nil:
            return nil
        }
    }
}
