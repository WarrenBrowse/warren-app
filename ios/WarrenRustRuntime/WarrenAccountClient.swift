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
//  - campaign:     signed `GET /v1/campaign/{id}/voucher` -> code
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

/// Outcome of a `POST /v1/forum/login` (community-forum wallet login, doc 55):
/// the `login` table of `fixtures/client-rules/forum_outcomes.json`, which the
/// desktop and Android decoders read from the same crate.
public enum WarrenForumLoginOutcome: Equatable {
    /// The provider accepted the signature; the browser completes the login.
    /// Carries the forum identity the body handed back, `nil` from an older
    /// provider that names none.
    case approved(WarrenForumIdentity?)
    /// The wallet has never subscribed to Warren; forum access is refused (403).
    case subscriptionRequired
    /// The provider refused the signature because this device's clock is off by
    /// more than its accepted window. The one failure the user repairs themselves.
    case clockSkew
    /// The session is gone (404): expired, cancelled or already consumed. A
    /// retry on the same link can only fail again.
    case expired
    /// Any other failure, with its class (`transport`, `build`, `runtime`,
    /// `http-<status>`; `unknown` when the envelope names none). The class is
    /// for the log only, never shown as such.
    case failed(reason: String)

    /// True for the outcomes after which the pending link is spent (the
    /// fixture's `terminal_kinds`): the prompt must not offer a retry.
    public var isTerminal: Bool {
        switch self {
        case .subscriptionRequired, .clockSkew, .expired:
            return true
        case .approved, .failed:
            return false
        }
    }
}

/// What the provider made of a forum attach-logs upload (doc 55): the
/// desktop `ForumAttachResult` plus the clock-skew refusal the login already
/// tells apart. Single-sourced in the Rust `warren-forum` crate
/// (`attach_envelope`); the tokens are pinned by `WarrenForumAttachOutcomeTests`.
public enum WarrenForumAttachOutcome: Equatable {
    /// Attached to the topic, or parked for a report still being composed.
    case attached
    /// The wallet is not the author of the topic; the provider refused (403).
    case notAuthor
    /// The session is gone: expired, cancelled, already served, or bound to
    /// another topic than the one sent (404).
    case expired
    /// The gzipped report is over the cap, here before any byte leaves or at
    /// the provider (413).
    case tooLarge
    /// The signature was refused for a device clock outside the window.
    case clockSkew
    /// The provider failed on its own side (5xx); nothing to fix here.
    case serverError
    /// Any other failure, with its class (`build`, `runtime`, `transport`,
    /// `upload-timeout`, `http-<status>`, `unknown`). The class is for the log
    /// and the journal, never shown as such.
    case failed(reason: String)

    /// True when no retry from this device can change the outcome: the
    /// provider refused the signer as author, the session is gone, or the
    /// report is over the cap. A clock fix, a settled tunnel or a recovered
    /// provider are retries worth offering, as on Android.
    public var isTerminal: Bool {
        switch self {
        case .notAuthor, .expired, .tooLarge:
            return true
        case .attached, .clockSkew, .serverError, .failed:
            return false
        }
    }

    /// The coarse class for the log and the events journal; never a value.
    public var journalClass: String {
        switch self {
        case .attached: return "attached"
        case .notAuthor: return "not-author"
        case .expired: return "expired"
        case .tooLarge: return "too-large"
        case .clockSkew: return "clock-skew"
        case .serverError: return "server-error"
        case let .failed(reason): return reason
        }
    }
}

/// Where a session id typed by hand belongs, from the unsigned status reads
/// (`warren_forum_code_probe`, the attach meta included). Single-sourced in
/// the Rust crate (`code_placement_envelope`).
public enum WarrenForumCodePlacement: Equatable {
    /// A pending sign-in session: the login consent applies.
    case login
    /// A pending attach-logs session and the topic its meta named (0 for a
    /// report still being composed): the attach consent applies.
    case attach(topicId: UInt64)
    /// An attach session whose meta named no usable topic (an older broker):
    /// not offered as one, because the upload could only ever be refused as a
    /// dead session. The login consent applies.
    case attachWithoutTopic
    /// The code is spent, whatever it was.
    case gone
    /// The reads did not settle it: the caller falls back to the login flow.
    case unknown

    /// The class the journal records for the placement.
    public var journalClass: String {
        switch self {
        case .login: return "login"
        case .attach: return "attach"
        case .attachWithoutTopic: return "attach-no-topic"
        case .gone: return "gone"
        case .unknown: return "unknown"
        }
    }
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

    /// Signed `GET /v1/campaign/{campaignID}/voucher`. Returns the production
    /// voucher code this account was pre-assigned when the campaign was
    /// published, or `nil` when the account is outside the cohort (the
    /// server's 404, a normal and quiet outcome).
    ///
    /// The offer itself rides `GET /v1/announcements`, a document identical
    /// for every caller, which is what keeps the server from learning who
    /// asks about what. A per-account value cannot ride that document, so it
    /// comes from here, behind the same wallet signature that guards
    /// `/v1/subscription`. The lookup is a pure server-side read that never
    /// mints and never assigns, so repeating it is always safe.
    ///
    /// A failure is a `.failure`, never a `nil`: a transient outage must not
    /// tell a cohort member they were never eligible. The code is a bearer
    /// token worth a month of service: it goes to the account's own screen and
    /// nowhere else, never to a log, an error or a problem report. Blocking,
    /// run off the main thread.
    public static func campaignVoucher(seed: Data, campaignID: String) -> Result<String?, WarrenAccountError> {
        guard seed.count == seedByteCount else { return .failure(.invalidInput("seed must be 32 bytes")) }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return campaignID.withCString { campaignPtr in
                warren_account_campaign_voucher(base, campaignPtr)
            }
        }
        guard let raw else { return .failure(.transport("FFI returned a null result")) }
        defer { warren_wallet_free_mnemonic(raw) }
        return campaignVoucherOutcome(fromEnvelope: String(cString: raw))
    }

    /// Maps the `warren_account_campaign_voucher` JSON envelope:
    /// `{"ok":true,"code":"<code>"}` for a cohort member,
    /// `{"ok":true,"code":null}` for an account outside it, and the shared
    /// `{"ok":false,...}` error shape otherwise. Pure so the three outcomes
    /// are unit-tested off the device.
    static func campaignVoucherOutcome(fromEnvelope envelope: String?) -> Result<String?, WarrenAccountError> {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return .failure(.transport("FFI returned an unparseable envelope"))
        }
        guard object["ok"] as? Bool == true else {
            let message = object["error"] as? String ?? "unknown error"
            if let status = object["status"] as? NSNumber {
                return .failure(.server(status: status.intValue, message: message))
            }
            return .failure(.transport(message))
        }
        // A missing `code` reads exactly like an explicit null: both mean the
        // server named no code for this account.
        return .success(object["code"] as? String)
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
        guard seed.count == seedByteCount else { return .failed(reason: "build") }
        let raw = seed.withUnsafeBytes { rawBuffer -> UnsafeMutablePointer<CChar>? in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return sid.withCString { sidPtr in
                host.withCString { hostPtr in
                    warren_forum_login(base, sidPtr, hostPtr)
                }
            }
        }
        guard let raw else { return .failed(reason: "runtime") }
        defer { warren_wallet_free_mnemonic(raw) }
        return forumLoginOutcome(fromEnvelope: String(cString: raw))
    }

    /// Maps the `warren_forum_login` JSON envelope to an outcome. The shapes
    /// are single-sourced in the Rust `warren-forum` crate and pinned by the
    /// `envelope` column of `fixtures/client-rules/forum_outcomes.json`:
    /// `{"ok":true}` with the additive `handle` and `notify_slot`, or
    /// `{"ok":false,"error":<kind>}` with the additive `reason` on `error`.
    /// The handle is taken as the crate hands it (it validated the shape
    /// before crossing the FFI), as the Android decoder does. Pure so the
    /// mapping is unit-tested off-device.
    static func forumLoginOutcome(fromEnvelope envelope: String?) -> WarrenForumLoginOutcome {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return .failed(reason: "unknown")
        }
        if object["ok"] as? Bool == true {
            guard let handle = object["handle"] as? String else {
                return .approved(nil)
            }
            let slot = (object["notify_slot"] as? NSNumber).map { UInt32(truncating: $0) }
            return .approved(WarrenForumIdentity(handle: handle, notifySlot: slot))
        }
        switch object["error"] as? String {
        case "subscription-required":
            return .subscriptionRequired
        case "clock-skew":
            return .clockSkew
        case "expired":
            return .expired
        default:
            return .failed(reason: object["reason"] as? String ?? "unknown")
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

    // MARK: - Community-forum attach-logs (doc 55)

    /// Signs and submits the attach-logs upload for the forum's "attach your
    /// logs" page (`POST /v1/forum/attach-logs`): `sid` and `host` from the
    /// deep link or the typed code, `topicId` the topic the logs join (0 for a
    /// report still being composed), `logGz` the gzipped redacted problem
    /// report. Everything wire-sensitive happens in Rust; only the opaque
    /// inputs and the gzip cross the boundary, so the wallet signature never
    /// surfaces to Swift. `host` is re-validated against a hard allowlist in
    /// Rust. Blocking (run off the main thread); the seed, sid, signature and
    /// report are never logged.
    public static func forumAttachLogs(
        seed: Data, sid: String, topicId: UInt64, host: String, logGz: Data
    ) -> WarrenForumAttachOutcome {
        guard seed.count == seedByteCount, !logGz.isEmpty else { return .failed(reason: "build") }
        let raw = seed.withUnsafeBytes { seedBuffer -> UnsafeMutablePointer<CChar>? in
            guard let seedBase = seedBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return logGz.withUnsafeBytes { gzBuffer -> UnsafeMutablePointer<CChar>? in
                guard let gzBase = gzBuffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
                return sid.withCString { sidPtr in
                    host.withCString { hostPtr in
                        warren_forum_attach_logs(seedBase, sidPtr, topicId, hostPtr, gzBase, UInt(logGz.count))
                    }
                }
            }
        }
        guard let raw else { return .failed(reason: "runtime") }
        defer { warren_wallet_free_mnemonic(raw) }
        return forumAttachOutcome(fromEnvelope: String(cString: raw))
    }

    /// Best-effort: tells the connect `host` the user declined the attach for
    /// `sid` (`POST /v1/attach/<sid>/cancel`), so the waiting forum page shows
    /// "cancelled" instead of polling to its timeout. Unsigned (no wallet
    /// material); mirrors the desktop `cancelForumAttach`. Blocking, run off
    /// the main thread; failures are ignored (the session expires in 30 min).
    public static func forumAttachCancel(sid: String, host: String) {
        sid.withCString { sidPtr in
            host.withCString { hostPtr in
                warren_forum_attach_cancel(sidPtr, hostPtr)
            }
        }
    }

    /// Places a session id typed by hand before any consent is raised: reads
    /// the login status, the attach status when the first answered 404, and
    /// the attach meta for a pending attach session, which names the topic
    /// (`warren_forum_code_probe`, up to three unsigned GETs in Rust).
    /// Blocking, run off the main thread; the sid is never logged.
    public static func forumCodeProbe(sid: String, host: String) -> WarrenForumCodePlacement {
        let raw = sid.withCString { sidPtr in
            host.withCString { hostPtr in
                warren_forum_code_probe(sidPtr, hostPtr)
            }
        }
        guard let raw else { return .unknown }
        defer { warren_wallet_free_mnemonic(raw) }
        return forumCodePlacement(fromEnvelope: String(cString: raw))
    }

    /// Maps the `warren_forum_attach_logs` JSON envelope to an outcome. The
    /// shapes are single-sourced in the Rust `warren-forum` crate
    /// (`attach_envelope`) and pinned by `WarrenForumAttachOutcomeTests`.
    /// `ok` names an attach; a named error maps to its case, and any
    /// unreadable, unnamed or unrecognised envelope is a generic failure so a
    /// broken envelope can never read as success. Pure, unit-tested off-device.
    static func forumAttachOutcome(fromEnvelope envelope: String?) -> WarrenForumAttachOutcome {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return .failed(reason: "unknown")
        }
        if object["ok"] as? Bool == true {
            return .attached
        }
        switch object["error"] as? String {
        case "not-author":
            return .notAuthor
        case "expired":
            return .expired
        case "too-large":
            return .tooLarge
        case "clock-skew":
            return .clockSkew
        case "server-error":
            return .serverError
        case "error":
            let reason = (object["reason"] as? String).flatMap { $0.isEmpty ? nil : $0 }
            return .failed(reason: reason ?? "unknown")
        default:
            return .failed(reason: "unknown")
        }
    }

    /// Maps the `warren_forum_code_probe` envelope to a placement: `kind` and,
    /// for an attach session, `topic_id` (0 for a pre-topic session). An
    /// attach kind without a usable topic is not offered as one; anything off
    /// the table is `unknown`, which the caller treats as the login flow (it
    /// preflights again before signing). Pure, unit-tested off-device.
    static func forumCodePlacement(fromEnvelope envelope: String?) -> WarrenForumCodePlacement {
        guard let envelope,
            let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let kind = object["kind"] as? String
        else {
            return .unknown
        }
        switch kind {
        case "login":
            return .login
        case "gone":
            return .gone
        case "attach":
            // The topic the meta named, 0 for a report still being composed,
            // within the safe integer every client caps a topic at.
            // A JSON boolean decodes as an NSNumber too; its Objective-C type
            // is the one thing that tells it from 0 or 1 (an NSNumber of 0
            // bridges to `Bool`, so `is Bool` would refuse the pre-topic 0).
            guard let number = object["topic_id"] as? NSNumber, number.objCType.pointee != 0x63 else {
                return .attachWithoutTopic
            }
            let value = number.int64Value
            guard value >= 0, number.doubleValue == Double(value), value <= 9_007_199_254_740_991 else {
                return .attachWithoutTopic
            }
            return .attach(topicId: UInt64(value))
        default:
            return .unknown
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
