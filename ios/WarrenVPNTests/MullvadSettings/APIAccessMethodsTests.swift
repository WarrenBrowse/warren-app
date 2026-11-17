//
//  APIAccessMethodsTests.swift
//  MullvadVPNTests
//
//  Created by Jon Petersson on 2023-12-13.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import XCTest
@testable import WarrenVPN

@testable import WarrenSettings
@testable import WarrenTypes

final class APIAccessMethodsTests: XCTestCase {
    static let store = InMemorySettingsStore<SettingNotFound>()

    override static func setUp() {
        SettingsManager.unitTestStore = store
    }

    override static func tearDown() {
        store.reset()
    }

    override func tearDownWithError() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])
        repository.fetchAll().forEach {
            repository.delete(id: $0.id)
        }
    }

    func testDefaultAccessMethodsExist() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])
        let storedMethods = repository.fetchAll()

        XCTAssertEqual(storedMethods.map(\.kind), [.direct])
    }

    func testStoreWrittenWithRemovedMethodsKeepsDirectAndCustomMethods() throws {
        let custom = socks5AccessMethod(with: UUID())
        let customJSON = String(decoding: try JSONEncoder().encode(custom), as: UTF8.self)
        let bridges = """
            {"id":"8586E75A-CA7B-4432-B70D-EE65F3F95084","name":"Warren bridges","isEnabled":true,\
            "proxyConfiguration":{"bridges":{}}}
            """
        let legacyStore = """
            {"lastReachableAccessMethod":\(bridges),"accessMethods":[\
            {"id":"C9DB7457-2A55-42C3-A926-C07F82131994","name":"Direct","isEnabled":true,\
            "proxyConfiguration":{"direct":{}}},\
            \(bridges),\
            {"id":"831CB1F8-1829-42DD-B9DC-82902F298EC0","name":"Encrypted DNS proxy","isEnabled":true,\
            "proxyConfiguration":{"encryptedDNS":{}}},\
            {"id":"E4BC6B6B-18E7-4E6F-9E53-95E3426B1796","name":"Domain fronting","isEnabled":true,\
            "proxyConfiguration":{"domainFronting":{}}},\
            \(customJSON)]}
            """
        try Self.store.write(Data(legacyStore.utf8), for: .apiAccessMethods)

        let repository = AccessMethodRepository(shadowsocksCiphers: [])

        XCTAssertEqual(repository.fetchAll().map(\.kind), [.direct, .socks5])
        XCTAssertEqual(repository.fetch(by: custom.id)?.name, custom.name)
        XCTAssertEqual(repository.fetchLastReachable().id, AccessMethodRepository.directId)
    }

    func testAddingSocks5AccessMethod() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])

        let uuid = UUID()
        let methodToStore = socks5AccessMethod(with: uuid)
        repository.save(methodToStore)

        let storedMethod = repository.fetch(by: uuid)

        XCTAssertEqual(methodToStore.id, storedMethod?.id)
    }

    func testAddingShadowSocksAccessMethod() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])

        let uuid = UUID()
        let methodToStore = shadowsocksAccessMethod(with: uuid)
        repository.save(methodToStore)

        let storedMethod = repository.fetch(by: uuid)

        XCTAssertEqual(methodToStore.id, storedMethod?.id)
    }

    func testAddingDuplicateAccessMethodDoesNothing() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])

        let methodToStore = socks5AccessMethod(with: UUID())

        repository.save(methodToStore)
        repository.save(methodToStore)

        let storedMethods = repository.fetchAll()

        // Account for .direct that is always added by default.
        XCTAssertEqual(storedMethods.count, 2)
    }

    func testUpdatingAccessMethod() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])

        let uuid = UUID()
        var methodToStore = socks5AccessMethod(with: uuid)
        repository.save(methodToStore)

        let newName = "Renamed method"
        methodToStore.name = newName

        repository.save(methodToStore)

        let storedMethod = repository.fetch(by: uuid)

        XCTAssertEqual(storedMethod?.name, newName)
    }

    func testDeletingAccessMethod() throws {
        let repository = AccessMethodRepository(shadowsocksCiphers: [])
        let uuid = UUID()
        let methodToStore = socks5AccessMethod(with: uuid)

        repository.save(methodToStore)
        repository.delete(id: uuid)

        let storedMethod = repository.fetch(by: uuid)

        XCTAssertNil(storedMethod)
    }
}

extension APIAccessMethodsTests {
    private func socks5AccessMethod(with uuid: UUID) -> PersistentAccessMethod {
        PersistentAccessMethod(
            id: uuid,
            name: "Method",
            isEnabled: true,
            proxyConfiguration: .socks5(
                PersistentProxyConfiguration.SocksConfiguration(
                    server: .ipv4(.any),
                    port: 1,
                    authentication: .noAuthentication
                ))
        )
    }

    private func shadowsocksAccessMethod(with uuid: UUID) -> PersistentAccessMethod {
        PersistentAccessMethod(
            id: uuid,
            name: "Method",
            isEnabled: true,
            proxyConfiguration: .shadowsocks(
                PersistentProxyConfiguration.ShadowsocksConfiguration(
                    server: .ipv4(.any),
                    port: 1,
                    password: "Password",
                    cipher: "aes-128-cfb"
                ))
        )
    }
}
