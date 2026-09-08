import XCTest

@testable import WarrenRustRuntime

/// The gzip framing of the attach-logs upload. The connect broker inflates
/// the `log_gz_b64` field as a gzip member (RFC 1952), the encoding the
/// desktop's `zlib.gzipSync` and Android's `GZIPOutputStream` produce, so a
/// raw deflate or a zlib-wrapped stream would be refused after a full upload.
final class WarrenGzipTests: XCTestCase {
    func testTheOutputIsAGzipMemberThatInflatesBackToTheInput() throws {
        let report = Data(String(repeating: "System information:\nos: iOS 26\n", count: 200).utf8)
        let gz = try WarrenGzip.compress(report)
        // The two magic bytes and the deflate method of RFC 1952.
        XCTAssertEqual(Array(gz.prefix(3)), [0x1f, 0x8b, 0x08])
        XCTAssertLessThan(gz.count, report.count, "a repetitive report must shrink")
        XCTAssertEqual(try WarrenGzip.decompress(gz), report)
    }

    func testAnEmptyReportStillFramesAsGzip() throws {
        let gz = try WarrenGzip.compress(Data())
        XCTAssertEqual(Array(gz.prefix(2)), [0x1f, 0x8b])
        XCTAssertEqual(try WarrenGzip.decompress(gz), Data())
    }

    func testALargeReportRoundTripsAcrossOutputChunks() throws {
        // Larger than one output chunk, and incompressible enough that the
        // deflate output itself spans several chunks.
        var bytes = [UInt8](repeating: 0, count: 300_000)
        var seed: UInt32 = 0x9E37_79B9
        for index in bytes.indices {
            seed = seed &* 1_664_525 &+ 1_013_904_223
            bytes[index] = UInt8(truncatingIfNeeded: seed >> 24)
        }
        let input = Data(bytes)
        XCTAssertEqual(try WarrenGzip.decompress(try WarrenGzip.compress(input)), input)
    }
}
