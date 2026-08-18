//
//  WarrenAccountClient.swift
//  WarrenRustRuntime
//
//  Created by Warren on 2026-06-10.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Swift facade over the `warren_account_ffi` Rust exports
//  (`warren-ios/src/warren_account_ffi.rs`). These drive the signed
//  warren-api client (`warren-api-client`), the same backend Android
//  talks to via `warren-jni` and the desktop daemon via
//  `WarrenRemoteAccountBackend`. They replace the legacy Mullvad
//  account-number REST flows: the Warren identity is the wallet.
//
//  Wire model:
//  - subscription: signed `GET /v1/subscription`  -> expiry
//  - voucher:      unsigned `POST /v1/register`    -> expiry
//  - delete:       signed `DELETE /v1/account`
//
//  Threading: every call blocks the calling thread (the Rust side
//  `block_on`s an HTTP round-trip). Callers MUST invoke these off the
//  main thread (the interactors dispatch onto a background queue).
//

import Foundation
import WarrenRustRuntimeProxy

/// Outcome of a `GET /v1/subscription` query.
public enum WarrenSubscriptionStatus: Equatable {
    /// The wallet has an active (or past) subscription with this expiry.
    case active(expiry: Date)
    /// The backend has no subscription bound to this wallet yet (HTTP
    /// 404). Aligned with the desktop `account-data-cache` treatment of
    /// `no-subscription`: the caller surfaces this as "out of time".
    case none
}

/// Errors surfaced by `WarrenAccountClient`.
public enum WarrenAccountError: Error, Equatable {
    /// The seed / argument marshaling failed before any network call.
    case invalidInput(String)
    /// Transport failure (network down, DNS, TLS) or an unparseable
    /// response. No HTTP status is available.
    case transport(String)
    /// The server replied with a non-2xx status. `status` lets callers
    /// map to a localized message (e.g. voucher 400 invalid, 409 used,
    /// 410 cancelled, 429 rate-limited). The response body is never
    /// surfaced (a 4xx body can echo request context).
    case server(status: Int, message: String)
}

/// Outcome of a `POST /v1/forum/login` (community-forum wallet login, doc 55).
public enum WarrenForumLoginOutcome: Equatable {
    /// The provider accepted the signature; the browser completes the login.
    case approved
    /// The wallet has never subscribed to Warren; forum access is refused (403).
    case subscriptionRequired
    /// The provider refused the signature because this device's clock is off by
    /// more than its accepted window. The one failure the user repairs themselves.
    case clockSkew
    /// Any other failure (bad input, provider error, transport error).
    case failed
}

/// Stateless facade over the Warren account FFI. All methods are
/// synchronous and blocking; run them off the main thread.
public enum WarrenAccountClient {
    /// The wallet seed length the FFI reads. A `Data` of any other length
    /// would make the Rust side read out of bounds, so callers are rejected
    /// before the FFI boundary.
    private static let seedByteCount = 32

    /// Signed `GET /v1/subscription`. Returns the subscription status
    /// for the wallet identified by `seed`.
    public static func subscription(seed: Data) -> Result<WarrenSubscriptionStatus, WarrenAccountError> {
        guard seed.count == seedByteCount else { return .failure(.invalidInput("seed must be 32 bytes")) }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return warren_account_get_subscription(base)
        }
        return parseEnvelope(raw).flatMap { envelope in
            switch envelope {
            case let .okExpiry(expiry):
                return .success(.active(expiry: expiry))
            case let .failure(error):
                // A 404 means "no subscription bound yet", not an error.
                if case let .server(status, _) = error, status == 404 {
                    return .success(.none)
                }
                return .failure(error)
            case .okVoid, .okToken:
                return .failure(.transport("subscription response missing expires_at"))
            }
        }
    }

    /// Unsigned `POST /v1/register`. Binds the wallet pubkey to a new
    /// subscription via `code`. Returns the new expiry. The voucher code
    /// is never logged.
    public static func redeemVoucher(seed: Data, code: String) -> Result<Date, WarrenAccountError> {
        guard seed.count == seedByteCount else { return .failure(.invalidInput("seed must be 32 bytes")) }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return code.withCString { codePtr in
                warren_account_redeem_voucher(base, codePtr)
            }
        }
        return parseEnvelope(raw).flatMap { envelope in
            switch envelope {
            case let .okExpiry(expiry):
                return .success(expiry)
            case let .failure(error):
                return .failure(error)
            case .okVoid, .okToken:
                return .failure(.transport("voucher response missing expires_at"))
            }
        }
    }

    /// Signed `DELETE /v1/account`. Permanently deletes the wallet's
    /// subscription server-side.
    public static func deleteAccount(seed: Data) -> Result<Void, WarrenAccountError> {
        guard seed.count == seedByteCount else { return .failure(.invalidInput("seed must be 32 bytes")) }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return warren_account_delete(base)
        }
        return parseEnvelope(raw).flatMap { envelope in
            switch envelope {
            case .okVoid, .okExpiry, .okToken:
                return .success(())
            case let .failure(error):
                return .failure(error)
            }
        }
    }

    /// Signed `POST /v1/payments/apple/init`. Mints an ephemeral
    /// payment session bound to the wallet and returns the session UUID
    /// to pass to StoreKit as the `appAccountToken`. The backend
    /// resolves that token back to this wallet at check time, so Apple
    /// never sees the pubkey.
    public static func storeKitInit(seed: Data) -> Result<String, WarrenAccountError> {
        guard seed.count == seedByteCount else { return .failure(.invalidInput("seed must be 32 bytes")) }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return warren_account_storekit_init(base)
        }
        return parseEnvelope(raw).flatMap { envelope in
            switch envelope {
            case let .okToken(token):
                return .success(token)
            case let .failure(error):
                return .failure(error)
            case .okExpiry, .okVoid:
                return .failure(.transport("storekit init response missing app_account_token"))
            }
        }
    }

    /// Signed `POST /v1/payments/apple/check`. Uploads the StoreKit 2
    /// signed transaction JWS so the backend can verify it against
    /// Apple's root CA and credit the wallet. Returns the new expiry.
    /// The JWS is never logged.
    public static func storeKitCheck(seed: Data, jws: String) -> Result<Date, WarrenAccountError> {
        guard seed.count == seedByteCount else { return .failure(.invalidInput("seed must be 32 bytes")) }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return jws.withCString { jwsPtr in
                warren_account_storekit_check(base, jwsPtr)
            }
        }
        return parseEnvelope(raw).flatMap { envelope in
            switch envelope {
            case let .okExpiry(expiry):
                return .success(expiry)
            case let .failure(error):
                return .failure(error)
            case .okToken, .okVoid:
                return .failure(.transport("storekit check response missing expires_at"))
            }
        }
    }

    /// Signs and submits a community-forum login challenge for `sid` to the
    /// connect `host` (`POST /v1/forum/login`, doc 55). Everything
    /// wire-sensitive (the signature, a fresh nonce, and the POST itself)
    /// happens in Rust; only `sid` and `host` cross the boundary, so the wallet
    /// signature never surfaces to Swift. `host` is re-validated against a hard
    /// allowlist in Rust so a hostile deep link cannot redirect the signed
    /// request. Blocking (run off the main thread); the seed and sid are never
    /// logged.
    public static func forumLogin(seed: Data, sid: String, host: String) -> WarrenForumLoginOutcome {
        guard seed.count == seedByteCount else { return .failed }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return sid.withCString { sidPtr in
                host.withCString { hostPtr in
                    warren_forum_login(base, sidPtr, hostPtr)
                }
            }
        }
        guard let raw else { return .failed }
        defer { warren_wallet_free_mnemonic(raw) }
        return forumLoginOutcome(fromEnvelope: String(cString: raw))
    }

    /// Maps the `warren_forum_login` JSON envelope to an outcome. The envelope
    /// is `{"ok":true}` or `{"ok":false,"error":"subscription-required"|
    /// "clock-skew"|"error"}`, single-sourced in the Rust `warren-forum` crate.
    /// Pure so the mapping is unit-tested off-device.
    static func forumLoginOutcome(fromEnvelope envelope: String?) -> WarrenForumLoginOutcome {
        guard let envelope,
              let data = envelope.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return .failed
        }
        if object["ok"] as? Bool == true {
            return .approved
        }
        switch object["error"] as? String {
        case "subscription-required":
            return .subscriptionRequired
        case "clock-skew":
            return .clockSkew
        default:
            return .failed
        }
    }

    /// Best-effort: tells the connect `host` the user declined the forum login
    /// for `sid` (`POST /v1/session/<sid>/cancel`), so the waiting browser page
    /// unblocks instead of polling to timeout. Unsigned (no wallet material);
    /// mirrors the desktop `cancelForumLogin`. Blocking, run off the main
    /// thread; failures are ignored (the server session expires in 5 min).
    public static func forumLoginCancel(sid: String, host: String) {
        sid.withCString { sidPtr in
            host.withCString { hostPtr in
                warren_forum_cancel(sidPtr, hostPtr)
            }
        }
    }

    // MARK: - Envelope parsing

    /// Parsed shape of the JSON envelope returned by the FFI.
    private enum Envelope {
        case okExpiry(Date)
        case okToken(String)
        case okVoid
        case failure(WarrenAccountError)
    }

    /// Parses the heap `CString` JSON envelope and frees it. A null
    /// pointer means the Rust side could not allocate the result.
    private static func parseEnvelope(_ raw: UnsafeMutablePointer<CChar>?) -> Result<Envelope, WarrenAccountError> {
        guard let raw else {
            return .failure(.transport("FFI returned a null result"))
        }
        defer { warren_wallet_free_mnemonic(raw) }
        let jsonString = String(cString: raw)
        guard let data = jsonString.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return .failure(.transport("FFI returned an unparseable envelope"))
        }

        let ok = object["ok"] as? Bool ?? false
        if ok {
            if let expiresAt = object["expires_at"] as? NSNumber {
                let date = Date(timeIntervalSince1970: expiresAt.doubleValue)
                return .success(.okExpiry(date))
            }
            if let token = object["app_account_token"] as? String {
                return .success(.okToken(token))
            }
            return .success(.okVoid)
        }

        let message = object["error"] as? String ?? "unknown error"
        if let status = object["status"] as? NSNumber {
            return .success(.failure(.server(status: status.intValue, message: message)))
        }
        return .success(.failure(.transport(message)))
    }
}
