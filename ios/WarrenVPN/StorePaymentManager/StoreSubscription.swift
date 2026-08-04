//
//  StoreSubscription.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import StoreKit

/// The non-renewing "add time" products offered in-app. These are NOT
/// auto-renewable subscriptions: each purchase adds a fixed amount of
/// time to the wallet's subscription, mirroring upstream Mullvad.
///
/// IMPORTANT: every raw value here MUST be created as a Non-Renewing
/// Subscription product in App Store Connect, AND the backend
/// product-id -> duration map (warren-api operator config,
/// `[mobile.apple] products`) MUST list the same product IDs with the
/// matching durations. A product ID present here but absent from the
/// backend map is rejected at credit time (HTTP 422 "unknown product").
enum StoreSubscription: String, CaseIterable {
    case oneMonth = "net.warrenbrowse.vpn.timeadd.1month"
    case threeMonths = "net.warrenbrowse.vpn.timeadd.3months"
    case sixMonths = "net.warrenbrowse.vpn.timeadd.6months"
    case twelveMonths = "net.warrenbrowse.vpn.timeadd.12months"

    /// Number of months credited by this product. Kept in lockstep with
    /// the backend duration map.
    var months: Int {
        switch self {
        case .oneMonth: 1
        case .threeMonths: 3
        case .sixMonths: 6
        case .twelveMonths: 12
        }
    }

    func localizedTitle(displayPrice: String) -> String {
        switch self {
        case .oneMonth:
            String(format: NSLocalizedString("Add 1 month (%@)", comment: ""), displayPrice)
        case .threeMonths:
            String(format: NSLocalizedString("Add 3 months (%@)", comment: ""), displayPrice)
        case .sixMonths:
            String(format: NSLocalizedString("Add 6 months (%@)", comment: ""), displayPrice)
        case .twelveMonths:
            String(format: NSLocalizedString("Add 12 months (%@)", comment: ""), displayPrice)
        }
    }
}

extension Product {
    var customLocalizedTitle: String? {
        StoreSubscription(rawValue: id)?.localizedTitle(displayPrice: displayPrice)
    }
}
