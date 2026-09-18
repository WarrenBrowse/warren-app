//
//  RelayListCacheTests.swift
//  WarrenRESTTests
//
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenMockData
import WarrenRustRuntime
import WarrenTypes
import Network
import XCTest

@testable import WarrenREST
@testable import WarrenSettings

/// Two properties of the relay list on its way into the cache.
///
/// This file used to serve the fetch half from `/app/v1/relays`, Mullvad's
/// route. Warren's iOS client asks for `v1/exits` instead
/// (`warren-ios/src/api_client/api.rs`, `warren_exits_response`), so the mock
/// matched nothing and answered 501 on every run. Nothing noticed, because no
/// iOS test ran in CI.
///
/// The fetch half now exercises the route the client really asks for, and the
/// verdict it really depends on: the body is a signed exit directory checked
/// against a baked server key, so an unsigned one has to be refused rather
/// than cached. Forging a valid signature from here is not possible by design,
/// which is why the storage half is tested on its own, with no network in it.
class RelayListCacheTests: XCTestCase {
    static let store = InMemorySettingsStore<SettingNotFound>()

    override func setUp() {
        super.setUp()
        SettingsManager.unitTestStore = Self.store
        RustLogging.initialize()
    }

    /// The cache stores the bytes the server sent, not a re-encoding of the
    /// fields this version of the app happens to model. On main before this
    /// property existed, `StoredRelays` encoded a decoded `relays` value
    /// through Codable and dropped everything it did not declare.
    func testTheCacheKeepsTheServersBytesIncludingFieldsItDoesNotModel() throws {
        let rawData = try ServerRelaysResponseStubs.sampleRelaysJSONWithUnknownField()
        let etag = "\"an-etag\""

        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let fileURL = tempDir.appendingPathComponent("relays.json")
        let relayCache = RelayCache(fileCache: FileCache<StoredRelays>(fileURL: fileURL))
        try relayCache.write(record: StoredRelays(etag: etag, rawData: rawData, updatedAt: Date()))

        // A fresh cache, so the read goes to disk rather than to memory.
        let readBack = try FileCache<StoredRelays>(fileURL: fileURL).read()

        XCTAssertEqual(readBack.rawData, rawData)
        XCTAssertEqual(readBack.etag, etag)

        let json = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: readBack.rawData) as? [String: Any]
        )
        XCTAssertNotNil(
            json["future_feature"],
            "an unmodelled field did not survive the store and read round trip"
        )
        // The known fields still decode, so keeping the bytes costs nothing.
        XCTAssertNotNil(try readBack.cachedRelays)
    }

    /// The exit directory is signed, and the signature is the only thing
    /// standing between a hostile answer on this route and the list of servers
    /// the app will connect to. An unsigned body must not come back as content
    /// to cache.
    func testAnExitDirectoryWithNoSignatureIsRefusedRatherThanCached() async throws {
        let unsigned = String(
            data: try ServerRelaysResponseStubs.sampleRelaysJSONWithUnknownField(),
            encoding: .utf8
        )!
        let mock = MullvadApiMock.get(
            path: "/v1/exits",
            responseCode: 200,
            responseData: unsigned
        )
        let apiProxy = try makeApiProxy(port: mock.port)

        let result: Result<REST.ServerRelaysCacheResponse, Error> =
            await withCheckedContinuation { continuation in
                _ = apiProxy.getRelays(etag: nil, retryStrategy: .noRetry) { result in
                    continuation.resume(returning: result)
                }
            }

        switch result {
        case let .success(response):
            if case .newContent = response {
                XCTFail("an unsigned exit directory was accepted as new content")
            }
        case .failure:
            // Refused is the outcome under test; which error carries the
            // refusal is the FFI's business.
            break
        }
    }

    // MARK: - Helpers

    private func makeApiProxy(port: UInt16) throws -> APIQuerying {
        let shadowsocksLoader = ShadowsocksLoaderStub(
            configuration: ShadowsocksConfiguration(
                address: .ipv4(.loopback),
                port: 1080,
                password: "123",
                cipher: "aes-128-cfb"
            )
        )

        let accessMethodsRepository = AccessMethodRepositoryStub.stub

        let context = try MullvadApiContext(
            host: "localhost",
            address: "\(IPv4Address.loopback.debugDescription):\(port)",
            encryptedDnsDomain: REST.encryptedDNSHostname,
            domainFrontingFront: "",
            domainFrontingProxyHost: "",
            disableTls: true,
            shadowsocksProvider: shadowsocksLoader,
            accessMethodWrapper: initAccessMethodSettingsWrapper(methods: accessMethodsRepository.fetchAll()),
            accessMethodChangeListeners: []
        )

        return REST.MullvadAPIProxy(
            transportProvider: APITransportProvider(
                requestFactory: .init(
                    apiContext: context,
                    encoder: JSONEncoder()
                )
            ),
            dispatchQueue: .main,
            responseDecoder: REST.Coding.makeJSONDecoder()
        )
    }
}
