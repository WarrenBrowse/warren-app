//
//  WarrenGzip.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Gzip framing for the forum attach-logs upload (doc 55). The connect
//  broker inflates the `log_gz_b64` field as a gzip member (RFC 1952), the
//  same encoding the desktop's `zlib.gzipSync` and Android's
//  `GZIPOutputStream` produce, so a raw deflate or a zlib-wrapped stream
//  would be refused after a full upload. Apple's Compression framework only
//  emits and reads raw DEFLATE, so this wraps it with the gzip header, the
//  CRC-32 and the input length that RFC 1952 requires.
//

import Compression
import Foundation

/// Errors from the gzip framing.
public enum WarrenGzipError: Error, Equatable {
    /// The Compression stream could not be initialised or advanced.
    case stream
    /// A gzip member was expected but the bytes are not one (bad magic,
    /// truncated, or the trailer does not match the inflated content).
    case notGzip
}

public enum WarrenGzip {
    /// The RFC 1952 fixed header: magic `1f 8b`, deflate method `08`, no
    /// flags, no mtime, no extra flags, OS unknown (`ff`).
    private static let header: [UInt8] = [0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff]

    /// The gzip member for `data`: header, raw DEFLATE body, CRC-32 of the
    /// input and its length modulo 2^32, both little-endian.
    ///
    /// # Errors
    /// [`WarrenGzipError/stream`] if the Compression stream fails.
    public static func compress(_ data: Data) throws -> Data {
        let deflated = try run(operation: COMPRESSION_STREAM_ENCODE, source: data)
        var out = Data(header)
        out.append(deflated)
        var crc = crc32(data).littleEndian
        withUnsafeBytes(of: &crc) { out.append(contentsOf: $0) }
        var size = UInt32(truncatingIfNeeded: data.count).littleEndian
        withUnsafeBytes(of: &size) { out.append(contentsOf: $0) }
        return out
    }

    /// The bytes a gzip member frames, verifying its magic and its trailing
    /// CRC-32. The inverse of [`compress`], and only that: it assumes the
    /// member has FLG == 0 (no extra field, name, comment or header CRC), the
    /// shape [`compress`] writes, so the deflate body starts at byte 10. It
    /// exists for the tests and the preview; nothing on the wire is inflated
    /// here.
    ///
    /// # Errors
    /// [`WarrenGzipError/notGzip`] for a non-gzip or corrupt member;
    /// [`WarrenGzipError/stream`] if the Compression stream fails.
    public static func decompress(_ gz: Data) throws -> Data {
        guard gz.count >= header.count + 8,
            gz[gz.startIndex] == 0x1f,
            gz[gz.startIndex + 1] == 0x8b,
            gz[gz.startIndex + 2] == 0x08
        else {
            throw WarrenGzipError.notGzip
        }
        let body = gz.subdata(in: (gz.startIndex + header.count)..<(gz.endIndex - 8))
        let inflated = try run(operation: COMPRESSION_STREAM_DECODE, source: body)
        let expectedCrc = readUInt32LE(gz, at: gz.endIndex - 8)
        guard crc32(inflated) == expectedCrc else {
            throw WarrenGzipError.notGzip
        }
        return inflated
    }

    /// Pumps `source` through a Compression `zlib` (raw DEFLATE) stream in
    /// either direction, growing the output across as many chunks as needed.
    private static func run(operation: compression_stream_operation, source: Data) throws -> Data {
        let streamPointer = UnsafeMutablePointer<compression_stream>.allocate(capacity: 1)
        defer { streamPointer.deallocate() }
        guard compression_stream_init(streamPointer, operation, COMPRESSION_ZLIB) == COMPRESSION_STATUS_OK else {
            throw WarrenGzipError.stream
        }
        defer { compression_stream_destroy(streamPointer) }

        let chunk = 64 * 1024
        let destination = UnsafeMutablePointer<UInt8>.allocate(capacity: chunk)
        defer { destination.deallocate() }

        var output = Data()
        let flags = Int32(COMPRESSION_STREAM_FINALIZE.rawValue)

        return try source.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> Data in
            let base = raw.bindMemory(to: UInt8.self).baseAddress
            streamPointer.pointee.src_ptr = base ?? UnsafePointer<UInt8>(bitPattern: 0x1)!
            streamPointer.pointee.src_size = source.count
            while true {
                streamPointer.pointee.dst_ptr = destination
                streamPointer.pointee.dst_size = chunk
                let status = compression_stream_process(streamPointer, flags)
                switch status {
                case COMPRESSION_STATUS_OK, COMPRESSION_STATUS_END:
                    let produced = chunk - streamPointer.pointee.dst_size
                    if produced > 0 {
                        output.append(destination, count: produced)
                    }
                    if status == COMPRESSION_STATUS_END {
                        return output
                    }
                default:
                    throw WarrenGzipError.stream
                }
            }
        }
    }

    private static func readUInt32LE(_ data: Data, at index: Data.Index) -> UInt32 {
        var value: UInt32 = 0
        for offset in 0..<4 {
            value |= UInt32(data[index + offset]) << (8 * offset)
        }
        return value
    }

    /// The IEEE CRC-32 RFC 1952 seals a gzip member with, table-driven.
    private static let crcTable: [UInt32] = {
        (0..<256).map { index -> UInt32 in
            var c = UInt32(index)
            for _ in 0..<8 {
                c = (c & 1) != 0 ? 0xEDB8_8320 ^ (c >> 1) : c >> 1
            }
            return c
        }
    }()

    private static func crc32(_ data: Data) -> UInt32 {
        var crc: UInt32 = 0xFFFF_FFFF
        for byte in data {
            crc = crcTable[Int((crc ^ UInt32(byte)) & 0xFF)] ^ (crc >> 8)
        }
        return crc ^ 0xFFFF_FFFF
    }
}
