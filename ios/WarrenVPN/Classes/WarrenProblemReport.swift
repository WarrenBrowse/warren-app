//
//  WarrenProblemReport.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The redacted problem report the forum attach-logs flow uploads (doc 55).
//  Built on Mullvad's `ConsolidatedApplicationLog`, which consolidates log
//  files and redacts container paths, the account number, IPv4 and IPv6
//  addresses, plus any custom strings handed to it (the wallet SS58 address
//  here). Pure over its inputs: the app supplies the file URLs, the group
//  identifiers and the buffer size, so the consolidation compiles into the
//  non-hosted test bundle and is exercised over temp files on a developer Mac
//  (`WarrenProblemReportTests`).
//

import Foundation
import WarrenRustRuntime

enum WarrenProblemReport {
    /// Errors from building the report.
    enum Failure: Error, Equatable {
        /// The consolidated log had no content to send.
        case empty
    }

    /// The consolidated, redacted report text, in hand when this returns.
    /// Reads the files on the calling thread, so run it off the main thread.
    static func consolidate(fileURLs: [URL], redacting: [String], groupIdentifiers: [String], bufferSize: UInt64)
        -> String
    {
        let log = ConsolidatedApplicationLog(
            redactCustomStrings: redacting,
            redactContainerPathsForSecurityGroupIdentifiers: groupIdentifiers,
            bufferSize: bufferSize)
        // In hand on this thread: the queued `addLogFiles` plus `string`
        // shape read an empty report every time (the appends are enqueued
        // from inside the barrier block the read runs behind).
        return log.consolidated(adding: fileURLs)
    }

    /// The gzipped redacted report, ready for `forumAttachLogs`.
    ///
    /// # Errors
    /// [`Failure/empty`] when nothing was consolidated; a `WarrenGzipError`
    /// from the framing.
    static func gzipped(fileURLs: [URL], redacting: [String], groupIdentifiers: [String], bufferSize: UInt64) throws
        -> Data
    {
        let text = consolidate(
            fileURLs: fileURLs, redacting: redacting, groupIdentifiers: groupIdentifiers, bufferSize: bufferSize)
        guard !text.isEmpty else { throw Failure.empty }
        return try WarrenGzip.compress(Data(text.utf8))
    }
}
