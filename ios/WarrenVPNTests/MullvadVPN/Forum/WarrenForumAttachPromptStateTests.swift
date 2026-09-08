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
/// mirrored: what Approve needs, what disarms it, and what a typed code's
/// empty topic field stands for.
@MainActor
final class WarrenForumAttachPromptStateTests: XCTestCase {
    private let host = "connect.warrenbrowse.com"
    private let sid = "0123456789abcdef0123456789abcdef"

    func testALinkWithATopicNeedsNoFieldAndSendsItsOwnTopic() {
        let state = WarrenForumAttachPromptState(link: ForumAttachLink(sid: sid, host: host, topicId: 42))
        XCTAssertFalse(state.needsTopic)
        XCTAssertEqual(state.topicIdOrNil(), 42)
        XCTAssertTrue(state.canApprove)
    }

    func testATypedCodeAsksForTheTopicAndAnEmptyFieldIsTheReportStillBeingComposed() {
        let state = WarrenForumAttachPromptState(link: ForumAttachLink(sid: sid, host: host, topicId: nil))
        XCTAssertTrue(state.needsTopic)
        XCTAssertEqual(state.topicIdOrNil(), ForumAttachLink.preTopic)
        XCTAssertTrue(state.canApprove)
        state.updateTopicInput("t-1 98")
        XCTAssertEqual(state.topicInput, "198", "only digits survive, whatever was pasted")
        XCTAssertEqual(state.topicIdOrNil(), 198)
        state.updateTopicInput("99999999999999999999")
        XCTAssertNil(state.topicIdOrNil(), "a number no topic can have")
        XCTAssertFalse(state.canApprove)
    }

    func testAnUploadInFlightDisarmsBothButtonsAndATerminalOutcomeDisarmsApproveForGood() {
        let state = WarrenForumAttachPromptState(link: ForumAttachLink(sid: sid, host: host, topicId: 42))
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

    func testBeginClearsTheLastFailureAndAStaleLinkFailsWithoutAnAttempt() {
        let state = WarrenForumAttachPromptState(link: ForumAttachLink(sid: sid, host: host, topicId: 0))
        state.fail(message: "expired")
        XCTAssertEqual(state.failure, "expired")
        XCTAssertFalse(state.busy, "a stale link never starts an upload")
        state.begin()
        XCTAssertNil(state.failure)
    }

    func testThePreviewCollectsOnceAndReportsItsOwnFailure() {
        let state = WarrenForumAttachPromptState(link: ForumAttachLink(sid: sid, host: host, topicId: 42))
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
