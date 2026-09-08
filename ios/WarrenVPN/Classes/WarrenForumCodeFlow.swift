//
//  WarrenForumCodeFlow.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Places a session id typed under "Sign in to the forum with a code" and
//  raises the consent it calls for. The forum's attach page prints its session
//  id in the same shape as the sign-in page's code, and a reader whose "Open
//  the app" button did nothing types whichever they see; before the probe
//  existed such a code was preflighted as a login and answered "expired"
//  (topic 199, 2026-09-07). The probe reads the unsigned status endpoints in
//  Rust (`WarrenAccountClient.forumCodeProbe`), the attach meta included,
//  which names the topic the code cannot carry: an attach session opens the
//  attach consent with its topic, anything else the login consent, which
//  preflights again before signing, so an unplaced code costs nothing it did
//  not cost before. The reads are bounded: the screen shows progress
//  meanwhile, and a broker that does not answer falls back to the login flow.
//

import Foundation
import UIKit
import WarrenLogging
import WarrenRustRuntime

final class WarrenForumCodeFlow: @unchecked Sendable {
    /// The bound on the placement: up to three broker reads, each on the
    /// transport's own 15 s. Past it the login consent is raised, which
    /// preflights again before signing. Android's `PROBE_TIMEOUT_MILLIS`.
    static let probeTimeout: TimeInterval = 20

    private let logger = Logger(label: "WarrenForumCode")
    private let anchors: WarrenProductAnchors
    private let journal: WarrenForumEventsJournal
    private let loginFlow: WarrenForumLoginFlow
    private let attachFlow: WarrenForumAttachFlow
    private let probe: @Sendable (String, String) -> WarrenForumCodePlacement
    private let probeTimeout: TimeInterval

    init(
        anchors: WarrenProductAnchors = .current,
        journal: WarrenForumEventsJournal,
        loginFlow: WarrenForumLoginFlow,
        attachFlow: WarrenForumAttachFlow,
        probe: @escaping @Sendable (String, String) -> WarrenForumCodePlacement = {
            WarrenAccountClient.forumCodeProbe(sid: $0, host: $1)
        },
        probeTimeout: TimeInterval = WarrenForumCodeFlow.probeTimeout
    ) {
        self.anchors = anchors
        self.journal = journal
        self.loginFlow = loginFlow
        self.attachFlow = attachFlow
        self.probe = probe
        self.probeTimeout = probeTimeout
    }

    /// Hands a typed code to the flow. Returns `false` when it is not a session
    /// id, in which case the screen shows why; otherwise the code is probed off
    /// the main thread, bounded by `probeTimeout`, and routed to the attach or
    /// the login consent, after which `placed` runs on the main thread so the
    /// screen can leave.
    @MainActor
    func submit(code: String, placed: @escaping @MainActor () -> Void) -> Bool {
        guard let sid = WarrenForumLinks.normalizeSignInCode(code) else { return false }
        let host = anchors.connectHost
        let probe = self.probe
        let timeout = probeTimeout
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let box = PlacementBox()
            let group = DispatchGroup()
            group.enter()
            DispatchQueue.global(qos: .userInitiated).async {
                box.set(probe(sid, host))
                group.leave()
            }
            // A broker that does not answer in time is not waited for: the
            // login consent is raised with the class `timeout`, and a late
            // answer is dropped.
            let placement = group.wait(timeout: .now() + timeout) == .timedOut ? nil : box.get()
            DispatchQueue.main.async {
                placed()
                self?.route(sid: sid, host: host, placement: placement)
            }
        }
        return true
    }

    @MainActor
    private func route(sid: String, host: String, placement: WarrenForumCodePlacement?) {
        let probeClass = placement?.journalClass ?? "timeout"
        if case let .attach(topicId)? = placement {
            journal.record(
                .linkReceived, .verdict("accepted"), .source(.typedCode), .kind(.attach), .class(probeClass),
                .preTopic(topicId == ForumAttachLink.preTopic))
            logger.info("typed code placed: attach")
            attachFlow.requestFromCode(WarrenForumLinks.attachLinkFromCode(sid, host: host, topicId: topicId))
            return
        }
        // The login flow preflights again before signing, so a gone, unknown
        // or unanswered code is no worse off here than a login code always was.
        journal.record(
            .linkReceived, .verdict("accepted"), .source(.typedCode), .kind(.login), .class(probeClass))
        logger.info("typed code placed: login (\(probeClass))")
        _ = loginFlow.handle(code: sid)
    }
}

/// The probe's answer, handed from the probing thread to the waiting one.
private final class PlacementBox: @unchecked Sendable {
    private let lock = NSLock()
    private var placement: WarrenForumCodePlacement?

    func set(_ value: WarrenForumCodePlacement) {
        lock.lock()
        placement = value
        lock.unlock()
    }

    func get() -> WarrenForumCodePlacement? {
        lock.lock()
        defer { lock.unlock() }
        return placement
    }
}
