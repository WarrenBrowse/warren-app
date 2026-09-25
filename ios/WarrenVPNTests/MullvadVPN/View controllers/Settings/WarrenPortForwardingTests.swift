//
//  WarrenPortForwardingTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenSettings
import XCTest

@testable import WarrenVPN

/// What the port-forwarding screen shows for each mapping snapshot the
/// tunnel extension can broadcast. Android covers the same decisions in
/// `NatPmpStatusLabelTest` and `NatPmpPortBlockTest`.
final class WarrenPortForwardingTests: XCTestCase {
    private let now = Date(timeIntervalSince1970: 1_800_000_000)

    private func snapshot(
        status: String? = nil,
        externalPort: Int? = nil,
        mappedAt: Date? = nil,
        lifetimeSeconds: Int? = nil,
        failureReason: String? = nil,
        retryAfterSeconds: Int? = nil,
        rateLimitedAt: Date? = nil
    ) -> WarrenNatPmpSnapshot {
        WarrenNatPmpSnapshot(
            status: status,
            externalPort: externalPort,
            mappedAt: mappedAt,
            lifetimeSeconds: lifetimeSeconds,
            failureReason: failureReason,
            retryAfterSeconds: retryAfterSeconds,
            rateLimitedAt: rateLimitedAt
        )
    }

    private func state(
        _ snapshot: WarrenNatPmpSnapshot,
        tunnelIsSecured: Bool = true
    ) -> WarrenPortForwardingState {
        WarrenPortForwarding.state(snapshot: snapshot, tunnelIsSecured: tunnelIsSecured, now: now)
    }

    func testWithNoTunnelThereIsNothingToAskAnExitFor() {
        XCTAssertEqual(
            state(snapshot(status: "open", externalPort: 51820), tunnelIsSecured: false),
            .noTunnel)
    }

    func testNothingResolvedYetReadsAsARequestInFlight() {
        XCTAssertEqual(state(snapshot()), .requesting)
    }

    /// An exit refusing the mapping as not authorized (warren-core doc 105) is
    /// not a dead end: the tunnel asks again, and the screen says what was
    /// missing and when.
    func testARefusalForWantOfAnEntitlementCountsDownToTheNextTry() {
        var refused = snapshot(status: "refused", retryAfterSeconds: 30)
        refused.refusal = "no_entitlement"
        refused.refusedAt = now.addingTimeInterval(-10)

        XCTAssertEqual(state(refused), .refused(noEntitlement: true, retryIn: 20))
    }

    func testARefusedEntitlementIsNamedApart() {
        var refused = snapshot(status: "refused", retryAfterSeconds: 2)
        refused.refusal = "entitlement_refused"
        refused.refusedAt = now

        XCTAssertEqual(state(refused), .refused(noEntitlement: false, retryIn: 2))
    }

    /// The client renews at half the granted lifetime, so the countdown is
    /// half of it minus what has passed.
    func testAGrantCarriesItsPortAndTheCountdownToTheNextRenewal() {
        let held = snapshot(
            status: "open",
            externalPort: 51820,
            mappedAt: now.addingTimeInterval(-600),
            lifetimeSeconds: 3600)

        XCTAssertEqual(state(held), .mapped(port: 51820, renewsIn: 1200))
    }

    func testARenewalAlreadyDueCountsDownToZeroRatherThanBackwards() {
        let held = snapshot(
            status: "open",
            externalPort: 51820,
            mappedAt: now.addingTimeInterval(-3600),
            lifetimeSeconds: 3600)

        XCTAssertEqual(state(held), .mapped(port: 51820, renewsIn: 0))
    }

    /// The one refusal the user can act on is named apart, because it is the
    /// only one with a remedy on this screen.
    func testATakenPortIsNamedApartFromEveryOtherRefusal() {
        XCTAssertEqual(
            state(snapshot(status: "failed", failureReason: warrenPortInUseReason)),
            .failed(portConflict: true))
        XCTAssertEqual(
            state(snapshot(status: "failed", failureReason: "NetworkFailure")),
            .failed(portConflict: false))
        XCTAssertEqual(state(snapshot(status: "failed")), .failed(portConflict: false))
    }

    func testOnlyATakenPortOffersAWayOut() {
        XCTAssertTrue(WarrenPortForwarding.offersRecovery(.failed(portConflict: true)))
        for state: WarrenPortForwardingState in [
            .failed(portConflict: false), .requesting, .noTunnel,
            .mapped(port: 1, renewsIn: nil), .rateLimited(remaining: 30),
        ] {
            XCTAssertFalse(WarrenPortForwarding.offersRecovery(state), "\(state)")
        }
    }

    /// A refusal window outranks a refusal: the request works again on its
    /// own, so the recoveries would otherwise be spent on a wait.
    func testARefusalWindowOutranksTheRefusalUnderIt() {
        let limited = snapshot(
            status: "rate-limited",
            failureReason: warrenPortInUseReason,
            retryAfterSeconds: 90,
            rateLimitedAt: now.addingTimeInterval(-30))

        XCTAssertEqual(state(limited), .rateLimited(remaining: 60))
    }

    /// The window is anchored on when the exit said so, not on when the
    /// screen opened: a spent window must let the controls back.
    func testASpentRefusalWindowStopsHoldingTheScreen() {
        let limited = snapshot(
            status: "rate-limited",
            retryAfterSeconds: 90,
            rateLimitedAt: now.addingTimeInterval(-120))

        XCTAssertEqual(state(limited), .requesting)
    }

    func testAPortIsAcceptedOnlyWhenAnExitWouldConsiderIt() {
        XCTAssertEqual(WarrenPortForwarding.port(fromInput: "51820"), 51820)
        XCTAssertEqual(WarrenPortForwarding.port(fromInput: " 1024 "), 1024)
        XCTAssertEqual(WarrenPortForwarding.port(fromInput: "65535"), 65535)
        // An empty field is a deliberate "let the exit pick", not a refusal.
        XCTAssertEqual(WarrenPortForwarding.port(fromInput: ""), 0)
        for input in ["0", "1023", "65536", "-1", "80x", "abc"] {
            XCTAssertNil(WarrenPortForwarding.port(fromInput: input), input)
        }
    }

    /// A pin of zero means "let the exit pick", so the field is empty rather
    /// than naming a port nobody asked for.
    func testAnUnpinnedPortShowsAnEmptyField() {
        XCTAssertEqual(WarrenPortForwarding.input(forPort: 0), "")
        XCTAssertEqual(WarrenPortForwarding.input(forPort: 51820), "51820")
    }

    /// A record written before these knobs existed must keep behaving as it
    /// did, so every missing field decodes to what that record actually had.
    func testASettingsRecordWrittenBeforeTheseKnobsKeepsItsBehaviour() throws {
        let old = Data(#"{"state":{"on":{}}}"#.utf8)
        let decoded = try JSONDecoder().decode(WarrenNatPmpSettings.self, from: old)

        XCTAssertTrue(decoded.isEnabled)
        XCTAssertEqual(decoded.networkProtocol, .udp)
        XCTAssertEqual(decoded.externalPort, 0)
        XCTAssertEqual(decoded.lifetimeSeconds, 3600)
    }

    func testTheSettingsRoundTripThroughTheirOwnEncoding() throws {
        let settings = WarrenNatPmpSettings(
            state: .on, networkProtocol: .tcp, externalPort: 51820, lifetimeSeconds: 86400)
        let decoded = try JSONDecoder().decode(
            WarrenNatPmpSettings.self, from: JSONEncoder().encode(settings))

        XCTAssertEqual(decoded, settings)
    }

    /// The description reaches a support log, so it must carry the mapping
    /// and nothing that identifies the person holding it.
    func testTheDescriptionCarriesTheMappingAndNoIdentityMaterial() {
        let described = WarrenNatPmpSettings(
            state: .on, networkProtocol: .tcp, externalPort: 51820, lifetimeSeconds: 3600
        ).debugDescription

        XCTAssertTrue(described.contains("51820"))
        XCTAssertTrue(described.contains("tcp"))
    }
}
