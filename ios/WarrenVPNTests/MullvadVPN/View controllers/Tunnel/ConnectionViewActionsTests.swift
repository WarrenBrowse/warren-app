//
//  ConnectionViewActionsTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenMockData
import WarrenREST
import WarrenSettings
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The connect screen offers one location control with a shuffle on its side,
/// in every tunnel state, as `SwitchLocationButton.kt` does.
///
/// It used to offer three things: a split button whose side half was a
/// reconnect the campaign removed from the other clients, a detached shuffle
/// square sharing the row with it, and a plain label in the disconnected
/// states. Sharing the row with that square is what left "Changer de
/// localisation" too little width and wrapped it onto two lines.
final class ConnectionViewActionsTests: XCTestCase {
    /// A reconnect action the UI can no longer raise would be dead code the
    /// next reader has to prove unreachable.
    func testThereIsNoReconnectActionLeftToRaise() {
        let actions: [ConnectionViewViewModel.TunnelAction] = [
            .connect, .disconnect, .cancel, .selectLocation, .shuffleLocation,
        ]
        // Exhaustive: adding a case makes this switch fail to compile, which is
        // the point of listing them.
        for action in actions {
            switch action {
            case .connect, .disconnect, .cancel, .selectLocation, .shuffleLocation:
                continue
            }
        }
        XCTAssertEqual(actions.count, 5)
    }

    func testTheShuffleIsOfferedOnlyWhenThereIsAnActiveExitToPick() throws {
        let withRelays = ConnectionViewViewModel(
            tunnelStatus: TunnelStatus(),
            relayConstraints: RelayConstraints(),
            relayCache: MockRelayCache(),
            customListRepository: CustomListRepository()
        )
        XCTAssertTrue(withRelays.shuffleEnabled)

        let withoutRelays = ConnectionViewViewModel(
            tunnelStatus: TunnelStatus(),
            relayConstraints: RelayConstraints(),
            relayCache: MockRelayCache(isEmpty: true),
            customListRepository: CustomListRepository()
        )
        XCTAssertFalse(
            withoutRelays.shuffleEnabled,
            "the shuffle is offered with nothing to shuffle to")
    }
}
