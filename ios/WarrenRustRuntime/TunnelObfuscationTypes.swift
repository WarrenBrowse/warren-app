//
//  TunnelObfuscationTypes.swift
//  WarrenRustRuntime
//
//  Created by Warren on 2026-05-22 (C.4.5 partial — Warren tunnels via
//  Quinn over HTTP/3 mimicry baked into the warren-tunnel stack, NOT
//  via Mullvad's local obfuscation-proxy pattern). This file extracts
//  just the namespace + protocol + enum surface so legacy Mullvad
//  consumers (ProtocolObfuscator, PacketTunnelActor) still compile.
//  The actual FFI-backed `TunnelObfuscator` class is replaced by a
//  no-op stub since Warren never needs to spin up a local proxy.
//

import Foundation
import Network
import WarrenTypes

public enum TunnelObfuscationProtocol {
    case udpOverTcp
    case shadowsocks
    case quic(hostname: String, token: String)
    case lwo(serverPublicKey: WireGuard.PublicKey, clientPublicKey: WireGuard.PublicKey)

    public var isLwo: Bool {
        if case .lwo = self { return true }
        return false
    }
}

public protocol TunnelObfuscation {
    init(
        remoteAddress: IPAddress,
        remotePort: UInt16,
        obfuscationProtocol: TunnelObfuscationProtocol
    )
    func start()
    func stop()
    var localUdpPort: UInt16 { get }
    var remotePort: UInt16 { get }
    var transportLayer: TransportLayer { get }
}

/// Warren no-op stub `TunnelObfuscation` implementation. Warren's
/// HTTP/3 mimicry (M4.0) is baked into the `warren-tunnel` Quinn
/// transport_config and applied uniformly to every connection ; no
/// local proxy is needed. Consumers that instantiate this stub get
/// a `localUdpPort = remotePort` passthrough — effectively a no-op
/// obfuscator that leaves the original endpoint intact.
public final class TunnelObfuscator: TunnelObfuscation {
    private let remoteAddress: IPAddress
    private let port: UInt16
    private let obfuscationProtocol: TunnelObfuscationProtocol

    public var localUdpPort: UInt16 { port }
    public var remotePort: UInt16 { port }

    public var transportLayer: TransportLayer {
        switch obfuscationProtocol {
        case .udpOverTcp:
            .tcp
        case .shadowsocks, .quic, .lwo:
            .udp
        }
    }

    public init(
        remoteAddress: IPAddress,
        remotePort: UInt16,
        obfuscationProtocol: TunnelObfuscationProtocol
    ) {
        self.remoteAddress = remoteAddress
        self.port = remotePort
        self.obfuscationProtocol = obfuscationProtocol
    }

    public func start() {
        // No-op : Warren's M4.0 HTTP/3 mimicry is applied by the
        // warren-tunnel Quinn transport layer, not by a local proxy.
    }

    public func stop() {
        // No-op : nothing to tear down.
    }
}
