//
//  WarrenBetaExplanation.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  What a non-production build tells its user about the network it is on.
//
//  The header already carried a "BETA" chip, and it said only that. A user on
//  the free beta had no way to learn that the speed they get is the network's
//  cap rather than their own line, or that these are not the conditions of the
//  paid product. Desktop says both in `BetaBadge`, and Android in its own beta
//  card; this is the same text, assembled here so it is tested off-device.
//

import Foundation
import WarrenRustRuntime

public enum WarrenBetaExplanation {
    /// The one line under the chip. Names the cap when the server gave one,
    /// and says "limited" when it did not: a number nobody can check is worse
    /// than none.
    public static func summary(networkInfo: WarrenNetworkInfo?) -> String {
        guard let mbps = networkInfo?.capMbps else {
            return NSLocalizedString(
                "Free beta network, limited bandwidth",
                tableName: "Settings",
                comment: ""
            )
        }
        return String(
            format: NSLocalizedString(
                "Free beta network, speed capped at %d Mbps",
                tableName: "Settings",
                comment: ""
            ),
            mbps
        )
    }

    /// The explanation itself, one paragraph per idea, in the order desktop
    /// states them: what this build is, what it costs in speed, and that the
    /// conditions are temporary.
    public static func paragraphs(networkInfo: WarrenNetworkInfo?) -> [String] {
        [
            NSLocalizedString(
                "This app uses the free Warren beta, here to help us validate Warren in real conditions.",
                tableName: "Settings",
                comment: ""
            ),
            capSentence(networkInfo: networkInfo),
            NSLocalizedString(
                "These are not the final service conditions: the full-speed network is a separate, paid product.",
                tableName: "Settings",
                comment: ""
            ),
        ]
    }

    public static func title() -> String {
        NSLocalizedString("Warren beta", tableName: "Settings", comment: "")
    }

    private static func capSentence(networkInfo: WarrenNetworkInfo?) -> String {
        guard let mbps = networkInfo?.capMbps else {
            return NSLocalizedString(
                "It runs on a separate network with limited bandwidth.",
                tableName: "Settings",
                comment: ""
            )
        }
        return String(
            format: NSLocalizedString(
                "It runs on a separate network with bandwidth capped at %d Mbps.",
                tableName: "Settings",
                comment: ""
            ),
            mbps
        )
    }
}
