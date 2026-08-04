//
//  PacketTunnelPathObserver.swift
//  PacketTunnel
//
//  Created by pronebird on 10/08/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import WarrenLogging
import WarrenRustRuntime
import WarrenTypes
import Network
import NetworkExtension
import PacketTunnelCore

final class PacketTunnelPathObserver: DefaultPathObserverProtocol, Sendable {
    private let eventQueue: DispatchQueue
    private let pathMonitor = NWPathMonitor()
    nonisolated(unsafe) let logger = Logger(label: "PacketTunnelPathObserver")
    private let stateLock = NSLock()

    nonisolated(unsafe) private var started = false
    nonisolated(unsafe) private var pendingPathUpdate: DispatchWorkItem?
    private static let pathUpdateDebounceDelay: DispatchTimeInterval = .seconds(2)

    public var currentPathStatus: Network.NWPath.Status {
        stateLock.withLock {
            pathMonitor.currentPath.status
        }
    }

    init(eventQueue: DispatchQueue) {
        self.eventQueue = eventQueue
    }

    func start(_ body: @escaping @Sendable (Network.NWPath.Status) -> Void) {
        stateLock.withLock {
            guard started == false else { return }
            defer { started = true }
            pathMonitor.pathUpdateHandler = { [weak self] updatedPath in
                guard let self else { return }
                // Warren relays dial over IPv4, so a satisfied-but-v4-less
                // path cannot carry the tunnel and is reported unsatisfied
                // (family gating, mirrors the desktop error-state gate).
                let effectiveStatus = updatedPath.status
                    .warrenEffectiveStatus(supportsIPv4: updatedPath.supportsIPv4)
                if case .satisfied = effectiveStatus {
                    // Wake the Rust migration watchdog on the same edge the
                    // reconnect trigger fires on, and before the debounce
                    // bookkeeping below: it migrates the live QUIC path onto
                    // the new interface, and escalates to that very reconnect
                    // when it cannot. Called outside `stateLock`, which guards
                    // this observer's own state and nothing the FFI touches.
                    WarrenQuinnAdapter.notifyPathChange()
                }
                self.stateLock.withLock {
                    self.pendingPathUpdate?.cancel()

                    let workItem = DispatchWorkItem {
                        body(effectiveStatus)
                    }
                    self.pendingPathUpdate = workItem

                    // Debounce only the offline edge: iOS synthesizes short
                    // unsatisfied blips on routine handovers that must not
                    // flash the offline treatment (same rising-edge debounce
                    // idea as the desktop's host-offline signal). Recovery
                    // is reported immediately so the reconnect trigger fires
                    // on the spot.
                    if case .satisfied = effectiveStatus {
                        self.eventQueue.async(execute: workItem)
                    } else {
                        self.eventQueue.asyncAfter(
                            deadline: .now() + Self.pathUpdateDebounceDelay,
                            execute: workItem
                        )
                    }
                }
            }

            pathMonitor.start(queue: eventQueue)
        }
    }

    func stop() {
        stateLock.withLock {
            guard started == true else { return }
            defer { started = false }
            pendingPathUpdate?.cancel()
            pendingPathUpdate = nil
            pathMonitor.pathUpdateHandler = nil
            pathMonitor.cancel()
        }
    }
}
