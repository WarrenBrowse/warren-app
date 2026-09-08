//
//  WarrenProblemReport.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The redacted problem report the forum attach-logs flow uploads (doc 55).
//  Built on Mullvad's `ConsolidatedApplicationLog`, which already consolidates
//  the app and packet-tunnel log files, redacts container paths, the account
//  number, IPv4 and IPv6 addresses, plus any custom strings handed to it. The
//  Rust engine logs are forwarded into the same app log files by
//  `RustLogging`, so consolidating the app and packet-tunnel targets covers
//  both, and the forum events journal is added so the staff see the history
//  of the attempts. The wallet SS58 address is passed as a custom redaction
//  string: it is public, but a report shared with staff carries no identifier
//  the no-log rule would keep out of a log.
//

import Foundation
import WarrenRustRuntime

enum WarrenProblemReport {
    /// Errors from building the report.
    enum Failure: Error, Equatable {
        /// The consolidated log had no readable content to send.
        case empty
    }

    /// The consolidated, redacted report text. Reads log files, so run it off
    /// the main thread. `journalURL`, when given, is added so the events
    /// journal rides in the report.
    static func collect(walletAddress: String?, journalURL: URL?) -> String {
        let redact = walletAddress.map { [$0] } ?? []
        let log = ConsolidatedApplicationLog(
            redactCustomStrings: redact,
            redactContainerPathsForSecurityGroupIdentifiers: [ApplicationConfiguration.securityGroupIdentifier],
            bufferSize: ApplicationConfiguration.logMaximumFileSize)
        let container = ApplicationConfiguration.containerURL
        var files = ApplicationConfiguration.logFileURLs(for: .mainApp, in: container)
        files += ApplicationConfiguration.logFileURLs(for: .packetTunnel, in: container)
        if let journalURL, FileManager.default.fileExists(atPath: journalURL.path) {
            files.append(journalURL)
        }
        log.addLogFiles(fileURLs: files)
        // `string` reads on the consolidation queue behind the queued file
        // appends, so it returns the consolidated result on this thread.
        return log.string
    }

    /// The gzipped redacted report, ready for `forumAttachLogs`. Collects,
    /// then gzips, both off whatever thread this is called on.
    ///
    /// # Errors
    /// [`Failure/empty`] when nothing was collected; a `WarrenGzipError` from
    /// the framing.
    static func collectGzipped(walletAddress: String?, journalURL: URL?) throws -> Data {
        let text = collect(walletAddress: walletAddress, journalURL: journalURL)
        guard !text.isEmpty else { throw Failure.empty }
        return try WarrenGzip.compress(Data(text.utf8))
    }
}
