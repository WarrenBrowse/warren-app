# iOS Security Audit — Warren-native parts & parity with Android / desktop daemon

Scope: `ios/PacketTunnel`, `ios/PacketTunnelCore`, `ios/WarrenRustRuntime`, `ios/WarrenVPN`, `ios/Shared`, `ios/WarrenSettings`.
Method: source read only, no code changed, no build run. Date: 2026-06-09.

Comparison baseline (other clients):
- Android P0 leaks fixed: IPv6 blackhole, in-tunnel DNS, kill-switch blackhole-TUN.
- Desktop CRITICAL was: mnemonic readable via world-readable management socket. Seed/mnemonic must have minimal in-memory lifetime, never logged.
- Mnemonic UX: onboarding deliberately shows mnemonic + copy button; seed must not leak beyond that to logs/crash/App-Group/disk.

## Severity-sorted summary

| # | Severity | Area | Finding | File:line |
|---|----------|------|---------|-----------|
| 1 | HIGH | Leak protection (DNS) | Warren Quinn path never sets `NEDNSSettings`; no in-tunnel DNS enforcement (Android has it). DNS can resolve outside the tunnel. | `PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift:263`; `PacketTunnelCore/Actor/WarrenQuinnActor.swift` (no `apply(settings:)`) |
| 2 | HIGH | Leak protection (kill switch) | No `includeAllNetworks` / on-disconnect blackhole. On tunnel drop / extension crash / sleep, traffic can leak. No fail-closed equivalent to Android blackhole-TUN. | `PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift:263-285`; `WarrenQuinnActor.swift:270-291` |
| 3 | HIGH | Keychain cross-process | Wallet mnemonic stored with NO `kSecAttrAccessGroup` and no `keychain-access-groups` entitlement, yet the PacketTunnel extension reads it. Either the extension cannot read the seed (functional break) or relies on an unstated shared-group assumption. Inconsistent with `KeychainSettingsStore` which sets an access group. | `Shared/WarrenWalletKeychain.swift:54-78`; `PacketTunnelCore/Actor/WarrenQuinnTunnelImplementation.swift:118`; entitlements files |
| 4 | MED | Keychain protection class | Wallet uses `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`; extension cannot read seed when device locked, breaking on-demand/background reconnect. Settings store uses `AfterFirstUnlock`. Inconsistent + may force fail-open or connect failure in background. | `Shared/WarrenWalletKeychain.swift:61` |
| 5 | MED | Biometric gating | Biometric check is a separate `LAContext.evaluatePolicy` NOT bound to the Keychain item (`kSecAttrAccessControl`/`SecAccessControl`). The gate can be bypassed by any code path that calls `WarrenWalletKeychain.load()` directly (e.g. the extension does, with no auth). | `WarrenVPN/.../WarrenWalletInteractor.swift:189-233`; `Shared/WarrenWalletKeychain.swift:81-103` |
| 6 | MED | FFI lifetime / UAF | Outbound + event callbacks recover `self` via `Unmanaged.takeUnretainedValue()` from a Rust/Tokio thread. `stop()` releases the retain and then drops the Rust handle; if a callback is already in flight on another thread when the retain is released, it can use a deallocated `self`. Ordering relies on Rust dropping the dispatcher synchronously inside `warren_tunnel_stop`. | `WarrenRustRuntime/WarrenQuinnAdapter.swift:238-257, 528-541, 553-582` |
| 7 | MED | Seed in-memory lifetime | Wallet seed `Data` is copied into `WarrenQuinnActor.walletSigningSeed` and held for the whole connected session; only `stop()` clears it. `Data` is not zeroized on clear (just dropped). Wider window + no wipe vs the deliberate `WarrenWallet.deinit` `memset_s`. | `PacketTunnelCore/Actor/WarrenQuinnActor.swift:66,119-123,229-235` |
| 8 | LOW | Log redaction | `ConsolidatedApplicationLog` redacts account numbers / IPs / paths but has NO rule for a BIP39 mnemonic or hex seed. If a seed/mnemonic ever reaches a log (Rust `init_rust_logging` forwards all Rust logs verbatim), it is shipped to support unredacted. | `WarrenVPN/Classes/ConsolidatedApplicationLog.swift:183-190`; `WarrenRustRuntime/RustLogging.swift:37-42` |
| 9 | LOW | App Group exposure | App Group `UserDefaults` carries only stats + failover country + NAT-PMP port + state label. No secret. Acceptable; noted for completeness. | `Shared/WarrenAppGroupKey.swift`; `WarrenQuinnTunnelImplementation.swift:159-225` |
| 10 | LOW | TLS / endpoint trust | Warren Quinn transport trust is anchored entirely in Rust (`warren_tunnel_start` takes the exit Ed25519 pubkey); no Swift-side TLS/cert decision. Cannot verify pinning/validation from Swift source alone — verify in `warren-tunnel`/`warren-core`. | `WarrenRustRuntime/WarrenQuinnAdapter.swift:381-457`; `include/warren_rust_runtime.h:313` |
| 11 | LOW | Bypass CIDRs / multihop allowlist | `bypassCidrs` is plumbed Swift→FFI but always `[]` from the actor; multihop exit allowlist/rate-limit (a known core issue) is not visible on iOS. Confirm enforcement is server-side. | `WarrenQuinnActor.swift:210-218` |

---

## Detail

### 1. HIGH — No in-tunnel DNS on the Warren Quinn path (DNS leak / Android parity gap)

`PacketTunnelProvider.startTunnel` applies `initialTunnelNetworkSettings()` (`PacketTunnelProvider.swift:145,263`) which sets IPv4 + IPv6 **default routes** but assigns **no `dnsSettings`**:

```
let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "\(IPv4Address.loopback)")
// ipv4Settings.includedRoutes = [.default()]; ipv6Settings.includedRoutes = [.default()]
// <-- no settings.dnsSettings assigned
```

The in-tunnel DNS code (`NEDNSSettings(servers:)` with `matchDomains = [""]`, forcing all DNS through the tunnel) lives in `TunnelInterfaceSettings.asTunnelSettings()` (`Protocols/TunnelAdapterProtocol.swift:63-76`). That is only invoked from `PacketTunnelActor.swift:296` and `PacketTunnelActor+PostQuantum.swift:91,108` — the legacy Mullvad/WireGuard actor. The Warren path uses `WarrenQuinnActor`, which **never** calls `apply(settings:)` and never sets DNS. Net effect: the system keeps its pre-tunnel resolvers, so DNS queries can egress outside the QUIC tunnel.

Risk: DNS leak (queries observable by the local network / ISP) and a clear regression vs Android's enforced in-tunnel DNS.

Fix: in `WarrenQuinnActor.start(...)` (or in `WarrenQuinnTunnelImplementation`) re-apply `NEPacketTunnelNetworkSettings` once connected, including `NEDNSSettings` with the Warren resolver(s) and `matchDomains = [""]`. Cannot verify from Swift alone what resolver Warren expects; coordinate with the Rust tunnel (the exit-side DNS). At minimum point DNS at an address routed only inside the tunnel.

### 2. HIGH — No kill switch / fail-closed on drop, crash, or sleep

There is no `includeAllNetworks`, no `excludeLocalNetworks=false` lockdown, and no blackhole route installed when the tunnel is not up:
- `initialTunnelNetworkSettings()` adds default routes but the routes only take effect while the extension is alive and the flow is pumping.
- `WarrenQuinnActor.onSleep()` calls `adapter.pause()` (`WarrenQuinnActor.swift:270-280`), which stops the inbound pump but keeps the NE routes; on a longer suspension the Quinn connection dies (peer idle timeout) while the default routes still point at a dead tunnel — packets are dropped rather than blackholed deliberately, but there is no explicit fail-closed guarantee and no IPv6 blackhole equivalent to Android.
- On extension crash/restart, until `setTunnelNetworkSettings` re-applies, the OS may route via the physical interface.

Risk: traffic / IP leak on tunnel drop, sleep/wake, or extension restart. Android explicitly blackholes (IPv6 + kill-switch TUN); iOS Warren path has no equivalent.

Fix: adopt a fail-closed posture: keep default routes installed even in error/disconnected state (Mullvad's blocked-state pattern, `PacketTunnelActor+ErrorState.swift`) so the Warren actor enters a blocking state instead of tearing routes down; consider `NETunnelProviderManager` `includeAllNetworks`/lockdown when the user enables a kill switch. Verify behavior on real device with `tcpdump` (cannot be verified from source).

### 3. HIGH — Wallet Keychain has no access group but is read cross-process

`WarrenWalletKeychain` (`Shared/WarrenWalletKeychain.swift:54-78`) writes the mnemonic with:
```
kSecClass: kSecClassGenericPassword
kSecAttrService: "com.warrenbrowse.vpn.ios.wallet"
kSecAttrAccount: "mnemonic"
kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
// NO kSecAttrAccessGroup
```
Neither `WarrenVPN.entitlements` nor `PacketTunnel.entitlements` declares `keychain-access-groups` (only `application-groups`). By contrast, `KeychainSettingsStore` (`WarrenSettings/KeychainSettingsStore.swift:99-104`) explicitly sets `kSecAttrAccessGroup`.

But `WarrenQuinnTunnelImplementation.setUp` (in the extension process) calls `WarrenWalletKeychain.load()` to derive the signing seed (`WarrenQuinnTunnelImplementation.swift:118`). Generic-password items without an explicit access group default to the app's primary application-identifier access group, which is NOT shared between the main app and the extension. So either:
- the extension's `load()` returns `errSecItemNotFound` and the tunnel silently no-ops auth (functional break + the `catch` only logs a warning, `WarrenQuinnTunnelImplementation.swift:123-127`), or
- there is an unstated build-time shared default that happens to work (fragile, undocumented).

Risk: fragile / undocumented cross-process secret sharing; at best a latent functional break, at worst a future change that silently widens accessibility.

Fix: add a dedicated `keychain-access-groups` entry shared by both targets and set `kSecAttrAccessGroup` explicitly in `WarrenWalletKeychain` (mirror `KeychainSettingsStore`). Keep it as narrow as possible (its own group, not reused for settings).

### 4. MED — `WhenUnlockedThisDeviceOnly` blocks background/on-demand reads

`kSecAttrAccessibleWhenUnlockedThisDeviceOnly` (`WarrenWalletKeychain.swift:61`) means the seed is unreadable while the device is locked. The PacketTunnel extension frequently starts on-demand (no UI, possibly locked device). It will fail to load the seed and cannot authenticate the tunnel. The settings store deliberately uses `AfterFirstUnlock` for exactly this reason.

Risk: connect-on-demand failure when locked; or, if a future fix loosens it carelessly, over-broad accessibility.

Fix: decide the threat model. If background connect is required, use `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` (still device-local, survives lock after first unlock). Keep `ThisDeviceOnly` to forbid iCloud/backup migration. Do NOT use plain `WhenUnlocked` without `ThisDeviceOnly`.

### 5. MED — Biometric gate not bound to the Keychain item

`loadMnemonicWithAuth` (`WarrenWalletInteractor.swift:189-233`) runs `LAContext.evaluatePolicy(.deviceOwnerAuthentication)` and, on success, calls `WarrenWalletKeychain.load()`. The biometric check and the Keychain read are independent: the item is not protected with a `SecAccessControl` (`kSecAttrAccessControl` with `.userPresence`/`.biometryCurrentSet`). Any caller that invokes `WarrenWalletKeychain.load()` directly (the extension does, `WarrenQuinnTunnelImplementation.swift:118`) reads the mnemonic with no auth at all. The Face ID prompt is therefore advisory UX, not an enforced access control.

Risk: the "biometric-gated mnemonic reveal" is bypassable; the secret is retrievable by any in-process code without user presence.

Fix: if the mnemonic reveal must be biometric-gated, attach a `SecAccessControl` (e.g. `.biometryCurrentSet`/`.userPresence`) to the item and pass an `LAContext` via `kSecUseAuthenticationContext`. Note this conflicts with silent extension reads (finding 4) — likely the right design is two items, or derive+store only the non-secret pubkey for the extension and keep the mnemonic biometric-gated. Reconcile 3/4/5 together.

### 6. MED — FFI callback can use released `self` (UAF window)

In `start` (`WarrenQuinnAdapter.swift:201-235`) a single `Unmanaged.passRetained(self)` backs both the outbound and event callbacks; `stop` (`238-257`) sets `handle=nil`, calls `warren_tunnel_stop`, then `oldRetain?.release()`. Both C callbacks (`outboundCallback:528`, `eventCallbackBridge:553`) do `Unmanaged.fromOpaque(contextPtr).takeUnretainedValue()` from a Tokio thread. Correctness depends entirely on `warren_tunnel_stop` synchronously joining/dropping the dispatcher so that no callback is mid-flight when `release()` runs. The header (`warren_rust_runtime.h:381-403`) says the callback "must outlive the call to `warren_tunnel_stop`" but does not state stop is synchronous w.r.t. in-flight callbacks. If the Rust side drops the channel but a callback closure is already executing on another thread, `takeUnretainedValue()` touches freed memory.

Also `deinit` (`177-185`) calls `warren_tunnel_stop` but does NOT release `ffiContextRetain`, so reaching deinit with a live handle leaks the retain (benign vs UAF, but a correctness smell).

Risk: rare use-after-free / data race on stop, especially under teardown + concurrent inbound traffic.

Fix: confirm (in `warren-tunnel`) that `warren_tunnel_stop` blocks until all dispatcher tasks that may invoke the callbacks have terminated, before returning. If it cannot guarantee that, gate callback bodies behind an atomic "alive" flag and use `passRetained` semantics that the callback itself balances, or take a strong ref inside a lock. Cannot fully verify the Rust side from this repo slice.

### 7. MED — Seed held for the whole session, not zeroized on clear

`WarrenQuinnActor` copies the 32-byte seed `Data` (`bindWalletSigningSeed`, `:119-123`) and keeps it in `walletSigningSeed` for the entire connected session, clearing it only in `stop()` (`:229-235`). Clearing is `self.walletSigningSeed = nil` — the underlying `Data` bytes are not wiped (unlike `WarrenWallet.deinit` which `memset_s`-zeroes, `WarrenWallet.swift:51-61`). The FFI already copies the seed by value into Rust (`callTunnelStart` wipes its local `signingSeedBytes`, `WarrenQuinnAdapter.swift:390-395`), so the actor's long-lived copy is an avoidable second residence of the secret.

Risk: longer plaintext-seed lifetime in extension heap; recoverable via memory dump / crash report.

Fix: don't retain the seed in the actor past `adapter.start(config:)`. Pass it straight through and drop it. If it must be retained for reconnect, hold it in a zeroizing buffer and `memset_s` on clear.

### 8. LOW — Mnemonic / seed not in the log redactor

`ConsolidatedApplicationLog.redact` (`:183-190`) covers account numbers, IPv4/IPv6, container paths, and caller-supplied custom strings — nothing for a BIP39 phrase or 32/64-byte hex. Rust logs are forwarded verbatim to the Swift logger (`RustLogging.swift:37-42`) and end up in the on-disk packet-tunnel log that the problem-report flow ships. No current Warren Swift code logs the secret (verified: `WarrenWalletInteractor`, `WarrenWalletCoordinator`, display views log only metadata), so this is defense-in-depth.

Risk: if any Rust/Swift path ever logs the seed/mnemonic, it is shipped to support unredacted.

Fix: add a redaction rule for 12/24-word BIP39 sequences and long hex blobs; and assert in `warren-tunnel`/`warren-identity` that the seed is never logged.

### 9. LOW — App Group contents reviewed, no secret

`WarrenQuinnTunnelImplementation` writes only `bytesIn/Out`, `connectedDurationSeconds`, `failoverCount`, `stateLabel`, `lastFailoverExit`, `lastFailoverAt`, `natPmpExternalPort` to App Group `UserDefaults` (`:159-225`, keys in `Shared/WarrenAppGroupKey.swift`). No seed, no pubkey, no endpoint. App Group is sandbox-local. Acceptable.

### 10. LOW — Transport trust is Rust-side only (cannot verify here)

The Warren tunnel is established by `warren_tunnel_start` with the exit's 32-byte Ed25519 pubkey passed by value (`WarrenQuinnAdapter.swift:439-449`, `warren_rust_runtime.h:121-156`). There is no Swift-side TLS/cert/SPKI pinning or trust decision for the Warren data plane (distinct from the Mullvad REST API context, which has its own TLS host validation, `warren_rust_runtime.h:575`). Endpoint/pubkey authenticity therefore depends entirely on the Rust `warren-tunnel`/`warren-core` handshake.

Risk: if the Rust handshake does not actually authenticate the exit against the supplied pubkey, iOS has no second line of defense. Out of scope for Swift-only review.

Fix: verify in `warren-core` that the QUIC/Noise handshake binds to the provided exit pubkey and rejects mismatches; ensure the pubkey itself is delivered over an authenticated channel (relay list signature).

### 11. LOW — bypassCidrs / multihop allowlist not exercised on iOS

`WarrenQuinnActor.start` always passes `bypassCidrs: []` and `natPmpEnabled: false` (`:210-218`); the marshalling exists but is unused. The known core concern "multihop exit skips allowlist/rate-limit" is not observable on the iOS client (enforcement is server-side). No client-side action, but flag for the core audit.

---

## What could NOT be verified from source alone
- Whether `warren_tunnel_stop` synchronously drains the dispatcher (finding 6) — needs `warren-tunnel` Rust.
- Whether the QUIC handshake authenticates the exit against the supplied pubkey (finding 10) — needs `warren-core`.
- Real-device leak behavior on drop/sleep/crash (findings 1, 2) — needs on-device `tcpdump`/Charles.
- Whether the no-access-group Keychain item is actually readable by the extension at runtime (finding 3) — needs a device run; static evidence says it should not be.
- iCloud Keychain / backup exposure of the mnemonic — `ThisDeviceOnly` forbids it in code, not verified at OS level.
