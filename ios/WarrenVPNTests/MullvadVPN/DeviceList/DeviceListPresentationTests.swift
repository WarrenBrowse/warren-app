//
//  DeviceListPresentationTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The rules the account's device list reads by on every client
/// (warren-core doc 115 section 7.1).
final class DeviceListPresentationTests: XCTestCase {
    private func device(
        _ id: String,
        name: String = "device",
        platform: String = "linux",
        lastSeenOn: String? = "2026-10-20",
        thisDevice: Bool = false
    ) -> WarrenAccountDevice {
        WarrenAccountDevice(
            id: id, name: name, platform: platform, appVersion: "1.1.40",
            lastSeenOn: lastSeenOn.flatMap(WarrenRegistryDay.init), thisDevice: thisDevice
        )
    }

    func testThisDeviceComesFirstThenTheMostRecentlyActiveThenByName() {
        let devices = [
            device("old", lastSeenOn: "2026-09-01"),
            device("unknown-day", lastSeenOn: nil),
            device("b", name: "beta", lastSeenOn: "2026-10-27"),
            device("me", lastSeenOn: "2026-08-01", thisDevice: true),
            device("a", name: "alpha", lastSeenOn: "2026-10-27"),
            device("recent", lastSeenOn: "2026-10-28"),
        ]

        let order = DeviceListPresentation.inDisplayOrder(devices).map(\.id)

        XCTAssertEqual(order, ["me", "recent", "a", "b", "old", "unknown-day"])
    }

    /// Another device chose the name: its direction controls must not
    /// reorder the text around it.
    func testANameIsShownWithoutItsBidirectionalControls() {
        let controls = ["\u{202A}", "\u{202B}", "\u{202C}", "\u{202D}", "\u{202E}", "\u{2066}", "\u{2067}", "\u{2068}", "\u{2069}"]
        let name = "brave" + controls.joined() + " otter\u{200F}"

        XCTAssertEqual(DeviceListPresentation.displayedName(name), "brave otter\u{200F}")
    }

    func testANameIsKeptOnlyWithOneToFortyCharactersAndNoControlCharacter() {
        XCTAssertTrue(DeviceListPresentation.isValidName(" brave otter "))
        XCTAssertTrue(DeviceListPresentation.isValidName(String(repeating: "é", count: 40)))
        XCTAssertFalse(DeviceListPresentation.isValidName(String(repeating: "e", count: 41)))
        XCTAssertFalse(DeviceListPresentation.isValidName("   "))
        XCTAssertFalse(DeviceListPresentation.isValidName("tab\there"))
    }

    func testProductNamesStayAndClientKindsAreWords() {
        XCTAssertEqual(DeviceListPresentation.platformLabel("ios"), "iOS")
        XCTAssertEqual(DeviceListPresentation.platformLabel("macos"), "macOS")
        XCTAssertEqual(DeviceListPresentation.platformLabel("cli"), "Command line")
        XCTAssertEqual(DeviceListPresentation.platformLabel("browser"), "Browser extension")
        XCTAssertEqual(DeviceListPresentation.platformLabel("toaster"), "Other platform")
        XCTAssertEqual(
            DeviceListPresentation.platformLine(device("x", platform: "windows")),
            "Windows · 1.1.40"
        )
    }

    /// The registry counts days in UTC: a device seen on today's UTC day is
    /// active today whatever the device's own time zone says.
    func testADeviceSeenOnTheUtcDayIsActiveTodayAndAnOlderOneShowsItsDay() {
        let locale = Locale(identifier: "en_US")
        let today = WarrenRegistryDay("2026-10-28")!

        XCTAssertEqual(
            DeviceListPresentation.activityLabel(lastSeenOn: today, today: today, locale: locale),
            "Active today"
        )
        XCTAssertEqual(
            DeviceListPresentation.activityLabel(
                lastSeenOn: WarrenRegistryDay("2026-10-26")!, today: today, locale: locale),
            "Last active 10/26/26"
        )
        XCTAssertEqual(
            DeviceListPresentation.olderClientSeenLabel(WarrenRegistryDay("2026-10-25")!, locale: locale),
            "Last seen 10/25/26"
        )
    }
}
