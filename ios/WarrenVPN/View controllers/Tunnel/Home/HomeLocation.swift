//
//  HomeLocation.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The location row of the home: the name it shows and its round picture.
//  The desktop copies are `views/main/home/location-name.ts` and
//  `LocationThumb.tsx`.
//

import CoreGraphics
import Foundation
import WarrenTypes

/// The round picture of a location.
enum HomeLocationThumb: Equatable {
    /// A full-width square of a landscape, from `topRow` of its canvas down.
    case landscape(imageName: String, topRow: CGFloat)
    /// The flag of a country that has no bespoke art, by its ISO code.
    case flag(countryCode: String)
}

struct HomeLocation: Equatable {
    let name: String
    let thumb: HomeLocationThumb

    /// `selectedName` is what the picked location is called while no tunnel is up (the describer's
    /// answer, resolved by the caller from the relay list).
    init(tunnelState: TunnelState, exitLocations: RelayConstraint<UserSelectedRelays>, selectedName: String?) {
        // Once a tunnel is up or coming up the row names the exit, country first; before that, the
        // location the user picked.
        if let exit = tunnelState.relays?.exit.location {
            name = Self.exitPlaceName(country: exit.country, city: exit.city)
            thumb = Self.thumb(country: exit.country, code: exit.countryCode)
            return
        }
        switch exitLocations {
        case .any:
            name = NSLocalizedString("Automatic", comment: "")
            thumb = Self.thumb(country: nil, code: nil)
        case let .only(selection):
            // The describer answers in the relay list's English; a country or
            // city name has its translation in the catalog.
            name =
                selectedName.map { NSLocalizedString($0, comment: "") }
                ?? NSLocalizedString("Select location", comment: "")
            thumb = Self.thumb(country: nil, code: Self.countryCode(of: selection))
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

    /// A square of the country's landscape crowned by its skyline, the flag of a country without
    /// art, the watched plain when no country is named.
    static func thumb(country: String?, code: String?) -> HomeLocationThumb {
        let artKey = [country, code].compactMap { $0 }.first { SceneryViewController.hasCountryArt($0) }
        if artKey == nil, let code, !code.isEmpty {
            return .flag(countryCode: code)
        }
        let imageName = SceneryViewController.landscapeImageName(forCountry: artKey)
        let skyline = SceneryViewController.skylineTopRow(of: imageName)
        return .landscape(imageName: imageName, topRow: SceneryFraming.thumbnailTopRow(skylineTopRow: skyline))
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
