//
//  WarrenForumAttachUpload.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The attach-logs upload from an approval to the FFI call (doc 55), with
//  its three system boundaries injected: the wallet store, the report
//  collector and the signing client. Pure Foundation over WarrenRustRuntime,
//  so it compiles into the non-hosted test bundle and is exercised with fakes
//  on a developer Mac (`WarrenForumAttachUploadTests`), the Android
//  `WarrenForumAttachUseCase` mirrored.
//

import Foundation
import WarrenRustRuntime

struct WarrenForumAttachUpload {
    /// The wallet, loaded for this one upload and forgotten right after; `nil`
    /// when the device has none.
    let loadWallet: () -> WarrenWallet?
    /// The gzipped redacted report, with the wallet address to redact.
    let collectGzipped: (_ redactAddress: String?) throws -> Data
    /// Sign and send: seed, sid, topic, host, gzip. The Rust FFI in production.
    let attach: (Data, String, UInt64, String, Data) -> WarrenForumAttachOutcome

    /// Largest gzipped report sent: the shared crate's `MAX_LOG_GZ_BYTES`, read
    /// off the FFI rather than copied, the broker's 16,000,000-character
    /// base64 cap translated to bytes. The first leg of the report-size
    /// chain, applied here before any byte leaves; the Rust side applies it
    /// again.
    static var maxLogGzBytes: Int { WarrenAccountClient.forumMaxLogGzBytes }

    struct Result: Equatable {
        let outcome: WarrenForumAttachOutcome
        /// The gzip's size when one was produced, for the journal.
        let gzBytes: Int?
    }

    /// Load the wallet, collect and gzip the report redacted of the wallet's
    /// own address, size it, then sign and send it. The wallet is read first
    /// because the address the collector redacts derives from its seed.
    func run(sid: String, host: String, topicId: UInt64) -> Result {
        guard let wallet = loadWallet() else {
            return Result(outcome: .failed(reason: "wallet-absent"), gzBytes: nil)
        }
        defer { wallet.forgetSecret() }
        let address = wallet.publicKeyAddress
        let gz: Data
        do {
            gz = try collectGzipped(address.isEmpty ? nil : address)
        } catch {
            return Result(outcome: .failed(reason: "collect-failed"), gzBytes: nil)
        }
        // The first leg of the report-size chain: a gzip the broker would
        // refuse reaches no host. The Rust side gates again before signing.
        if gz.count > Self.maxLogGzBytes {
            return Result(outcome: .tooLarge, gzBytes: gz.count)
        }
        return Result(outcome: attach(wallet.seed, sid, topicId, host, gz), gzBytes: gz.count)
    }
}
