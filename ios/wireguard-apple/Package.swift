// swift-tools-version:5.9
// Warren placeholder for the upstream Mullvad `wireguard-apple` submodule.
// Warren replaces WireGuard with Quinn (see crates/warren-tunnel in warren-core).
// This stub satisfies Xcode SPM package graph resolution and keeps `xcodebuild
// -list` working. The XCLocalSwiftPackageReference entry in the Xcode project
// (and the `WireGuardKit*` framework links) are removed in sub-phase C.4 once
// the PacketTunnelProvider is migrated off WireGuardAdapter.
import PackageDescription

let package = Package(
    name: "WireGuardKit",
    platforms: [.iOS(.v15)],
    products: [
        .library(name: "WireGuardKit", targets: ["WireGuardKit"]),
        .library(name: "WireGuardKitTypes", targets: ["WireGuardKitTypes"]),
    ],
    targets: [
        .target(name: "WireGuardKit", dependencies: ["WireGuardKitTypes"]),
        .target(name: "WireGuardKitTypes"),
    ]
)
