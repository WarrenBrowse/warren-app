//
//  WarrenForumEventsJournal.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The forum flows' event journal: one JSON line per step of every sign-in
//  and attach attempt (link received and its verdict, prompt shown, signed,
//  the outcome class and how long it took), in the app log directory so the
//  problem-report collector carries it like any other log. It exists because
//  the rolling logs are filled by the tunnel first, and the one thing a
//  report about "the forum did nothing" needs is the history of the attempts.
//  The field grammar is Android's `ForumEventsJournal`: a closed key set and
//  values that are numbers, flags, or tokens shorter than a session id, so no
//  call site has a field to put a sid, an address or a handle in.
//

import Foundation
import WarrenLogging

/// The steps of the forum flows a journal line can name. Closed: never built
/// from data. The tokens are Android's.
enum ForumEvent: String {
    case linkReceived = "link.received"
    case loginDeferred = "login.deferred"
    case loginSigning = "login.signing"
    case loginResult = "login.result"
    case loginDeclined = "login.declined"
    case attachDeferred = "attach.deferred"
    case attachSigning = "attach.signing"
    case attachResult = "attach.result"
    case attachDeclined = "attach.declined"
}

/// Which consent an accepted link or typed code raises.
enum ForumLinkKind: String {
    case login
    case attach
}

/// Where an accepted request came from.
enum LinkSource: String {
    case deepLink = "deep-link"
    case typedCode = "typed-code"
}

/// One field of a journal line. The key set is closed and every value is a
/// number, a flag, or a token checked against a grammar shorter than a session
/// id, so no call site has a field to put a sid, an address or a handle in.
enum JournalField {
    case `class`(String)
    case reason(String)
    case verdict(String)
    case source(LinkSource)
    case kind(ForumLinkKind)
    case preTopic(Bool)
    case coldStart(Bool)
    case crossDevice(Bool)
    case elapsedMs(Int)
    case gzBytes(Int)

    static let malformed = "malformed"
    static let none = "none"

    /// A class token is lowercase words joined by hyphens or a colon, at most
    /// `maxTokenChars` long: shorter than a session id (32 hex characters) and
    /// an SS58 address (49), so neither fits even through a wrong call site.
    /// Anything else is `malformed`.
    static let maxTokenChars = 24
    private static let classTokenPattern = "^[a-z][a-z0-9]*([-:][a-z0-9]+)*$"

    static func classToken(_ token: String) -> String {
        guard token.count <= maxTokenChars,
            token.range(of: classTokenPattern, options: .regularExpression) != nil
        else {
            return malformed
        }
        return token
    }

    var key: String {
        switch self {
        case .class: return "class"
        case .reason: return "reason"
        case .verdict: return "verdict"
        case .source: return "source"
        case .kind: return "kind"
        case .preTopic: return "pre_topic"
        case .coldStart: return "cold_start"
        case .crossDevice: return "cross_device"
        case .elapsedMs: return "elapsed_ms"
        case .gzBytes: return "gz_bytes"
        }
    }

    var value: String {
        switch self {
        case let .class(token), let .reason(token), let .verdict(token):
            return Self.classToken(token)
        case let .source(source): return source.rawValue
        case let .kind(kind): return kind.rawValue
        case let .preTopic(flag), let .coldStart(flag), let .crossDevice(flag):
            return flag ? "true" : "false"
        case let .elapsedMs(count): return String(count)
        case let .gzBytes(count): return String(count)
        }
    }
}

/// The forum flows' event journal file. `record` formats the line on the
/// caller (every call site is `@MainActor`) and hands the write to the
/// journal's own serial queue, so the main thread never touches the file and
/// the staff can order the attempts by the sequence number stamped here.
final class WarrenForumEventsJournal: @unchecked Sendable {
    static let fileName = "warren-events.log"
    static let maxBytes = 256 * 1024

    let fileURL: URL
    private let logger = Logger(label: "WarrenForumEvents")
    private let queue = DispatchQueue(label: "com.warrenbrowse.forum.events", qos: .utility)
    private let lock = NSLock()
    private var sequence: Int = 0

    init(directory: URL) {
        fileURL = directory.appendingPathComponent(Self.fileName, isDirectory: false)
    }

    func record(_ event: ForumEvent, _ fields: JournalField...) {
        record(event, fields: fields)
    }

    func record(_ event: ForumEvent, fields: [JournalField]) {
        lock.lock()
        let sequenceNumber = sequence
        sequence += 1
        lock.unlock()
        let line = Self.format(at: Date(), sequence: sequenceNumber, event: event, fields: fields)
        logger.info("\(event.rawValue) \(fields.map { "\($0.key)=\($0.value)" }.joined(separator: " "))")
        queue.async { [self] in
            do {
                try FileManager.default.createDirectory(
                    at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
                if let size = try? fileURL.resourceValues(forKeys: [.fileSizeKey]).fileSize, size > Self.maxBytes {
                    truncateHead()
                }
                append(line + "\n")
            } catch {
                logger.warning("forum events journal write failed")
            }
        }
    }

    /// The lines currently in the journal file, oldest first, read behind
    /// every pending write on the journal's queue.
    func drain() throws -> [String] {
        try queue.sync {
            guard FileManager.default.fileExists(atPath: fileURL.path) else { return [] }
            let text = try String(contentsOf: fileURL, encoding: .utf8)
            return text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
        }
    }

    /// Keeps the newest half of the file: the history that matters is recent.
    private func truncateHead() {
        guard let text = try? String(contentsOf: fileURL, encoding: .utf8) else { return }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: true)
        let kept = lines.suffix(lines.count - lines.count / 2)
        try? (kept.joined(separator: "\n") + "\n").write(to: fileURL, atomically: true, encoding: .utf8)
    }

    private func append(_ line: String) {
        guard let data = line.data(using: .utf8) else { return }
        if let handle = try? FileHandle(forWritingTo: fileURL) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        } else {
            try? data.write(to: fileURL)
        }
    }

    /// One journal line, pure so the shape is unit-testable. `seq` and the
    /// three envelope keys are JSON, then each field's key and its string
    /// value, in order.
    static func format(at: Date, sequence: Int, event: ForumEvent, fields: [JournalField]) -> String {
        var parts = [
            "\"seq\":\(sequence)",
            "\"at\":\(jsonString(iso8601(at)))",
            "\"event\":\(jsonString(event.rawValue))",
        ]
        for field in fields {
            parts.append("\(jsonString(field.key)):\(jsonString(field.value))")
        }
        return "{" + parts.joined(separator: ",") + "}"
    }

    private static func iso8601(_ date: Date) -> String {
        // A fresh formatter per line: `ISO8601DateFormatter` is not `Sendable`,
        // and `format` is called off several threads before the write lock.
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: date)
    }

    private static func jsonString(_ value: String) -> String {
        var out = "\""
        for scalar in value.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default: out.unicodeScalars.append(scalar)
            }
        }
        out += "\""
        return out
    }
}
