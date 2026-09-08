import XCTest

@testable import WarrenRustRuntime

/// The gzip framing of the attach-logs upload. The connect broker inflates
/// the `log_gz_b64` field as a gzip member (RFC 1952) with `flate2`'s
/// `GzDecoder`, which validates the CRC-32 and the ISIZE of the trailer at
/// end of stream, so the bytes are pinned here against the RFC for known
/// inputs rather than only round-tripped through this module's own decoder.
final class WarrenGzipTests: XCTestCase {
    /// RFC 1952 fixed header: magic, deflate, no flags, no mtime, no extra
    /// flags, OS unknown.
    private let header: [UInt8] = [0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff]

    func testTheHeaderAndTheTrailerAreRfc1952ForAKnownInput() throws {
        // CRC-32 of "hello" is 0x3610a686, ISIZE 5; both little-endian.
        let gz = try WarrenGzip.compress(Data("hello".utf8))
        XCTAssertEqual(Array(gz.prefix(10)), header)
        XCTAssertEqual(Array(gz.suffix(8)), [0x86, 0xa6, 0x10, 0x36, 0x05, 0x00, 0x00, 0x00])
        XCTAssertGreaterThan(gz.count, 18, "a deflate body sits between header and trailer")
    }

    func testTheTrailerCarriesTheCrcAndTheLengthOfALongerKnownInput() throws {
        // CRC-32 of the pangram is 0x414fa339, its length 43.
        let gz = try WarrenGzip.compress(Data("The quick brown fox jumps over the lazy dog".utf8))
        XCTAssertEqual(Array(gz.prefix(10)), header)
        XCTAssertEqual(Array(gz.suffix(8)), [0x39, 0xa3, 0x4f, 0x41, 0x2b, 0x00, 0x00, 0x00])
    }

    func testTheOutputIsAGzipMemberThatInflatesBackToTheInput() throws {
        let report = Data(String(repeating: "System information:\nos: iOS 26\n", count: 200).utf8)
        let gz = try WarrenGzip.compress(report)
        XCTAssertEqual(Array(gz.prefix(3)), [0x1f, 0x8b, 0x08])
        XCTAssertLessThan(gz.count, report.count, "a repetitive report must shrink")
        XCTAssertEqual(try WarrenGzip.decompress(gz), report)
    }

    func testAnEmptyReportStillFramesAsGzip() throws {
        // CRC-32 of nothing is 0, ISIZE 0.
        let gz = try WarrenGzip.compress(Data())
        XCTAssertEqual(Array(gz.prefix(10)), header)
        XCTAssertEqual(Array(gz.suffix(8)), [0, 0, 0, 0, 0, 0, 0, 0])
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

    func testACorruptTrailerIsRefusedByTheDecoder() throws {
        var gz = try WarrenGzip.compress(Data("hello".utf8))
        gz[gz.endIndex - 8] ^= 0xff
        XCTAssertThrowsError(try WarrenGzip.decompress(gz)) { error in
            XCTAssertEqual(error as? WarrenGzipError, .notGzip)
        }
        XCTAssertThrowsError(try WarrenGzip.decompress(Data("not gzip at all".utf8)))
    }
}
