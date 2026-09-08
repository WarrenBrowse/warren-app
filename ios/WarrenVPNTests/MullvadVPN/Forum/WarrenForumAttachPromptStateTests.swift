//
//  WarrenForumAttachPromptStateTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The attach consent's transitions, the Android `ForumAttachPromptState`
/// mirrored: what Approve needs, what disarms it, and whether leaving the
/// prompt still tells the provider.
@MainActor
final class WarrenForumAttachPromptStateTests: XCTestCase {
    private let host = "connect.warrenbrowse.com"
    private let sid = "0123456789abcdef0123456789abcdef"

    private func state(topicId: UInt64 = 42) -> WarrenForumAttachPromptState {
        WarrenForumAttachPromptState(link: ForumAttachLink(sid: sid, host: host, topicId: topicId))
    }

    func testAFreshPromptIsArmedAndCancelsOnDecline() {
        let state = state()
        XCTAssertTrue(state.canApprove)
        XCTAssertTrue(state.cancelsOnDecline)
        XCTAssertFalse(state.link.isPreTopic)
        XCTAssertTrue(self.state(topicId: 0).link.isPreTopic)
    }

    func testAnUploadInFlightDisarmsBothButtonsAndATerminalOutcomeDisarmsApproveForGood() {
        let state = state()
        state.begin()
        XCTAssertTrue(state.busy)
        XCTAssertFalse(state.canApprove)
        state.settle(.serverError, message: "later")
        XCTAssertFalse(state.busy)
        XCTAssertEqual(state.failure, "later")
        XCTAssertFalse(state.terminal, "a recovered provider is a retry worth offering")
        XCTAssertTrue(state.canApprove)
        state.settle(.notAuthor, message: "not yours")
        XCTAssertTrue(state.terminal)
        XCTAssertFalse(state.canApprove)
    }

    func testOnlyAGoneSessionStopsTheDeclineFromTellingTheProvider() {
        // A refusal as author or a report over the cap leaves the session
        // pending on the provider, and the forum page polling it; only a
        // session the provider reported gone has nothing left to cancel.
        let refused = state()
        refused.settle(.notAuthor, message: "not yours")
        XCTAssertTrue(refused.cancelsOnDecline)
        let over = state()
        over.settle(.tooLarge, message: "too large")
        XCTAssertTrue(over.cancelsOnDecline)
        let gone = state()
        gone.settle(.expired, message: "expired")
        XCTAssertFalse(gone.cancelsOnDecline)
    }

    func testAnAttachedUploadEndsTheBusyStateWithNoFailureLeft() {
        let state = state()
        state.settle(.serverError, message: "later")
        state.begin()
        state.markAttached()
        XCTAssertFalse(state.busy)
        XCTAssertNil(state.failure)
    }

    func testBeginClearsTheLastFailureAndAStaleLinkFailsWithoutAnAttempt() {
        let state = state(topicId: 0)
        state.fail(message: "expired")
        XCTAssertEqual(state.failure, "expired")
        XCTAssertFalse(state.busy, "a stale link never starts an upload")
        state.begin()
        XCTAssertNil(state.failure)
    }

    func testThePreviewCollectsOnceAndReportsItsOwnFailure() {
        let state = state()
        state.beginCollect()
        XCTAssertTrue(state.collecting)
        state.previewReady("System information:\n")
        XCTAssertFalse(state.collecting)
        XCTAssertEqual(state.preview, "System information:\n")
        state.closePreview()
        XCTAssertNil(state.preview)
        state.beginCollect()
        state.previewFailed()
        XCTAssertFalse(state.collecting)
        XCTAssertTrue(state.collectFailed)
        state.beginCollect()
        XCTAssertFalse(state.collectFailed, "a new attempt clears the last failure")
    }
}
