//
//  DestinationDescriberTests.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2025-01-21.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenMockData
import Network
import XCTest
@testable import WarrenVPN

@testable import WarrenREST
@testable import WarrenSettings

final class DestinationDescriberTests: XCTestCase {
    static let store = InMemorySettingsStore<SettingNotFound>()
    override static func setUp() {
        SettingsManager.unitTestStore = store
    }

    override static func tearDown() {
        store.reset()
    }

    // Other suites point `SettingsManager.unitTestStore` at their own store (a
    // Swift Testing suite does it from its initializer), and nothing here deletes
    // a saved list, so a fixed name meets its own earlier save when a test runs
    // again in the same process and `save` throws `duplicateName`. A name unique
    // to each run cannot collide, whichever store is current. It stays short
    // because `save` refuses a name longer than `NameInputFormatter.maxLength`.
    private func uniqueListName() -> String {
        "List-\(UUID().uuidString.prefix(8))"
    }

    func testDescribeList() throws {
        let relayCache = MockRelayCache()
        let customListRepository = CustomListRepository()
        let describer = DestinationDescriber(
            relayCache: relayCache,
            customListRepository: customListRepository
        )
        let listid = UUID()
        let listName = uniqueListName()
        try customListRepository.save(
            list: .init(
                id: listid,
                name: listName,
                locations: [.country("se"), .country("dk")]
            ))
        XCTAssertEqual(
            describer.describe(
                .init(
                    locations: [.country("se"), .country("dk")],
                    customListSelection: .init(listId: listid, isList: true)
                )),
            listName
        )
    }

    func testDescribeSubsetOfList() throws {
        let relayCache = MockRelayCache()
        let customListRepository = CustomListRepository()
        let describer = DestinationDescriber(
            relayCache: relayCache,
            customListRepository: customListRepository
        )
        let listid = UUID()
        try customListRepository.save(
            list: .init(
                id: listid,
                name: uniqueListName(),
                locations: [.country("se"), .country("dk")]
            ))
        XCTAssertEqual(
            describer.describe(
                .init(
                    locations: [.country("se")],
                    customListSelection: .init(listId: listid, isList: false)
                )),
            "Sweden"
        )
    }

    func testDescribeCountryDestination() {
        let relayCache = MockRelayCache()
        let customListRepository = CustomListRepository()
        let describer = DestinationDescriber(
            relayCache: relayCache,
            customListRepository: customListRepository
        )
        XCTAssertEqual(describer.describe(.init(locations: [.country("se")])), "Sweden")
    }

    func testDescribeCityDestination() {
        let relayCache = MockRelayCache()
        let customListRepository = CustomListRepository()
        let describer = DestinationDescriber(
            relayCache: relayCache,
            customListRepository: customListRepository
        )
        XCTAssertEqual(describer.describe(.init(locations: [.city("se", "sto")])), "Stockholm")
    }

    func testDescribeRelayDestination() {
        let relayCache = MockRelayCache()
        let customListRepository = CustomListRepository()
        let describer = DestinationDescriber(
            relayCache: relayCache,
            customListRepository: customListRepository
        )
        XCTAssertEqual(
            describer.describe(.init(locations: [.hostname("se", "sto", "se6-wireguard")])),
            "Stockholm (se6-wireguard)"
        )
    }
}
