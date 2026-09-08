//
//  WarrenForumAttach.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The forum page's "attach your logs" flow on iOS (doc 55). A
//  `<scheme>://attach-logs?sid=..&topic=..&host=..` deep link, or a session
//  id typed under Settings that the broker holds as an attach session, asks
//  the app to attach its redacted problem report to a forum bug report. The
//  app NEVER uploads silently: the report is collected and sent only after
//  the user approves the consent prompt, and the exact report can be read
//  first ("View the logs"). The link rules mirror the login's and are pinned
//  by the same fixture (`fixtures/client-rules/forum_link.json`); the wire
//  bytes and outcome mapping live in the Rust `warren-forum` crate.
//

import Foundation
import SwiftUI
import UIKit
import WarrenLogging
import WarrenRustRuntime

/// A validated `<scheme>://attach-logs` deep link: the forum's "attach your
/// logs" page asking the app to attach its redacted problem report to
/// `topicId`, or, when it is 0, to a report still being composed (the forum
/// binds the logs to the topic after creation).
///
/// `topicId` is `nil` for a session id typed by hand: the broker's status
/// endpoints carry no topic id, so the consent prompt asks for it.
struct ForumAttachLink: Equatable {
    let sid: String
    let host: String
    let topicId: UInt64?

    /// The topic id of a report still being composed.
    static let preTopic: UInt64 = 0

    var isPreTopic: Bool { topicId == Self.preTopic }
}

/// An attach link's verdict; a rejection names its class only, like the login's.
enum ForumAttachVerdict: Equatable {
    case accepted(ForumAttachLink)
    case rejected(String)
}

extension WarrenForumLinks {
    /// The two deep-link actions, one URL scheme serving both flows.
    static let loginAction = "forum-login"
    static let attachAction = "attach-logs"

    /// The largest topic id every platform accepts: a JavaScript safe integer,
    /// because the desktop carries the value through a `number`
    /// (`fixtures/client-rules/forum_link.json`).
    static let maxForumTopicID: UInt64 = (1 << 53) - 1

    /// The action of `raw` (the host, or the first path segment for a
    /// `scheme:///action` shape), or `nil` when it is not a URI. The scene
    /// picks the parser by this before either flow's classifier runs.
    static func action(of raw: String?) -> String? {
        guard let raw, let components = URLComponents(string: raw), components.scheme != nil else {
            return nil
        }
        return components.host ?? components.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    }

    /// A topic id as a deep link, or the consent prompt's field, spells it:
    /// decimal digits only (so no sign), within `maxForumTopicID`; `nil`
    /// otherwise.
    static func parseTopicId(_ text: String) -> UInt64? {
        guard !text.isEmpty, text.allSatisfy({ $0.isASCII && $0.isNumber }), let value = UInt64(text) else {
            return nil
        }
        return value <= maxForumTopicID ? value : nil
    }

    /// The attach request a session id typed by hand stands for, against the
    /// one allowlisted host. The topic is unknown, so the prompt asks for it.
    static func attachLinkFromCode(_ sid: String, host: String) -> ForumAttachLink {
        ForumAttachLink(sid: sid, host: host, topicId: nil)
    }

    /// Classifies `raw` as an attach-logs link. Classes: the login parser's
    /// (`no-data`, `not-a-uri`, `wrong-scheme:<scheme>`, `wrong-action`,
    /// `missing-sid`, `missing-host`, `host-not-allowlisted`, `bad-sid-shape`)
    /// plus `missing-topic` and `bad-topic`. The Rust layer re-validates the
    /// sid and host before signing, so this is a fail-fast guard.
    static func classifyAttach(_ raw: String?, expectedScheme: String, allowedHost: String)
        -> ForumAttachVerdict
    {
        guard let raw else { return .rejected("no-data") }
        guard let components = URLComponents(string: raw), let scheme = components.scheme else {
            return .rejected("not-a-uri")
        }
        guard scheme == expectedScheme else { return .rejected("wrong-scheme:\(scheme)") }
        let action = components.host ?? components.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard action == attachAction else { return .rejected("wrong-action") }
        var params: [String: String] = [:]
        for item in components.queryItems ?? [] where params[item.name] == nil {
            params[item.name] = item.value ?? ""
        }
        guard let sid = params["sid"] else { return .rejected("missing-sid") }
        guard let host = params["host"] else { return .rejected("missing-host") }
        guard let topic = params["topic"] else { return .rejected("missing-topic") }
        guard isValidSid(sid) else { return .rejected("bad-sid-shape") }
        guard let topicId = parseTopicId(topic) else { return .rejected("bad-topic") }
        guard host == allowedHost else { return .rejected("host-not-allowlisted") }
        return .accepted(ForumAttachLink(sid: sid, host: host, topicId: topicId))
    }
}

/// The attach consent's state for one pending link, the Android
/// `ForumAttachPromptState` mirrored. Plain observable state so the
/// transitions are unit-tested off-device (`WarrenForumAttachPromptStateTests`).
@MainActor
final class WarrenForumAttachPromptState: ObservableObject {
    let link: ForumAttachLink

    /// The link carries no topic (a typed code), so the prompt shows the topic
    /// field; an empty field means a report still being composed.
    let needsTopic: Bool

    /// What the topic field holds: digits only, whatever was typed or pasted.
    @Published private(set) var topicInput = ""
    /// An upload is out: Approve and Cancel are disabled.
    @Published private(set) var busy = false
    /// The inline message of the last non-attached outcome, if any.
    @Published private(set) var failure: String?
    /// No retry can change the outcome, so Approve is disarmed for good.
    @Published private(set) var terminal = false
    /// "View the logs" is collecting the report.
    @Published private(set) var collecting = false
    @Published private(set) var collectFailed = false
    /// The collected report on screen, or `nil` when the preview is closed.
    @Published private(set) var preview: String?

    init(link: ForumAttachLink) {
        self.link = link
        needsTopic = link.topicId == nil
    }

    func updateTopicInput(_ text: String) {
        topicInput = String(text.filter { $0.isASCII && $0.isNumber })
    }

    /// The topic the approval sends: the link's own, else the field's, an
    /// empty field standing for a report still being composed. `nil` when the
    /// field holds a number no topic can have.
    func topicIdOrNil() -> UInt64? {
        if let topicId = link.topicId { return topicId }
        return topicInput.isEmpty ? ForumAttachLink.preTopic : WarrenForumLinks.parseTopicId(topicInput)
    }

    var canApprove: Bool { !busy && !terminal && topicIdOrNil() != nil }

    /// The user approved: the upload is in flight.
    func begin() {
        busy = true
        failure = nil
    }

    /// A non-attached `outcome` came back, rendered as `message`.
    func settle(_ outcome: WarrenForumAttachOutcome, message: String) {
        busy = false
        terminal = outcome.isTerminal
        failure = message
    }

    /// A message for the current link without an attempt (a stale link).
    func fail(message: String) {
        failure = message
    }

    func beginCollect() {
        collecting = true
        collectFailed = false
    }

    func previewReady(_ text: String) {
        collecting = false
        preview = text
    }

    func previewFailed() {
        collecting = false
        collectFailed = true
    }

    func closePreview() {
        preview = nil
    }
}

/// Drives one attach-logs request from a link or a typed code to its result.
/// Owned by the app delegate; the scene that owns the window supplies the
/// presenter and the tunnel state. The app NEVER uploads silently: every
/// accepted link goes through the consent prompt. `@unchecked Sendable` like
/// the login flow: the UI touches are `@MainActor`, the collect-and-upload
/// runs on a background queue and touches only the journal and the logger.
final class WarrenForumAttachFlow: @unchecked Sendable {
    private let logger = Logger(label: "WarrenForumAttach")
    private let anchors: WarrenProductAnchors
    private let journal: WarrenForumEventsJournal

    /// Where the prompt is presented, resolved at presentation time so it
    /// lands over whatever is on screen.
    @MainActor var presenter: (() -> UIViewController?)?
    /// The tunnel's state at the moment of the upload; unset reads as settled.
    @MainActor var tunnelState: (() -> TunnelState)?

    @MainActor private weak var presented: UIViewController?

    init(anchors: WarrenProductAnchors = .current, journal: WarrenForumEventsJournal) {
        self.anchors = anchors
        self.journal = journal
    }

    /// A deep link handed to the scene (cold start or `openURLContexts`).
    /// Rejected links are journaled by class and dropped.
    @MainActor
    func handle(url: URL, coldStart: Bool) {
        switch WarrenForumLinks.classifyAttach(
            url.absoluteString, expectedScheme: anchors.deepLinkScheme, allowedHost: anchors.connectHost)
        {
        case .accepted(let link):
            journal.record(
                .linkReceived, .verdict("accepted"), .source(.deepLink), .kind(.attach),
                .preTopic(link.isPreTopic), .coldStart(coldStart))
            present(link)
        case .rejected(let reason):
            logger.info("attach link rejected: \(reason)")
            journal.record(.linkReceived, .verdict(reason), .kind(.attach))
        }
    }

    /// A session id the code flow placed as an attach session.
    @MainActor
    func requestFromCode(_ link: ForumAttachLink) {
        present(link)
    }

    @MainActor
    private func present(_ link: ForumAttachLink) {
        guard let presenter = presenter?() else {
            logger.warning("forum attach prompt has nowhere to present")
            return
        }
        let state = WarrenForumAttachPromptState(link: link)
        let view = WarrenForumAttachConsentView(
            state: state,
            onApprove: { [weak self] in self?.approve(state) },
            onCancel: { [weak self] in self?.cancel(state) },
            onViewLogs: { [weak self] in self?.viewLogs(state) })
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        presented = host
        presenter.present(host, animated: true)
    }

    @MainActor
    private func dismiss() {
        presented?.dismiss(animated: true)
        presented = nil
    }

    @MainActor
    private func approve(_ state: WarrenForumAttachPromptState) {
        guard !state.busy, let topicId = state.topicIdOrNil() else { return }
        // A request signed while the tunnel is between states cannot resolve
        // the broker's host name, so the flow reads the tunnel and defers
        // instead of spending the session on it (the login's rule).
        if case let .deferred(tunnelClass) = WarrenForumPreflight.verdict(
            for: tunnelState?() ?? .disconnected)
        {
            logger.info("forum attach deferred: tunnel \(tunnelClass)")
            journal.record(.attachDeferred, .class(tunnelClass))
            state.fail(message: Self.tunnelBusyMessage)
            return
        }
        state.begin()
        let link = state.link
        journal.record(.attachSigning, .preTopic(topicId == ForumAttachLink.preTopic))
        let started = Date()
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            let outcome = self.collectAndAttach(link: link, topicId: topicId)
            let elapsed = Int(Date().timeIntervalSince(started) * 1000)
            self.journal.record(.attachResult, .class(outcome.journalClass), .elapsedMs(elapsed))
            DispatchQueue.main.async {
                switch outcome {
                case .attached:
                    self.dismiss()
                default:
                    state.settle(outcome, message: Self.message(for: outcome, link: link))
                }
            }
        }
    }

    /// Off the main thread: load the wallet, collect and gzip the report, then
    /// sign and POST it in Rust. The seed is forgotten right after. Not
    /// `@MainActor`: it touches only the journal, the logger and static APIs.
    private func collectAndAttach(link: ForumAttachLink, topicId: UInt64) -> WarrenForumAttachOutcome {
        guard let mnemonic = try? WarrenWalletKeychain.load(),
            let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
        else {
            return .failed(reason: "wallet-absent")
        }
        defer { wallet.forgetSecret() }
        let address = wallet.publicKeyAddress
        let gz: Data
        do {
            gz = try WarrenProblemReport.collectGzipped(
                walletAddress: address.isEmpty ? nil : address, journalURL: journal.fileURL)
        } catch {
            logger.warning("forum attach: report collection failed")
            return .failed(reason: "collect-failed")
        }
        return WarrenAccountClient.forumAttachLogs(
            seed: wallet.seed, sid: link.sid, topicId: topicId, host: link.host, logGz: gz)
    }

    @MainActor
    private func cancel(_ state: WarrenForumAttachPromptState) {
        guard !state.busy else { return }
        let link = state.link
        if !state.terminal {
            journal.record(.attachDeclined)
            DispatchQueue.global(qos: .utility).async {
                WarrenAccountClient.forumAttachCancel(sid: link.sid, host: link.host)
            }
        }
        dismiss()
    }

    @MainActor
    private func viewLogs(_ state: WarrenForumAttachPromptState) {
        guard !state.collecting, !state.busy else { return }
        state.beginCollect()
        let journalURL = journal.fileURL
        DispatchQueue.global(qos: .userInitiated).async {
            // Nothing leaves the device for the preview.
            let address = (try? WarrenWalletKeychain.load())
                .flatMap { try? WarrenWallet.fromMnemonic($0) }
                .map { wallet -> String in
                    defer { wallet.forgetSecret() }
                    return wallet.publicKeyAddress
                }
            let text = WarrenProblemReport.collect(
                walletAddress: (address?.isEmpty == false) ? address : nil, journalURL: journalURL)
            DispatchQueue.main.async {
                if text.isEmpty {
                    state.previewFailed()
                } else {
                    state.previewReady(text)
                }
            }
        }
    }

    private static func message(for outcome: WarrenForumAttachOutcome, link: ForumAttachLink) -> String {
        switch outcome {
        case .attached:
            return NSLocalizedString(
                "Your logs reached the Warren support team.", comment: "Forum attach success")
        case .notAuthor:
            return NSLocalizedString(
                "Only the author of the bug report can attach logs to it. This report was posted from another forum account.",
                comment: "Forum attach refused, the signer is not the topic author")
        case .expired:
            // A typed code carries a topic the person supplied, and the
            // provider answers a mismatch with the same 404 as an expiry, so
            // that message names both.
            return link.topicId == nil
                ? NSLocalizedString(
                    "This code has expired, or the topic number does not match it. Check the forum page and try again.",
                    comment: "Forum attach refused, a typed code that is gone or points at the wrong topic")
                : NSLocalizedString(
                    "This request has expired. Start again from the forum page.",
                    comment: "Forum attach refused, the session is gone")
        case .tooLarge:
            return NSLocalizedString(
                "The logs are too large to send.", comment: "Forum attach refused, the report is over the cap")
        case .clockSkew:
            return NSLocalizedString(
                "This device's clock is off by more than a minute. Enable automatic date and time, then send again.",
                comment: "Forum attach refused, the device clock is outside the accepted window")
        case .serverError:
            return NSLocalizedString(
                "The Warren support service could not take the logs. Nothing is wrong on your side; try again later.",
                comment: "Forum attach failed on the provider's own side")
        case .failed(let reason) where reason == "wallet-absent":
            return NSLocalizedString(
                "Set up your Warren wallet first.", comment: "Forum attach refused, no wallet on this device")
        case .failed:
            return NSLocalizedString(
                "Attaching the logs failed. Please try again in a moment.", comment: "Forum attach failed")
        }
    }

    private static var tunnelBusyMessage: String {
        NSLocalizedString(
            "The VPN is connecting or blocked. Wait for it to connect, or disconnect it, then try again.",
            comment: "Forum request not attempted, the tunnel is between states")
    }
}
