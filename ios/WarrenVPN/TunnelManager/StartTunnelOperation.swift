//
//  StartTunnelOperation.swift
//  MullvadVPN
//
//  Created by pronebird on 15/12/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenREST
import WarrenSettings
import NetworkExtension
import Operations
import PacketTunnelCore

class StartTunnelOperation: ResultOperation<Void>, @unchecked Sendable {
    typealias EncodeErrorHandler = (Error) -> Void

    private let interactor: TunnelInteractor
    private let standDownRecord: () -> WarrenEnvStandDownRecord
    private let logger = Logger(label: "StartTunnelOperation")

    /// `standDownRecord` is read when the operation runs, not when it is
    /// queued: a stand-down that lands while the operation waits its turn must
    /// still refuse it.
    init(
        dispatchQueue: DispatchQueue,
        interactor: TunnelInteractor,
        standDownRecord: @escaping () -> WarrenEnvStandDownRecord = { WarrenEnvStandDownRecord() },
        completionHandler: @escaping CompletionHandler
    ) {
        self.interactor = interactor
        self.standDownRecord = standDownRecord

        super.init(
            dispatchQueue: dispatchQueue,
            completionQueue: dispatchQueue,
            completionHandler: completionHandler
        )
    }

    override func main() {
        // Coexistence, first of all: a build that has stood down for a
        // higher-priority product environment must not answer a connect by
        // quietly bringing the tunnel up. It would hold a tunnel under a
        // banner that says it stood down, and the configuration written on the
        // way up would re-arm the on-demand rule the stand-down cleared. The
        // banner's re-enable is the way back, as `ClearEnvYield` is on the
        // desktop, and it has to stay the only one: a refusal that a login or
        // a tunnel state could step around is not a rule.
        let standDown = standDownRecord()
        guard !standDown.isStandingDown else {
            finish(
                result: .failure(
                    WarrenEnvStandDownRefusal(environment: standDown.higherEnvironmentSeen ?? "")
                )
            )
            return
        }

        guard case .loggedIn = interactor.deviceState else {
            finish(result: .failure(InvalidDeviceStateError()))
            return
        }

        switch interactor.tunnelStatus.state {
        case .disconnecting(.nothing):
            interactor.updateTunnelStatus { tunnelStatus in
                tunnelStatus = TunnelStatus()
                tunnelStatus.state = .disconnecting(.reconnect)
            }

            finish(result: .success(()))

        case .disconnected, .pendingReconnect, .waitingForConnectivity:
            makeTunnelProviderAndStartTunnel { error in
                self.finish(result: error.map { .failure($0) } ?? .success(()))
            }

        default:
            finish(result: .success(()))
        }
    }

    private func makeTunnelProviderAndStartTunnel(completionHandler: @escaping @Sendable (Error?) -> Void) {
        makeTunnelProvider { result in
            self.dispatchQueue.async {
                do {
                    try self.startTunnel(tunnel: result.get())
                    completionHandler(nil)
                } catch {
                    completionHandler(error)
                }
            }
        }
    }

    private func startTunnel(tunnel: any TunnelProtocol) throws {
        let selectedRelays = try? interactor.selectRelays()
        var tunnelOptions = PacketTunnelOptions()

        do {
            if let selectedRelays {
                try tunnelOptions.setSelectedRelays(selectedRelays)
            }
        } catch {
            logger.error(
                error: error,
                message: "Failed to encode the selector result."
            )
        }

        interactor.setTunnel(tunnel, shouldRefreshTunnelState: false)

        interactor.updateTunnelStatus { tunnelStatus in
            tunnelStatus = TunnelStatus()
            tunnelStatus.state = .connecting(
                selectedRelays,
                isPostQuantum: interactor.settings.tunnelQuantumResistance.isEnabled,
                isDaita: interactor.settings.daita.isEnabled
            )
        }

        try tunnel.start(options: tunnelOptions.rawOptions())
    }

    private func makeTunnelProvider(
        completionHandler:
            @escaping @Sendable (Result<any TunnelProtocol, Error>)
            -> Void
    ) {
        let persistentTunnels = interactor.getPersistentTunnels()
        let tunnel = persistentTunnels.first ?? interactor.createNewTunnel()
        let configuration = TunnelConfiguration(
            includeAllNetworks: interactor.settings.includeAllNetworks.includeAllNetworksIsEnabled,
            excludeLocalNetworks: interactor.settings.includeAllNetworks.localNetworkSharingIsEnabled,
            standDown: standDownRecord()
        )

        tunnel.setConfiguration(configuration)
        tunnel.saveToPreferences { error in
            completionHandler(error.map { .failure($0) } ?? .success(tunnel))
        }
    }
}
