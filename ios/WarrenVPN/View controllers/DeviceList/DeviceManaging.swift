//
//  DeviceManaging.swift
//  WarrenVPN
//
//  Created by pronebird on 26/07/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenRustRuntime

/// The account's devices in the registry (warren-core doc 115), signed with
/// the wallet on this device.
protocol DeviceManaging: Sendable {
    func listDevices() async -> Result<WarrenAccountDeviceList, WarrenDeviceRegistryError>
    /// Answers whether the renamed device is this one.
    func renameDevice(_ id: String, name: String) async -> Result<Bool, WarrenDeviceRegistryError>
    /// Answers whether the revoked device is this one.
    func revokeDevice(_ id: String) async -> Result<Bool, WarrenDeviceRegistryError>
}

/// The production registry: loads the mnemonic, derives the seed, hands it to
/// the FFI and wipes it. The seed exists for exactly one call.
struct DeviceManagementInteractor: DeviceManaging {
    func listDevices() async -> Result<WarrenAccountDeviceList, WarrenDeviceRegistryError> {
        await withSeed { WarrenDeviceRegistryClient.list(seed: $0) }
    }

    func renameDevice(_ id: String, name: String) async -> Result<Bool, WarrenDeviceRegistryError> {
        await withSeed { WarrenDeviceRegistryClient.rename(seed: $0, id: id, name: name) }
    }

    func revokeDevice(_ id: String) async -> Result<Bool, WarrenDeviceRegistryError> {
        await withSeed { WarrenDeviceRegistryClient.revoke(seed: $0, id: id) }
    }

    private func withSeed<T: Sendable>(
        _ body: @escaping @Sendable (Data) -> Result<T, WarrenDeviceRegistryError>
    ) async -> Result<T, WarrenDeviceRegistryError> {
        await Task.detached(priority: .userInitiated) {
            guard let mnemonic = try? WarrenWalletKeychain.loadSecure(),
                let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
            else {
                return .failure(.noWallet)
            }
            // The wallet zeroes its own seed on `forgetSecret`, so the 32
            // bytes live only as long as the call.
            defer { wallet.forgetSecret() }
            return body(wallet.seed)
        }.value
    }
}

/// What went wrong with an action on the list, each with its own message.
enum DeviceManagementFailure: Equatable {
    case disconnect
    case rename
    case logOut

    var message: String {
        switch self {
        case .disconnect:
            NSLocalizedString("Could not disconnect the device. Try again.", comment: "")
        case .rename:
            NSLocalizedString("Could not rename the device. Try again.", comment: "")
        case .logOut:
            NSLocalizedString("Something went wrong while logging out.", comment: "")
        }
    }
}

/// The device list's state: loading, the account's devices in display order
/// or a failed load, the rows a request is running for, and the last failed
/// action.
@MainActor
final class DeviceManagementViewModel: ObservableObject {
    enum LoadState: Equatable {
        case loading
        case loaded
        case failed
    }

    @Published private(set) var loadState: LoadState = .loading
    @Published private(set) var devices: [WarrenAccountDevice] = []
    @Published private(set) var olderClientSeenOn: WarrenRegistryDay?
    @Published private(set) var busy: Set<String> = []
    @Published var failure: DeviceManagementFailure?

    /// This device was revoked from another device of the account: the host
    /// ends the session and shows the revoked screen.
    var onRevoked: (() -> Void)?

    private let deviceManaging: any DeviceManaging
    private let today: () -> WarrenRegistryDay

    init(
        deviceManaging: any DeviceManaging,
        today: @escaping () -> WarrenRegistryDay = { WarrenRegistryDay(utc: Date()) }
    ) {
        self.deviceManaging = deviceManaging
        self.today = today
    }

    var todayInRegistry: WarrenRegistryDay {
        today()
    }

    func fetchDevices() async {
        loadState = .loading
        await reload()
    }

    /// Disconnects another device of the account, after its confirmation.
    func disconnect(_ device: WarrenAccountDevice) async {
        await act(device.id, failure: .disconnect) { [deviceManaging] in
            await deviceManaging.revokeDevice(device.id).map { _ in () }
        }
    }

    /// Renames a device of the account; a name the registry would refuse is
    /// never sent.
    func rename(_ device: WarrenAccountDevice, to name: String) async {
        guard DeviceListPresentation.isValidName(name) else { return }
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        await act(device.id, failure: .rename) { [deviceManaging] in
            await deviceManaging.renameDevice(device.id, name: trimmed).map { _ in () }
        }
    }

    /// "Log out" on this device's row, after the logout confirmation: the
    /// registry revokes it, then the wallet leaves the device as on any
    /// logout. Answers whether the logout went through; a revocation that
    /// failed leaves the wallet in place.
    func logOutThisDevice(_ device: WarrenAccountDevice, logOut: () async -> Void) async -> Bool {
        busy.insert(device.id)
        let result = await deviceManaging.revokeDevice(device.id)
        switch result {
        case .success:
            await logOut()
            busy.remove(device.id)
            return true
        case .failure(.revoked):
            busy.remove(device.id)
            onRevoked?()
            return false
        case .failure:
            busy.remove(device.id)
            failure = .logOut
            await reload()
            return false
        }
    }

    // `notFound` means the device is already gone: the list is read again
    // either way.
    private func act(
        _ id: String,
        failure: DeviceManagementFailure,
        request: () async -> Result<Void, WarrenDeviceRegistryError>
    ) async {
        busy.insert(id)
        let result = await request()
        busy.remove(id)
        switch result {
        case .success, .failure(.notFound):
            break
        case .failure(.revoked):
            onRevoked?()
        case .failure:
            self.failure = failure
        }
        await reload()
    }

    private func reload() async {
        switch await deviceManaging.listDevices() {
        case let .success(list):
            devices = DeviceListPresentation.inDisplayOrder(list.devices)
            olderClientSeenOn = list.olderClientSeenOn
            loadState = .loaded
        case let .failure(error):
            // The registry says this device was revoked: act on it now
            // rather than at the extension's next refresh.
            if error == .revoked {
                onRevoked?()
            }
            loadState = .failed
        }
    }
}

/// The registry, answered from memory: previews and the screenshot builds.
struct MockDeviceManaging: DeviceManaging {
    var list: Result<WarrenAccountDeviceList, WarrenDeviceRegistryError> = .success(Self.sample)

    func listDevices() async -> Result<WarrenAccountDeviceList, WarrenDeviceRegistryError> {
        list
    }

    func renameDevice(_ id: String, name: String) async -> Result<Bool, WarrenDeviceRegistryError> {
        .success(false)
    }

    func revokeDevice(_ id: String) async -> Result<Bool, WarrenDeviceRegistryError> {
        .success(false)
    }

    static let sample = WarrenAccountDeviceList(
        devices: [
            WarrenAccountDevice(
                id: "AAAAAAAAAAAAAAAAAAAAAA", name: "brave otter", platform: "ios", appVersion: "1.1.40",
                lastSeenOn: WarrenRegistryDay(utc: Date()), thisDevice: true
            ),
            WarrenAccountDevice(
                id: "BBBBBBBBBBBBBBBBBBBBBB", name: "quiet heron", platform: "windows", appVersion: "1.1.40",
                lastSeenOn: WarrenRegistryDay(utc: Date().addingTimeInterval(-3 * 86_400)), thisDevice: false
            ),
            WarrenAccountDevice(
                id: "CCCCCCCCCCCCCCCCCCCCCC", name: "swift lynx", platform: "cli", appVersion: "1.1.40",
                lastSeenOn: WarrenRegistryDay(utc: Date().addingTimeInterval(-12 * 86_400)), thisDevice: false
            ),
        ],
        olderClientSeenOn: WarrenRegistryDay(utc: Date().addingTimeInterval(-86_400))
    )
}
