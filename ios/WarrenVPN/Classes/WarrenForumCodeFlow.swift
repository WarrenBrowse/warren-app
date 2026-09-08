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
//  (topic 199, 2026-09-07). The probe reads the two unsigned status endpoints
//  in Rust (`WarrenAccountClient.forumCodeProbe`); an attach session opens the
//  attach consent, anything else the login consent, which preflights again
//  before signing, so an unplaced code costs nothing it did not cost before.
//

import Foundation
import UIKit
import WarrenLogging
import WarrenRustRuntime

final class WarrenForumCodeFlow: @unchecked Sendable {
    private let logger = Logger(label: "WarrenForumCode")
    private let anchors: WarrenProductAnchors
    private let journal: WarrenForumEventsJournal
    private let loginFlow: WarrenForumLoginFlow
    private let attachFlow: WarrenForumAttachFlow

    init(
        anchors: WarrenProductAnchors = .current,
        journal: WarrenForumEventsJournal,
        loginFlow: WarrenForumLoginFlow,
        attachFlow: WarrenForumAttachFlow
    ) {
        self.anchors = anchors
        self.journal = journal
        self.loginFlow = loginFlow
        self.attachFlow = attachFlow
    }

    /// Hands a typed code to the flow. Returns `false` when it is not a session
    /// id, in which case the screen shows why; otherwise the code is probed off
    /// the main thread and routed to the attach or the login consent.
    @MainActor
    func submit(code: String) -> Bool {
        guard let sid = WarrenForumLinks.normalizeSignInCode(code) else { return false }
        let host = anchors.connectHost
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let kind = WarrenAccountClient.forumCodeProbe(sid: sid, host: host)
            DispatchQueue.main.async {
                self?.route(sid: sid, host: host, kind: kind)
            }
        }
        return true
    }

    @MainActor
    private func route(sid: String, host: String, kind: WarrenForumCodeKind) {
        let linkKind: ForumLinkKind = kind == .attach ? .attach : .login
        journal.record(
            .linkReceived, .verdict("accepted"), .source(.typedCode), .kind(linkKind), .class(kind.rawValue))
        logger.info("typed code placed: \(kind.rawValue)")
        switch linkKind {
        case .attach:
            attachFlow.requestFromCode(WarrenForumLinks.attachLinkFromCode(sid, host: host))
        case .login:
            // The login flow preflights again before signing, so a gone or
            // unknown code is no worse off here than a login code always was.
            _ = loginFlow.handle(code: sid)
        }
    }
}
