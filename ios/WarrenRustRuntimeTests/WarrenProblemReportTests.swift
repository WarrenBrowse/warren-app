import XCTest

@testable import WarrenRustRuntime

/// The redacted problem report the attach-logs flow uploads, built in hand
/// on the calling thread. The report was empty on device once: the queued
/// consolidation enqueued its file appends from inside its own barrier block,
/// so a `string` read right after it ran ahead of every append. This suite
/// runs in the non-hosted bundle so it executes on a developer Mac with no
/// signing identity, where the hosted app cannot launch.
final class WarrenProblemReportTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("problem-report-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
        try super.tearDownWithError()
    }

    private func write(_ text: String, as name: String) throws -> URL {
        let url = directory.appendingPathComponent(name)
        try text.write(to: url, atomically: true, encoding: .utf8)
        return url
    }

    private let lines = """
        WarrenVPN version 2026.5-dev1
        [08/09/2026 @ 20:26:52][TunnelManager][debug] the tunnel came up
        [08/09/2026 @ 20:26:53][WarrenForumAttach][info] wallet wb7kgy8FF4rxhP9DnB signed
        [08/09/2026 @ 20:26:54][Rust][info] peer 192.168.1.124 answered in 12 ms
        """

    func testTheReportIsInHandRedactedAndNeverEmptyForAReadableFile() throws {
        let file = try write(lines, as: "com.warrenbrowse.vpn.ios_2026-09-08.log")
        let report = WarrenProblemReport.consolidate(
            fileURLs: [file], redacting: ["wb7kgy8FF4rxhP9DnB"], groupIdentifiers: [], bufferSize: 65_536)
        XCTAssertFalse(report.isEmpty, "the consolidation must complete before the report is read")
        XCTAssertTrue(report.hasPrefix("System information:\n"), String(report.prefix(40)))
        XCTAssertTrue(report.contains("the tunnel came up"))
        XCTAssertTrue(report.contains("peer [REDACTED] answered"), "IPv4 addresses are redacted")
        XCTAssertFalse(report.contains("192.168.1.124"))
        XCTAssertTrue(report.contains("wallet [REDACTED] signed"), "the wallet address is redacted")
        XCTAssertFalse(report.contains("wb7kgy8FF4rxhP9DnB"))
    }

    func testEveryFileIsConsolidatedInOrderAndAMissingOneIsNamedInsideTheReport() throws {
        let first = try write("first file line", as: "a.log")
        let second = try write("second file line", as: "b.log")
        let missing = directory.appendingPathComponent("gone.log")
        let report = WarrenProblemReport.consolidate(
            fileURLs: [first, second, missing], redacting: [], groupIdentifiers: [], bufferSize: 65_536)
        let firstAt = try XCTUnwrap(report.range(of: "first file line")?.lowerBound)
        let secondAt = try XCTUnwrap(report.range(of: "second file line")?.lowerBound)
        XCTAssertLessThan(firstAt, secondAt, "files ride in the order they were given")
        XCTAssertTrue(report.contains("Log file does not exist"), "a missing file is an error block, not a lost report")
    }

    func testNothingToReadIsAnEmptyReportThatTheGzipRefuses() {
        XCTAssertEqual(
            WarrenProblemReport.consolidate(fileURLs: [], redacting: [], groupIdentifiers: [], bufferSize: 65_536),
            "")
        XCTAssertThrowsError(
            try WarrenProblemReport.gzipped(fileURLs: [], redacting: [], groupIdentifiers: [], bufferSize: 65_536)
        ) { error in
            XCTAssertEqual(error as? WarrenProblemReport.Failure, .empty)
        }
    }

    func testTheGzippedReportInflatesToTheRedactedText() throws {
        let file = try write(lines, as: "app.log")
        let gz = try WarrenProblemReport.gzipped(
            fileURLs: [file], redacting: ["wb7kgy8FF4rxhP9DnB"], groupIdentifiers: [], bufferSize: 65_536)
        XCTAssertEqual(Array(gz.prefix(2)), [0x1f, 0x8b])
        let text = try XCTUnwrap(String(data: try WarrenGzip.decompress(gz), encoding: .utf8))
        XCTAssertTrue(text.hasPrefix("System information:\n"))
        XCTAssertTrue(text.contains("wallet [REDACTED] signed"))
        XCTAssertFalse(text.contains("192.168.1.124"))
    }
}
