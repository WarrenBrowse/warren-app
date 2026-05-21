//
//  SettingsInteractorFactory.swift
//  MullvadVPN
//
//  Created by pronebird on 26/10/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenREST
import WarrenSettings

final class SettingsInteractorFactory {
    private let apiProxy: APIQuerying
    private let relayCacheTracker: RelayCacheTracker
    private let ipOverrideRepository: IPOverrideRepositoryProtocol

    let tunnelManager: TunnelManager

    init(
        tunnelManager: TunnelManager,
        apiProxy: APIQuerying,
        relayCacheTracker: RelayCacheTracker,
        ipOverrideRepository: IPOverrideRepositoryProtocol
    ) {
        self.tunnelManager = tunnelManager
        self.apiProxy = apiProxy
        self.relayCacheTracker = relayCacheTracker
        self.ipOverrideRepository = ipOverrideRepository
    }

    func makeVPNSettingsInteractor() -> VPNSettingsInteractor {
        VPNSettingsInteractor(tunnelManager: tunnelManager, relayCacheTracker: relayCacheTracker)
    }

    func makeProblemReportInteractor() -> ProblemReportInteractor {
        ProblemReportInteractor(apiProxy: apiProxy, tunnelManager: tunnelManager)
    }

    func makeSettingsInteractor() -> SettingsInteractor {
        SettingsInteractor(tunnelManager: tunnelManager)
    }

    func makeIPOverrideInteractor() -> IPOverrideInteractor {
        IPOverrideInteractor(repository: ipOverrideRepository, tunnelManager: tunnelManager)
    }
}
