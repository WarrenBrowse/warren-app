//
//  WarrenForumLogin.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The community-forum wallet login on iOS (warren-core doc 55): the deep
//  link classification, the sign-in code typed by hand, the consent prompt,
//  the signed approval done in Rust, and the identity the broker hands back.
//  The link rules are the fixture's (`fixtures/client-rules/forum_link.json`),
//  replayed by `WarrenForumLinkTests`; the scheme and the connect host come
//  from the Rust product table, never from a literal here.
//

import Foundation
import UIKit
import WarrenLogging
import WarrenRustRuntime

/// A validated `<scheme>://forum-login` deep link.
///
/// `crossDevice` means the link came from the QR on the approval page, so the
/// browser signing in is on another device. That is also exactly what a
/// relayed (phished) approval looks like, and nothing on the wire tells the
/// two apart, so the consent prompt says which one it is and lets the person
/// decide.
struct ForumLoginLink: Equatable {
    let sid: String
    let host: String
    let crossDevice: Bool
}

/// A deep link's verdict. A rejection names its class only (never the
/// values): a scheme or host drift between the broker and the app is exactly
/// what a report has to be able to show, and it was invisible while a
/// rejected link was dropped in silence.
enum ForumLinkVerdict: Equatable {
    case accepted(ForumLoginLink)
    case rejected(String)
}

/// The link rules, mirrored from the Rust `warren-forum` crate for the
/// fail-fast check before any native call (Rust re-validates the host and the
/// sid before signing, so this is not the security boundary).
enum WarrenForumLinks {
    /// The forum SSO session id shape: exactly 32 lowercase hex characters.
    static func isValidSid(_ sid: String) -> Bool {
        sid.utf8.count == 32
            && sid.utf8.allSatisfy { byte in
                (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte)
                    || (UInt8(ascii: "a")...UInt8(ascii: "f")).contains(byte)
            }
    }

    /// Classifies `raw` against the scheme this build registers and the one
    /// allowlisted connect host. Classes: `no-data`, `not-a-uri`,
    /// `wrong-scheme:<scheme>`, `wrong-action`, `missing-sid`, `missing-host`,
    /// `bad-sid-shape`, `host-not-allowlisted`.
    static func classify(_ raw: String?, expectedScheme: String, allowedHost: String) -> ForumLinkVerdict {
        guard let raw else { return .rejected("no-data") }
        // A URL with no scheme is not a URI in this sense; `URLComponents` is
        // lenient about the rest (it percent-encodes what it can).
        guard let components = URLComponents(string: raw), let scheme = components.scheme else {
            return .rejected("not-a-uri")
        }
        // The received scheme is a product-environment name, not identity
        // material: it is the one fact that tells a prod/beta mismatch apart.
        guard scheme == expectedScheme else { return .rejected("wrong-scheme:\(scheme)") }
        // `warren://forum-login?..` parses with the action as the host.
        let action = components.host ?? components.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard action == "forum-login" else { return .rejected("wrong-action") }
        var params: [String: String] = [:]
        for item in components.queryItems ?? [] where params[item.name] == nil {
            params[item.name] = item.value ?? ""
        }
        guard let sid = params["sid"] else { return .rejected("missing-sid") }
        guard let host = params["host"] else { return .rejected("missing-host") }
        guard isValidSid(sid) else { return .rejected("bad-sid-shape") }
        guard host == allowedHost else { return .rejected("host-not-allowlisted") }
        // The provider sets `xd=1` on the QR link only. Anything else, an
        // older provider included, is the same-device button and gets the
        // ordinary prompt rather than a warning nobody can act on.
        return .accepted(ForumLoginLink(sid: sid, host: host, crossDevice: params["xd"] == "1"))
    }

    /// A sign-in code as a person types it: the 32 hex characters of the
    /// session id, in any case, with any spaces or dashes a display may have
    /// grouped them with. The canonical sid, or `nil` for anything else.
    static func normalizeSignInCode(_ typed: String) -> String? {
        let cleaned = String(
            typed.lowercased().filter { character in
                !character.isWhitespace && character != "-"
            })
        return isValidSid(cleaned) ? cleaned : nil
    }

    /// The link a sign-in code typed by hand stands for: the same request a
    /// deep link would carry, against the one allowlisted host.
    ///
    /// Marked cross-device, which is the honest reading: there is no link and
    /// no `xd` signal, so the app cannot tell a code the user read off this
    /// screen from one an attacker sent them ("paste this in Settings to
    /// finish your sign-in"). Only the cross-device prompt says that approving
    /// hands the forum identity to whoever sent the code, and that is exactly
    /// this case.
    static func linkFromCode(_ sid: String, host: String) -> ForumLoginLink {
        ForumLoginLink(sid: sid, host: host, crossDevice: true)
    }
}

/// Drives one forum login from a link or a typed code to its result alert.
/// Owned by the app delegate; the scene that owns the window supplies the
/// presenter. The app NEVER signs into the forum silently: every accepted link
/// goes through the consent prompt.
final class WarrenForumLoginFlow: @unchecked Sendable {
    private let logger = Logger(label: "WarrenForumLogin")
    private let anchors: WarrenProductAnchors

    /// The view controller prompts are presented on, resolved at presentation
    /// time so the prompt lands over whatever is on screen.
    @MainActor var presenter: (() -> UIViewController?)?

    /// The tunnel's state at the moment of the signature, read through the
    /// app delegate's tunnel manager. Unset (a flow with no tunnel behind it,
    /// and the tests) reads as settled, which is what it was before the
    /// preflight existed.
    @MainActor var tunnelState: (() -> TunnelState)?

    init(anchors: WarrenProductAnchors = .current) {
        self.anchors = anchors
    }

    /// A deep link handed to the scene (cold start or `openURLContexts`).
    /// Non-forum URLs and rejected links are logged by class and dropped.
    @MainActor
    func handle(url: URL) {
        switch WarrenForumLinks.classify(
            url.absoluteString, expectedScheme: anchors.deepLinkScheme, allowedHost: anchors.connectHost)
        {
        case .accepted(let link):
            presentConsent(link)
        case .rejected(let reason):
            logger.info("forum link rejected: \(reason)")
        }
    }

    /// A sign-in code typed under Settings. Raises the same consent prompt a
    /// deep link would; returns false when the code is not a session id.
    @MainActor
    func handle(code: String) -> Bool {
        guard let sid = WarrenForumLinks.normalizeSignInCode(code) else { return false }
        presentConsent(WarrenForumLinks.linkFromCode(sid, host: anchors.connectHost))
        return true
    }

    @MainActor
    private func presentConsent(_ link: ForumLoginLink) {
        guard let presenter = presenter?() else {
            logger.warning("forum login prompt has nowhere to present")
            return
        }
        let title =
            link.crossDevice
            ? NSLocalizedString(
                "Sign in to the forum on another device?",
                comment: "Forum login consent prompt title, cross device")
            : NSLocalizedString(
                "Sign in to the Warren community forum?",
                comment: "Forum login consent prompt title")
        // Two paragraphs, localized separately, word for word the desktop and
        // Android prompt: one flow, one wording, one set of translations. The
        // cross-device pair names no origin, because a code typed under
        // Settings raises this same prompt and has no QR anywhere in its flow.
        let first =
            link.crossDevice
            ? NSLocalizedString(
                "Warren cannot tell which browser is being signed in, or whether it is in front of you right now. Your app will sign a one-time challenge with your wallet key to prove it is you.",
                comment: "Forum login consent prompt body, cross device")
            : NSLocalizedString(
                "A sign-in to the Warren community forum was requested. Your app will sign a one-time challenge with your wallet key to prove it is you.",
                comment: "Forum login consent prompt body")
        let second =
            link.crossDevice
            ? NSLocalizedString(
                "Approve only if you are looking at that sign-in page right now. If someone sent you this code, they are signing in as you. No email and no password are used.",
                comment: "Forum login consent prompt warning, cross device")
            : NSLocalizedString(
                "No email and no password are used. You appear under an anonymous handle that cannot be linked to your Warren account. Only approve if you started this sign-in.",
                comment: "Forum login consent prompt warning")
        let message = first + "\n\n" + second
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString(
                    "Cancel", comment: "Forum login consent prompt, the refusing button"),
                style: .cancel,
                handler: { _ in Self.notifyCancelled(link) }))
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString(
                    "Approve sign-in", comment: "Forum login consent prompt, the approving button"),
                style: .default,
                handler: { [weak self] _ in self?.perform(link) }))
        presenter.present(alert, animated: true)
    }

    /// Best-effort cancel notify so the waiting browser page unblocks
    /// (mirrors the desktop). Fire-and-forget off the main thread; connect
    /// drops the session on its own after five minutes.
    private static func notifyCancelled(_ link: ForumLoginLink) {
        DispatchQueue.global(qos: .utility).async {
            WarrenAccountClient.forumLoginCancel(sid: link.sid, host: link.host)
        }
    }

    /// Loads the wallet seed silently and signs + POSTs the challenge in Rust
    /// off the main thread (the Keychain item is `WhenUnlockedThisDeviceOnly`,
    /// no biometric gate, matching the tunnel which signs with the same key).
    /// An approved answer's identity is stored for the account screen.
    @MainActor
    private func perform(_ link: ForumLoginLink) {
        // The POST leaves over the ordinary stack, but the broker's host name
        // is resolved by the system resolver, which points at the tunnel's
        // DNS while it is coming up or going down and at nothing at all while
        // it blocks. Signing then spends the session on a request that cannot
        // arrive, and the browser page waits for an approval that never
        // lands. Android defers on the same verdict.
        if case let .deferred(tunnelClass) = WarrenForumPreflight.verdict(
            for: tunnelState?() ?? .disconnected)
        {
            logger.info("forum login deferred: tunnel \(tunnelClass)")
            presentTunnelBusy()
            return
        }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let outcome: WarrenForumLoginOutcome
            if let mnemonic = try? WarrenWalletKeychain.load(),
                let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
            {
                defer { wallet.forgetSecret() }
                outcome = WarrenAccountClient.forumLogin(seed: wallet.seed, sid: link.sid, host: link.host)
            } else {
                outcome = .failed(reason: "wallet-absent")
            }
            if case .approved(let identity?) = outcome {
                do {
                    try WarrenForumIdentityStore.save(identity)
                } catch {
                    self?.logger.error("forum identity could not be stored: \(error)")
                }
            }
            self?.logger.info("forum login result: \(Self.resultClass(outcome))")
            DispatchQueue.main.async { self?.presentResult(outcome) }
        }
    }

    /// The class for the log: never a handle, never a sid.
    private static func resultClass(_ outcome: WarrenForumLoginOutcome) -> String {
        switch outcome {
        case .approved(let identity): identity == nil ? "approved" : "approved with identity"
        case .subscriptionRequired: "subscription-required"
        case .clockSkew: "clock-skew"
        case .expired: "expired"
        case .failed(let reason): "failed (\(reason))"
        }
    }

    /// Nothing was signed and the session is untouched: the person can start
    /// again from the browser page, or from the code, once the tunnel settles.
    @MainActor
    private func presentTunnelBusy() {
        guard let presenter = presenter?() else { return }
        let alert = UIAlertController(
            title: nil,
            message: NSLocalizedString(
                "The VPN is connecting or blocked. Wait for it to connect, or disconnect it, then try again.",
                comment: "Forum request not attempted, the tunnel is between states"),
            preferredStyle: .alert)
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("OK", comment: "Forum login result alert, the dismissing button"),
                style: .default))
        presenter.present(alert, animated: true)
    }

    @MainActor
    private func presentResult(_ outcome: WarrenForumLoginOutcome) {
        guard let presenter = presenter?() else { return }
        let message: String
        switch outcome {
        case .approved:
            message = NSLocalizedString(
                "Signed in to the Warren forum.", comment: "Forum login success")
        case .subscriptionRequired:
            message = NSLocalizedString(
                "Forum access requires a Warren subscription. This wallet has never subscribed.",
                comment: "Forum login refused, no subscription")
        case .clockSkew:
            message = NSLocalizedString(
                "Sign-in refused: this device's clock is off by more than a minute. Enable automatic date and time, then start again from the browser page.",
                comment: "Forum login refused, device clock outside the accepted window")
        case .expired:
            message = NSLocalizedString(
                "This sign-in request has expired. Start again from the browser page.",
                comment: "Forum login refused, the session is gone")
        case .failed(let reason) where reason == "wallet-absent":
            message = NSLocalizedString(
                "Set up your Warren wallet first.", comment: "Forum login refused, no wallet on this device")
        case .failed:
            message = NSLocalizedString(
                "Sign-in failed. Please try again in a moment.", comment: "Forum login failed")
        }
        let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("OK", comment: "Forum login result alert, the dismissing button"),
                style: .default))
        presenter.present(alert, animated: true)
    }
}
