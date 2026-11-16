//
//  HomeTimeLeft.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The time left on the account, as the home footer writes it: "14 days left".
//  Desktop's `formatRemainingTime(expiry, { suffix: true })` and the footer
//  that hides it once the expiry notice takes over.
//

import Foundation

enum HomeTimeLeft {
    /// Within this many days of the expiry the renewal notice speaks instead of the footer.
    static let closeToExpiryDays = 3

    /// The footer line, or nil when there is nothing to say there: no expiry, an expired account,
    /// or one close enough to expiry that the notification area carries the renewal call.
    static func text(now: Date, expiry: Date?, calendar: Calendar = .current) -> String? {
        guard let expiry, expiry > now else { return nil }
        if let threshold = calendar.date(byAdding: .day, value: closeToExpiryDays, to: now), expiry <= threshold {
            return nil
        }
        return remaining(from: now, to: expiry, calendar: calendar)
    }

    /// Whole years from two years on, whole days below that, and less than a day under one.
    static func remaining(from now: Date, to expiry: Date, calendar: Calendar = .current) -> String {
        let days = calendar.dateComponents([.day], from: now, to: expiry).day ?? 0
        if days >= 730 {
            let years = calendar.dateComponents([.year], from: now, to: expiry).year ?? 0
            return String.localizedStringWithFormat(NSLocalizedString("%lld years left", comment: ""), years)
        }
        if days > 0 {
            return String.localizedStringWithFormat(NSLocalizedString("%lld days left", comment: ""), days)
        }
        return NSLocalizedString("Less than a day left", comment: "")
    }
}
