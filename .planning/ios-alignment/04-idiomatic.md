# iOS Warren Client, Code Quality / Idiomatic / Legacy-Cleanup Audit

Scope: Swift iOS client of Warren (Mullvad VPN fork) under `ios/`. Read-only audit.
Focus: dead Mullvad/WireGuard code, naming leftovers, scaffold/policy comments, Swift
idiom in Warren-authored files, build/target hygiene.

Date: 2026-06-09. Branch: main.

---

## Severity-sorted summary table

| # | Severity | Area | File:line | Finding | Recommended action |
|---|----------|------|-----------|---------|--------------------|
| 1 | HIGH | Correctness | `PacketTunnelCore/Actor/WarrenQuinnActor.swift:326-331` | `setErrorState(reason:)` is a logging no-op but ships in production. Blocked-state reasons (revoked/logged-out device, no signal) from `PacketTunnelProvider.startDeviceCheckInner` are dropped, Settings panel cannot show the user-visible cause. | Wire the reason into the App Group broadcast / observed state so the main app surfaces it. Not safe to leave as no-op for release. |
| 2 | HIGH | Correctness | `PacketTunnelCore/Actor/WarrenQuinnActor.swift:313-317` | `reconnect(to:reconnectReason:)` is a no-op in production. Manual reconnect / relay-change reconnect requests do nothing; only the narrow `.unsatisfied -> .satisfied` branch of `updateNetworkReachability` calls `adapter.reconnect()`. | Implement re-marshal + `adapter.stop()` + `adapter.start(newConfig:)`, or call `adapter.reconnect()`. |
| 3 | HIGH | Correctness | `PacketTunnelCore/Actor/WarrenQuinnActor.swift:130-162` | `applyEvent` maps `.connected/.reconnecting/.failover` all to `.initial`. The actor NEVER reports `.connected`; any `observedStates` consumer never sees a connected transition (only `.disconnected` is definitive). | Map events to real `ObservedState` cases once connection details are threaded up (already flagged C.4.3.Z TODO, but it is shipping). |
| 4 | HIGH | Policy (em-dash) | `remove-mullvad-legacy-sources.rb:10` (x2), `drop-mullvad-actor-legacy.rb`, `restore-protocol-obfuscator.rb`, `fix-warrenlogging-spm-search.rb` | 4 authored `.rb` build scripts contain the banned em-dash `, ` in comments (`.rb` is a commentable config per CLAUDE.md). 5 occurrences total. | Replace each `, ` with a comma/colon/period. |
| 5 | HIGH | Policy (en-dash) | `WarrenVPN/View controllers/Wallet/WarrenWalletIdentityView.swift:18` | Doc comment `47-49 chars` uses the banned en-dash `, ` (numeric range). | Replace with hyphen: `47-49 chars`. |
| 6 | MED | Dead legacy code | `PacketTunnelCore/Actor/PacketTunnelActor*.swift` (11 files) | Mullvad-era `PacketTunnelActor` + 10 satellites on disk but dropped from ALL build targets (0 Sources entries, confirmed via `drop-mullvad-actor-legacy.rb`). The reducer is still referenced only by tests. | Safe to delete the production files (git preserves history). Keep/port the reducer tests only if the reducer is intentionally retained. |
| 7 | MED | Dead legacy code | `PacketTunnel/PostQuantum/*.swift` (4 files) + `WarrenRustRuntime/EphemeralPeer*.swift` (3) | Post-quantum ephemeral-peer exchange. `WarrenQuinnActor.notifyEphemeralPeerNegotiated` / `changeEphemeralPeerNegotiationState` are explicit no-ops (Warren is PQ-free). `EphemeralPeerExchangeActor.swift` dropped from build (0 entries); exchangers referenced only by their own test files. | Safe to delete `EphemeralPeerExchangeActor.swift`; the `PacketTunnel/PostQuantum` exchangers + their tests are dead in the Warren path, delete together, or keep behind the GotaTun debug path only if intentionally reserved. |
| 8 | MED | Build hygiene (stale target ref) | `WarrenVPN.xcodeproj/project.pbxproj:3961,4908-4916` | A `PBXGroup` named/`path = MullvadVPN` (id `DC80C06F35C54E9DCBB87ABD`) is a child of the `WarrenVPNTests` group. Path `MullvadVPN` does not exist on disk (real dir is `WarrenVPN`). Leftover from the Mullvad->Warren rename; clutters the navigator and is the only residual `MullvadVPN` token in the project. | Remove the stale group (or repoint its `path`/`name` to `WarrenVPN`). Verify the 3 children (Wallet / View controllers / Shared) still resolve through the real `WarrenVPN` group. |
| 9 | MED | Dead/legacy (WG monitor) | `PacketTunnelCore/Pinger/TunnelPinger.swift`, `PacketTunnelCore/TunnelMonitor/*`, `WgStats.swift` | WireGuard-era ICMP-ping tunnel monitor. `TunnelPinger.swift` dropped from all targets (0 entries). `TunnelMonitor.swift` still compiled (2 build entries) but NOT referenced by the Warren Quinn path (`WarrenQuinn*` files do not import/use it). | `TunnelPinger.swift` safe to delete. `TunnelMonitor.swift`/`WgStats.swift` need care: confirm no remaining test/target dependency before removing from the 2 build phases. |
| 10 | MED | Stale comment (false) | `Shared/WarrenWalletKeychain.swift:8-12` | Header says "Scaffold for C.5 ... NOT yet wired into the Xcode project (pbxproj target add is part of the C.5 implementation brief)". This is now false, the keychain IS wired and used by `WarrenQuinnTunnelImplementation.setUp` + `WarrenWalletInteractor`. Misleading narration/tombstone. | Delete the scaffold/"NOT yet wired" lines; keep only the security-design rationale. |
| 11 | MED | Naming leftover (internal API) | `WarrenVPN/**`, `.font(.mullvadBig/.mullvadSmall/...)`, `.mullvadBackground`, `.mullvadTextPrimary` etc. (200+ refs) | The shared design-system font/color API is still named `.mullvad*` and used pervasively across inherited UI (incl. Warren-authored `WarrenWalletIdentityView.swift:39`). Not user-facing, but a type-level "Mullvad" leftover. | Rename the design-system extensions to `.warren*` in a single mechanical pass (large but low-risk), or accept as load-bearing shared-API debt and document. Low priority vs correctness. |
| 12 | LOW | Naming/header | `PacketTunnelCore/Actor/GotaTunActor.swift:5-6`, `GotaTunTunnelImplementation.swift:5-6` | Warren-introduced debug-only files carry `Created by Mullvad VPN` / `Copyright © Mullvad VPN AB` headers (these are not upstream files). | Fix headers to Warren, or delete the whole GotaTun debug path if no longer needed (see #13). |
| 13 | LOW | Dead-ish debug scaffold | `PacketTunnelCore/Actor/GotaTunActor.swift`, `GotaTunTunnelImplementation.swift`, `Shared/PacketTunnelDebugSettings.swift` | `GotaTun*` is a pure no-op stub ("real GotaTun logic will be filled in later") wired only behind `#if DEBUG` + `PacketTunnelDebugSettings.useGotaTun`. It performs no tunnelling. Value is limited to UI/lifecycle smoke tests. | Decide: keep as deliberate debug smoke path (then fix headers #12 and the "will be filled in later" narration), or delete it + the `useGotaTun` toggle + the AccountViewController debug row. |
| 14 | LOW | Scaffold/TODO narration | `WarrenQuinnActor.swift:15-21,31-36,98,101,131-137,314-316,327-329` | Many `C.4.3 scaffold no-op`, `C.4.3.X follow-up`, `C.4.3.Z TODO` markers narrate task steps / future work rather than explaining "why". `init` logs `"(C.4.3.X wired)"`. Per CLAUDE.md "no step narration / no scaffold tombstones". | Trim to the load-bearing TODOs (findings #1, #3 reference real gaps); delete the step-narration and stale stage markers. |
| 15 | LOW | Misleading comment | `WarrenRustRuntime/WarrenQuinnAdapter.swift:22-23,376-380` | "The Quinn handshake / pump itself is wired in C.4.1; this file is the Swift transport plumbing" and "The Rust side currently ignores both fields (C.4.1 wires them...)", stage-progress narration; the marshalling-ignored note may be stale. | Verify against current Rust FFI; remove stage narration, keep only invariant docs (pointer-lifetime, copy-on-call contract). |
| 16 | LOW | Idiom (double event) | `WarrenQuinnActor.swift:236-242` | `stop()` calls `adapter.stop()` (which triggers a Rust `.disconnected` callback) AND then synthesizes `applyEvent(.disconnected)`. Benign double-yield onto `observedStates`; the comment acknowledges it as a fallback. | Acceptable; optionally guard against the redundant yield if a consumer is sensitive to duplicate `.disconnected`. |

---

## Detail notes

### 1. Tunnel-implementation selection (context)
`PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift:96-106`: production always uses
`WarrenQuinnTunnelImplementation`; `GotaTunTunnelImplementation` only under `#if DEBUG` +
`useGotaTun`. The legacy `WireGuardGoTunnelImplementation` / `WgAdapter` are gone from disk
(file no longer exists). So the actual data-plane path is Quinn-only, good.

### 2. Mullvad shared packages, STILL LOAD-BEARING (do NOT delete)
The `Mullvad*`-named files in `WarrenRustRuntime/` (`MullvadApiContext`, `MullvadAccessMethodReceiver`,
`MullvadConnectionModeProvider`, `MullvadShadowsocksBridgeProvider`, `MullvadAddressCacheKeychainStore`,
`MullvadApi*`, `MullvadAPIMock`) and `MullvadPostQuantum/` are wired into the live
`PacketTunnelProvider` (API context, access-method receiver, shadowsocks loader, device check via
`accountsProxy`/`devicesProxy`). The app still rides the Mullvad-derived REST/account/access-method
backend. These are naming leftovers but functionally load-bearing, only a rename would apply, not
deletion, and that is a larger effort than this audit recommends.

Note: this means the "Mullvad account-number / key-rotation / relay config" machinery flagged in the
task brief is largely still in use for account/device-check/relay-selection plumbing even though the
*data plane* is Quinn. `WarrenQuinnActor.notifyKeyRotation` is correctly a no-op (static wallet
identity), but the account/device side still uses Mullvad device/account proxies via `DeviceChecker`.

### 3. Localizable.xcstrings, CLEAN
`Assets/Localizable.xcstrings`: 0 occurrences of "Mullvad" (already bulk-replaced). No em-dash/en-dash
in any `.xcstrings`/`.strings`. User-facing strings are clean.

### 4. Em-dash / en-dash scan results
- Authored Swift: 0 em-dashes; 1 en-dash (#5 above).
- `.xcstrings` / `.strings`: 0.
- `.rb` build scripts: 5 em-dashes across 4 files (#4 above).
- `.sh` scripts: 0.

### 5. French comments, CLEAN
No French comments found in Warren-authored Swift (`WarrenVPN`, `WarrenRustRuntime`, `PacketTunnel`,
`PacketTunnelCore`, `Shared`). Comments are English-only per policy.

### 6. Swift idiom, generally GOOD
- No force-unwraps / `try!` / `as!` / `fatalError` in the Warren-authored tunnel + wallet files
  (the only `fatalError` is the standard unavailable `init?(coder:)`).
- `WarrenWalletInteractor` correctly off-loads blocking FFI/Keychain to a `.userInitiated` queue and
  hops results back via `@MainActor` completions; closures use `[weak self]`.
- `WarrenQuinnAdapter` FFI bridging is careful: retained `Unmanaged` self-ref released on both
  `stop()` and (defensively) `deinit`; sensitive seed bytes zeroed in a `defer`; `NSLock`-guarded
  mutable state; `@convention(c)` callbacks recover context via `takeUnretainedValue`.
- `WarrenWallet` zeroes seed in `deinit` (`memset_s`) mirroring the Rust `Zeroizing`.
- `WarrenWalletKeychain` uses `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, no iCloud sync, sound.
- Real correctness gaps are the shipping no-ops (#1, #3), not memory/concurrency unsafety.

### 7. Build/target hygiene
- Targets/entitlements/xctestplans already renamed to `WarrenVPN.*` (entitlements:
  `WarrenVPN/Supporting Files/WarrenVPN.entitlements`, `PacketTunnel/PacketTunnel.entitlements`;
  xctestplans all `WarrenVPN*.xctestplan`). Bundle IDs are `com.warrenbrowse.vpn.ios.*`.
- The ONLY residual `MullvadVPN` token in the project file is the stale `PBXGroup` (#8).
- The `.rb` scripts are the build-graph manipulation layer (add Warren files, drop Mullvad legacy
  from targets, fix SPM search paths). They are the reason legacy files persist on disk but are not
  compiled. Files dropped from all targets (`PacketTunnelActor*`, `TunnelPinger`,
  `EphemeralPeerExchangeActor`) are safe-delete candidates; files still in build phases
  (`TunnelMonitor`, `WgStats`, `WireGuardKey` x4) need a dependency check first.

### Safe-to-delete vs needs-care (legacy)
- SAFE TO DELETE (0 build references, git keeps history): `PacketTunnelActor*.swift` (11),
  `Pinger/TunnelPinger.swift`, `WarrenRustRuntime/EphemeralPeerExchangeActor.swift`.
- LIKELY DELETABLE (dead in Warren path, referenced only by own tests): `PacketTunnel/PostQuantum/*`
  (4) + their tests; `WarrenRustRuntime/EphemeralPeerNegotiator.swift`,
  `EphemeralPeerReceiver.swift`.
- NEEDS CARE (still compiled): `TunnelMonitor/*`, `WgStats.swift`, `WireGuardKey.swift`, confirm
  test/target deps; `TunnelObfuscator*` are restored on purpose by `restore-*-obfuscator.rb`.
- DO NOT DELETE: all `Mullvad*` REST/API/access-method files + `MullvadPostQuantum/` (load-bearing
  account/API backend).
