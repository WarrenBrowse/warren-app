//
//  WarrenSecureMnemonic.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The recovery phrase, held somewhere it can actually be erased.
//
//  A Swift `String` cannot be wiped: it is immutable, it copies on every
//  assignment and slice, small values live inline in the struct, and bridging
//  to `NSString` makes copies this code never sees. So a phrase read into a
//  `String` stays in whatever heap pages it touched until they are reused, and
//  `memset` on it is not even expressible. Every other secret in this app is
//  zeroed on drop (the wallet seed here, `Zeroizing` on the Rust side); the
//  phrase, which is the secret all the others derive from, was the one that
//  was not.
//
//  This holds the phrase as raw UTF-8 in a buffer this code owns outright, so
//  there is exactly one copy and `memset_s` reaches it. `memset_s` rather than
//  a loop: a loop writing bytes nobody reads again is precisely what a
//  compiler is allowed to delete.
//

import Foundation

/// A BIP39 recovery phrase in a buffer that is zeroed when it is done with.
///
/// A class, not a struct: a struct would be copied on every pass and this
/// type's whole purpose is that exactly one copy exists and it is the one that
/// gets wiped.
public final class WarrenSecureMnemonic {
    private var buffer: UnsafeMutableBufferPointer<UInt8>
    private var length: Int

    /// Takes ownership of `bytes`, the phrase as UTF-8. The caller's own copy
    /// is its own problem: pass a buffer nothing else holds.
    public init(utf8 bytes: [UInt8]) {
        length = bytes.count
        buffer = UnsafeMutableBufferPointer<UInt8>.allocate(capacity: max(bytes.count, 1))
        buffer.initialize(repeating: 0)
        _ = buffer.update(fromContentsOf: bytes)
    }

    /// Reads the phrase out of `data`, the shape the Keychain hands back, so
    /// no `String` is ever built on the way in.
    public convenience init(data: Data) {
        self.init(utf8: [UInt8](data))
    }

    /// From a phrase that is already a `String`: the one entry that cannot
    /// avoid it, because the user typed it into a text field and UIKit owns
    /// that copy. What this buys is that every hop AFTER the field is wiped.
    public convenience init(phrase: String) {
        self.init(utf8: Array(phrase.utf8))
    }

    deinit {
        wipe()
        buffer.deallocate()
    }

    /// Zeroes the phrase now. Idempotent, and the buffer stays allocated so a
    /// later read sees an empty phrase rather than freed memory.
    public func wipe() {
        guard length > 0 else { return }
        memset_s(buffer.baseAddress, buffer.count, 0, buffer.count)
        length = 0
    }

    /// Whether anything is left to read. False after a wipe, and for a phrase
    /// that was empty to begin with.
    public var isEmpty: Bool { length == 0 }

    /// Words in the phrase, without materialising it: the count is what the
    /// import screen checks, and it never needs the words themselves.
    public var wordCount: Int {
        var words = 0
        var inWord = false
        for index in 0..<length {
            let byte = buffer[index]
            let isSpace = byte == 0x20 || byte == 0x0A || byte == 0x0D || byte == 0x09
            if isSpace {
                inWord = false
            } else if !inWord {
                inWord = true
                words += 1
            }
        }
        return words
    }

    /// Hands the phrase to `body` as a NUL-terminated C string, which is what
    /// every Rust entry takes. The temporary carries the NUL only; the phrase
    /// bytes are this object's own, so nothing extra is copied and nothing
    /// extra needs wiping.
    ///
    /// The pointer is valid for the call and nowhere else: a `body` that
    /// stores it hands out a dangling secret.
    public func withCString<R>(_ body: (UnsafePointer<CChar>) throws -> R) rethrows -> R {
        var terminated = UnsafeMutableBufferPointer<CChar>.allocate(capacity: length + 1)
        terminated.initialize(repeating: 0)
        defer {
            memset_s(terminated.baseAddress, terminated.count, 0, terminated.count)
            terminated.deallocate()
        }
        for index in 0..<length {
            terminated[index] = CChar(bitPattern: buffer[index])
        }
        return try body(UnsafePointer(terminated.baseAddress!))
    }

    /// The phrase as `Data`, for the one caller that must hand it to the
    /// Keychain. The returned value is a copy this type can no longer wipe,
    /// so it is written and dropped in the same breath.
    public func withData<R>(_ body: (Data) throws -> R) rethrows -> R {
        try body(Data(bytes: buffer.baseAddress!, count: length))
    }

    /// The phrase as text, for the backup screen, which has to draw it.
    ///
    /// This is the one place the phrase becomes a `String`, and that copy is
    /// beyond reach the moment it exists: UIKit and SwiftUI own it from there.
    /// Call it as late as possible and never store what it returns.
    public func revealForDisplay() -> String {
        String(decoding: UnsafeBufferPointer(rebasing: buffer[0..<length]), as: UTF8.self)
    }
}
