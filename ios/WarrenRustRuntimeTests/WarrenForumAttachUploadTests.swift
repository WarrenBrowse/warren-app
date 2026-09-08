import XCTest

@testable import WarrenRustRuntime

/// The attach-logs upload from an approval to the FFI call, with the wallet,
/// the collector and the client faked: the Android
/// `WarrenForumAttachUseCaseTest` mirrored. The one property the reviewer
/// asked for by name is that the gzip the collector produced is the byte
/// array the FFI receives.
final class WarrenForumAttachUploadTests: XCTestCase {
    private let sid = "0123456789abcdef0123456789abcdef"
    private let host = "connect.warrenbrowse.com"
    /// The BIP39 all-zero-entropy vector: a real wallet, derived in Rust.
    private static let phrase =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

    private final class Recorder {
        var walletLoads = 0
        var collectAddresses: [String?] = []
        var attached: [(sid: String, topicId: UInt64, host: String, seedBytes: Int, logGz: Data)] = []
    }

    private func upload(
        _ recorder: Recorder,
        wallet: Bool = true,
        gz: Data? = Data("gz".utf8),
        outcome: WarrenForumAttachOutcome = .attached
    ) -> WarrenForumAttachUpload {
        WarrenForumAttachUpload(
            loadWallet: {
                recorder.walletLoads += 1
                return wallet ? try? WarrenWallet.fromMnemonic(Self.phrase) : nil
            },
            collectGzipped: { address in
                recorder.collectAddresses.append(address)
                guard let gz else { throw WarrenProblemReport.Failure.empty }
                return gz
            },
            attach: { seed, sid, topicId, host, logGz in
                recorder.attached.append((sid, topicId, host, seed.count, logGz))
                return outcome
            })
    }

    func testApprovingLoadsTheWalletCollectsGzipsAndSignsThroughRust() throws {
        let recorder = Recorder()
        let gz = Data((0..<1234).map { UInt8(truncatingIfNeeded: $0 &* 31) })
        let result = upload(recorder, gz: gz).run(sid: sid, host: host, topicId: 42)
        XCTAssertEqual(result.outcome, .attached)
        XCTAssertEqual(result.gzBytes, 1234)
        XCTAssertEqual(recorder.walletLoads, 1)
        let call = try XCTUnwrap(recorder.attached.first)
        XCTAssertEqual(recorder.attached.count, 1)
        XCTAssertEqual(call.logGz, gz, "the gzip the collector produced crosses into the FFI call")
        XCTAssertEqual(call.sid, sid)
        XCTAssertEqual(call.topicId, 42)
        XCTAssertEqual(call.host, host)
        XCTAssertEqual(call.seedBytes, 32, "the seed-derived identity signs, never the mnemonic")
        // The wallet's own SS58 address is what the collector redacts.
        let address = try XCTUnwrap(try XCTUnwrap(recorder.collectAddresses.first))
        XCTAssertTrue(address.hasPrefix("w"), address)
    }

    func testAPreTopicApprovalCrossesWithTopicZero() {
        let recorder = Recorder()
        _ = upload(recorder).run(sid: sid, host: host, topicId: 0)
        XCTAssertEqual(recorder.attached.map(\.topicId), [0])
    }

    func testNoWalletRefusesBeforeAnythingIsCollected() {
        let recorder = Recorder()
        let result = upload(recorder, wallet: false).run(sid: sid, host: host, topicId: 42)
        XCTAssertEqual(result.outcome, .failed(reason: "wallet-absent"))
        XCTAssertNil(result.gzBytes)
        XCTAssertEqual(recorder.collectAddresses.count, 0)
        XCTAssertEqual(recorder.attached.count, 0)
    }

    func testAFailedCollectionIsItsOwnClassAndReachesNoHost() {
        let recorder = Recorder()
        let result = upload(recorder, gz: nil).run(sid: sid, host: host, topicId: 42)
        XCTAssertEqual(result.outcome, .failed(reason: "collect-failed"))
        XCTAssertNil(result.gzBytes)
        XCTAssertEqual(recorder.attached.count, 0)
    }

    func testAReportOverTheCapIsRefusedBeforeItCrossesAndTheCapItselfIsSent() {
        // The first leg of the report-size chain: a gzip the broker would
        // refuse at its base64 cap reaches no host. The cap is the shared
        // crate's `MAX_LOG_GZ_BYTES`, 12,000,000 bytes.
        XCTAssertEqual(WarrenForumAttachUpload.maxLogGzBytes, 12_000_000)
        let over = Recorder()
        let refused = upload(over, gz: Data(count: WarrenForumAttachUpload.maxLogGzBytes + 1))
            .run(sid: sid, host: host, topicId: 42)
        XCTAssertEqual(refused.outcome, .tooLarge)
        XCTAssertEqual(refused.gzBytes, WarrenForumAttachUpload.maxLogGzBytes + 1, "the size reaches the journal")
        XCTAssertEqual(over.attached.count, 0)
        let atCap = Recorder()
        let sent = upload(atCap, gz: Data(count: WarrenForumAttachUpload.maxLogGzBytes))
            .run(sid: sid, host: host, topicId: 42)
        XCTAssertEqual(sent.outcome, .attached)
        XCTAssertEqual(atCap.attached.count, 1)
    }

    func testTheProviderVerdictPassesThroughWithTheGzipSize() {
        let recorder = Recorder()
        let result = upload(recorder, outcome: .notAuthor).run(sid: sid, host: host, topicId: 42)
        XCTAssertEqual(result.outcome, .notAuthor)
        XCTAssertEqual(result.gzBytes, 2)
    }
}
