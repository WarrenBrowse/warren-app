//
//  WarrenAnnouncementsTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime

/// The FFI table the Rust verifier renders, decoded through the same path the
/// app uses. Whether an announcement may be shown at all is settled in Rust
/// against the pinned server key; what is exercised here is that the table
/// crosses the boundary intact, and that one unreadable part of it never
/// erases the rest.
final class WarrenAnnouncementsTests: XCTestCase {
    func testTheTableCrossesTheBoundaryVerbatim() throws {
        let table = """
            {"announcements":[{"id":"a1","headline":"Production is open",\
            "body":"Your beta account gets a free month.","level":"warning",\
            "cta":{"label":"Get Warren","url":"https://warren.ro/download"},\
            "voucher_campaign_id":"prod-launch"}],"active_until":1800003600}
            """

        let verified = try XCTUnwrap(WarrenAnnouncementsVerifier.decode(table))

        XCTAssertEqual(verified.activeUntil, Date(timeIntervalSince1970: 1_800_003_600))
        XCTAssertEqual(verified.announcements.count, 1)
        let announcement = try XCTUnwrap(verified.announcements.first)
        XCTAssertEqual(announcement.id, "a1")
        XCTAssertEqual(announcement.headline, "Production is open")
        XCTAssertEqual(announcement.body, "Your beta account gets a free month.")
        XCTAssertEqual(announcement.level, .warning)
        XCTAssertEqual(announcement.cta?.label, "Get Warren")
        XCTAssertEqual(announcement.cta?.url.absoluteString, "https://warren.ro/download")
        XCTAssertEqual(announcement.voucherCampaignID, "prod-launch")
        XCTAssertNil(
            announcement.voucherCode,
            "the per-account code rides the second, wallet-signed call, never this document"
        )
    }

    func testAnAnnouncementWithNoOfferCarriesNoCampaign() throws {
        let table = """
            {"announcements":[{"id":"a1","headline":"Scheduled maintenance","body":"",\
            "level":"info","cta":null,"voucher_campaign_id":null}],"active_until":1800003600}
            """

        let verified = try XCTUnwrap(WarrenAnnouncementsVerifier.decode(table))

        XCTAssertNil(verified.announcements.first?.voucherCampaignID)
        XCTAssertNil(verified.announcements.first?.cta)
        XCTAssertEqual(verified.announcements.first?.level, .info)
    }

    func testAnUnreadableRowIsDroppedAndTheRestOfTheSetStillShows() throws {
        let table = """
            {"announcements":[{"id":"","headline":"no id"},{"headline":"no id at all"},\
            {"id":"a2","headline":"kept","body":"b","level":"error","cta":null,\
            "voucher_campaign_id":null}],"active_until":1800003600}
            """

        let verified = try XCTUnwrap(WarrenAnnouncementsVerifier.decode(table))

        XCTAssertEqual(
            verified.announcements.map(\.id),
            ["a2"],
            "one unreadable row must not erase a live announcement"
        )
        XCTAssertEqual(verified.announcements.first?.level, .error)
    }

    func testAnUnknownLevelReadsAsInformationalRatherThanWithholdingTheText() throws {
        let table = """
            {"announcements":[{"id":"a1","headline":"h","body":"b","level":"catastrophe",\
            "cta":null,"voucher_campaign_id":null}],"active_until":1800003600}
            """

        let verified = try XCTUnwrap(WarrenAnnouncementsVerifier.decode(table))

        XCTAssertEqual(verified.announcements.first?.level, .info)
    }

    func testACallToActionWithNoUsableUrlIsDroppedWithoutWithholdingTheAnnouncement() throws {
        let table = """
            {"announcements":[{"id":"a1","headline":"h","body":"b","level":"info",\
            "cta":{"label":"Claim it"},"voucher_campaign_id":null}],\
            "active_until":1800003600}
            """

        let verified = try XCTUnwrap(WarrenAnnouncementsVerifier.decode(table))

        XCTAssertEqual(verified.announcements.count, 1, "the operator's text is not the unsafe part")
        XCTAssertNil(verified.announcements.first?.cta)
    }

    func testAMalformedTableIsRefusedWhole() {
        XCTAssertNil(WarrenAnnouncementsVerifier.decode("not json at all"))
        XCTAssertNil(WarrenAnnouncementsVerifier.decode("{}"))
        XCTAssertNil(
            WarrenAnnouncementsVerifier.decode(#"{"announcements":[]}"#),
            "a table with no expiry has no anti-freeze bound and must not be believed"
        )
    }

    func testTheLiveVerifierRefusesAnEnvelopeThatIsNotSignedByThePinnedKey() {
        // The real FFI, with a body nothing signed. Anything able to answer for
        // the API host could otherwise put a link on every home screen.
        XCTAssertNil(
            WarrenAnnouncementsVerifier.verify(
                envelope: Data(#"{"version":1,"announcements":[]}"#.utf8),
                currentVersion: "2026.3"
            )
        )
        XCTAssertNil(
            WarrenAnnouncementsVerifier.verify(envelope: Data(), currentVersion: "2026.3")
        )
    }
}

/// The wallet-signed campaign lookup's envelope. A transient outage must never
/// tell a cohort member they were never eligible, so a failure and an empty
/// cohort are distinct outcomes rather than one null.
final class WarrenCampaignVoucherOutcomeTests: XCTestCase {
    func testACohortMemberGetsTheCodeDrawnForThisAccount() throws {
        let outcome = WarrenAccountClient.campaignVoucherOutcome(
            fromEnvelope: #"{"ok":true,"code":"ABCD1234EFGH5678"}"#
        )

        XCTAssertEqual(try outcome.get(), "ABCD1234EFGH5678")
    }

    func testAnAccountOutsideTheCohortIsAnAnswerRatherThanAFailure() throws {
        XCTAssertNil(
            try WarrenAccountClient.campaignVoucherOutcome(
                fromEnvelope: #"{"ok":true,"code":null}"#
            ).get()
        )
        XCTAssertNil(
            try WarrenAccountClient.campaignVoucherOutcome(fromEnvelope: #"{"ok":true}"#).get(),
            "a missing code reads exactly like an explicit null"
        )
    }

    func testAFailedLookupIsNeverReadAsAnEmptyCohort() {
        guard case let .failure(server) = WarrenAccountClient.campaignVoucherOutcome(
            fromEnvelope: #"{"ok":false,"error":"server returned status 503","status":503}"#
        ) else {
            return XCTFail("a server refusal must surface as a failure")
        }
        XCTAssertEqual(server, .server(status: 503, message: "server returned status 503"))

        guard case .failure = WarrenAccountClient.campaignVoucherOutcome(
            fromEnvelope: #"{"ok":false,"error":"transport failed"}"#
        ) else {
            return XCTFail("a transport failure must surface as a failure")
        }

        guard case .failure = WarrenAccountClient.campaignVoucherOutcome(fromEnvelope: nil) else {
            return XCTFail("a null envelope must surface as a failure")
        }
        guard case .failure = WarrenAccountClient.campaignVoucherOutcome(
            fromEnvelope: "not json at all"
        ) else {
            return XCTFail("an unreadable envelope must surface as a failure")
        }
    }
}
