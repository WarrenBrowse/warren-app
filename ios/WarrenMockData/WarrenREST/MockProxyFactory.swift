//
//  MockProxyFactory.swift
//  WarrenMockData
//
//  Created by Mojgan on 2024-05-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenREST
import WarrenRustRuntime
import WarrenTypes

public struct MockProxyFactory: ProxyFactoryProtocol {
    public var apiTransportProvider: APITransportProviderProtocol

    public func createAPIProxy() -> any APIQuerying {
        APIProxyStub()
    }

    public func createAccountsProxy() -> any RESTAccountHandling {
        AccountsProxyStub(createAccountResult: .success(.mockValue()))
    }

    public func createDevicesProxy() -> any DeviceHandling {
        DevicesProxyStub(deviceResult: .success(Device.mock(publicKey: WireGuard.PrivateKey().publicKey)))
    }

    public static func makeProxyFactory(
        apiTransportProvider: any APITransportProviderProtocol
    ) -> any ProxyFactoryProtocol {
        MockProxyFactory(
            apiTransportProvider: apiTransportProvider
        )
    }
}
