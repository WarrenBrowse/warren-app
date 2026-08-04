//
//  ApplicationConfiguration.swift
//  MullvadVPN
//
//  Created by pronebird on 05/06/2019.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import Network

enum ApplicationConfiguration {
    static var hostName: String {
        Bundle.main.object(forInfoDictionaryKey: "HostName") as! String
    }

    /// Shared container security group identifier.
    static var securityGroupIdentifier: String {
        Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as! String
    }

    /// Container URL for security group.
    static var containerURL: URL {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: securityGroupIdentifier)!
    }

    /// Returns URL for new log file associated with application target and located within the specified container.
    static func newLogFileURL(for target: ApplicationTarget, in containerURL: URL) -> URL {
        containerURL.appendingPathComponent(
            "\(target.bundleIdentifier)_\(Date().logFileFormatted).log",
            isDirectory: false
        )
    }

    /// Returns URLs for log files associated with application target and located within the specified container.
    static func logFileURLs(for target: ApplicationTarget, in containerURL: URL) -> [URL] {
        let fileManager = FileManager.default
        let filePathsInDirectory = try? fileManager.contentsOfDirectory(atPath: containerURL.relativePath)

        let filteredFilePaths: [URL] =
            filePathsInDirectory?.compactMap { path in
                let pathIsLog = path.split(separator: ".").last == "log"
                // Pattern should be "<Target Bundle ID>_", eg. "com.warrenbrowse.vpn.ios_".
                let pathBelongsToTarget = path.contains("\(target.bundleIdentifier)_")

                return pathIsLog && pathBelongsToTarget ? containerURL.appendingPathComponent(path) : nil
            } ?? []

        let sortedFilePaths = try? filteredFilePaths.sorted { path1, path2 in
            let path1Attributes = try fileManager.attributesOfItem(atPath: path1.relativePath)
            let date1 = (path1Attributes[.creationDate] as? Date) ?? Date.distantPast

            let path2Attributes = try fileManager.attributesOfItem(atPath: path2.relativePath)
            let date2 = (path2Attributes[.creationDate] as? Date) ?? Date.distantPast

            return date1 > date2
        }

        return sortedFilePaths ?? []
    }

    // Maximum file size for writing and reading logs.
    static let logMaximumFileSize: UInt64 = 131_072  // 128 kB.

    // The website auto-detects the browser language, so the URLs below carry
    // no language path segment; the `language` parameters are kept for call
    // sites but unused.

    /// Privacy policy URL.
    static func privacyPolicyLink(for _: String) -> String {
        "https://warren.ro/confidentialite"
    }

    /// Privacy first-steps guide URL.
    static func privacyGuidesURL(for _: String) -> URL {
        URL(string: "https://warren.ro/no-log")!
    }

    /// FAQ & Guides URL.
    static func faqAndGuidesURL(for _: String) -> URL {
        URL(string: "https://warren.ro/faq")!
    }

    /// Public help page (guest triage form, no account needed). Shown
    /// under onboarding and login errors, matching the desktop app.
    static var helpURL: URL {
        URL(string: "https://warren.ro/aide")!
    }

    /// Maximum number of simultaneously connected devices per account,
    /// matching the backend session-lease cap (warren-config
    /// MAX_DEVICES_PER_ACCOUNT) and the published terms.
    static let maxAllowedDevices = 3
}
