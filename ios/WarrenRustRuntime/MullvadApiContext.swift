//
//  MullvadApiContext.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2025-01-24.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenTypes

func onAccessChangeCallback(selfPtr: UnsafeRawPointer?, bytes: UnsafePointer<UInt8>?) {
    guard let selfPtr, let bytes else { return }
    let context = Unmanaged<MullvadApiContext>.fromOpaque(selfPtr).takeUnretainedValue()

    let uuid = NSUUID(uuidBytes: bytes) as UUID
    context.accessMethodChangeListeners.forEach { $0.accessMethodChangedTo(uuid) }
}

public class MullvadApiContext: @unchecked Sendable {
    enum Error: Swift.Error {
        case failedToConstructApiClient
    }

    public private(set) var context: SwiftApiContext!
    public let accessMethodChangeListeners: [MullvadAccessMethodChangeListening]

    public init(
        host: String,
        address: String,
        disableTls: Bool = false,
        accessMethodWrapper: SwiftAccessMethodSettingsWrapper,
        accessMethodChangeListeners: [MullvadAccessMethodChangeListening]
    ) throws {
        self.accessMethodChangeListeners = accessMethodChangeListeners

        let selfPtr = Unmanaged.passUnretained(self).toOpaque()
        context =
            switch disableTls {
            case true:
                mullvad_api_init_new_tls_disabled(
                    host,
                    address,
                    accessMethodWrapper,
                    onAccessChangeCallback,
                    selfPtr
                )
            case false:
                mullvad_api_init_new(
                    host,
                    address,
                    accessMethodWrapper,
                    onAccessChangeCallback,
                    selfPtr
                )
            }

        if context._0 == nil {
            throw Error.failedToConstructApiClient
        }
    }
}

extension SwiftApiContext: @unchecked @retroactive Sendable {}
