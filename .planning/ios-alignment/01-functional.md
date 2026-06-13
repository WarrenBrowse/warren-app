# iOS Warren Tunnel, Functional Completeness & Parity Audit

Scope: `ios/PacketTunnel`, `ios/PacketTunnelCore`, `ios/WarrenRustRuntime`, `ios/WarrenVPN`.
Reference targets: Android `WarrenQuinnAdapter.kt` + `WarrenTunInterfacePlan.kt` + `WarrenTunnelConfig.kt`; desktop daemon.
Method: read-only. All file:line citations verified against the working tree at audit time.

> Bottom line: the iOS Warren tunnel is a **scaffold that connects but does not protect**. The TUN network settings are hardcoded (no DNS-in-tunnel, no config-driven routing, no IPv6 blackhole choice, no kill-switch/blocking interface), the actor never reports a real `connected`/`error` state to the UI, and several config fields marshaled across the FFI (DNS, allow-LAN, lockdown, MTU, NAT-PMP, bypass CIDRs, obfuscation) are either hardcoded off or absent entirely. Wallet/account/voucher/subscription/support-report/relay-list are in good shape and largely at parity.

## Findings (severity-sorted)

| # | Severity | Area | Location | One-line |
|---|----------|------|----------|----------|
| F1 | BLOCKER | DNS leak | `PacketTunnelProvider.swift:263-285` | iOS TUN sets **no DNS servers** at all; queries leak to LAN resolver (Android forces in-tunnel `10.66.0.1`). |
| F2 | BLOCKER | Kill-switch / lockdown | `WarrenQuinnActor.swift` (whole), `WarrenTunInterfacePlan.kt:88-102` | No blocking/blackhole interface on tunnel drop; iOS has no equivalent of Android `enterBlockingMode`/lockdown. Traffic leaks when the tunnel dies. |
| F3 | BLOCKER | Error surfacing | `WarrenQuinnActor.swift:326-331` | `setErrorState(reason:)` is a no-op; tunnel can never enter `ObservedState.error`, so the UI can never show a blocked/failed reason. |
| F4 | BLOCKER | Connected state never reported | `WarrenQuinnActor.swift:130-162` | `applyEvent(.connected)` maps to `ObservedState.initial` (nil `connectionState`); the main-app UI can never render "Connected" with relay/IP. |
| F5 | HIGH | IPv6 leak control | `PacketTunnelProvider.swift:277-282` | iOS always assigns an IPv6 address + default route unconditionally; no `enableIpv6=false` blackhole path (Android default-off blackhole). |
| F6 | HIGH | DNS config ignored | `WarrenQuinnActor.swift:210-218`, `WarrenQuinnAdapter.swift` (no DNS field) | Custom DNS, content-blocking flags, and default exit-forwarder routing are never marshaled to the tunnel (Android `DnsConfig`). |
| F7 | HIGH | Allow-LAN / split routing | `PacketTunnelProvider.swift:269-282` | Routes are hardcoded `0.0.0.0/0` + `::/0`; no allow-LAN RFC1918 exclusion (Android `ipv4RoutesExcluding`). |
| F8 | HIGH | NAT-PMP disabled | `WarrenQuinnActor.swift:216`, no settings key | `natPmpEnabled` hardcoded `false`; comment "deferred (needs V9 schema)". No settings field exists. |
| F9 | HIGH | Bypass CIDRs disabled | `WarrenQuinnActor.swift:217`, no settings key | `bypassCidrs` hardcoded `[]`; "settings-driven, deferred". No settings field. |
| F10 | HIGH | `reconnect(to:reason:)` no-op | `WarrenQuinnActor.swift:313-317` | Relay-change reconnect (the IPC-driven path used by `TunnelManager.reconnectTunnel`) is a logged no-op; only the same-relay path observer call works. |
| F11 | MED | DAITA spec is a placeholder | `WarrenQuinnActor.swift:201-208` | DAITA marshaled as empty seed / padding 0; only "presence = on". Functionally a boolean, no machine selection like Android `padding_machine`. |
| F12 | MED | Obfuscation indicator never written | `WarrenQuinnTunnelImplementation.swift:211-225` | UI reads `WarrenAppGroupKey.obfuscationActive` but the producer never writes it; indicator is dead. |
| F13 | MED | MTU hardcoded | `PacketTunnelProvider.swift` (no MTU set on Warren path), `TunnelAdapterProtocol.swift:70` | Warren path never sets MTU on the network settings; no config-driven MTU (Android clamps `config.mtu`). |
| F14 | MED | sleep/wake relies on unimplemented pause/resume semantics | `WarrenQuinnActor.swift:270-291` | `onSleep`/`onWake` call `adapter.pause()/resume()`; correctness depends on Rust `warren_tunnel_pause/resume` actually preserving the connection (unverified here). |
| F15 | MED | `applyEvent` cannot represent `connecting`/`reconnecting`/`failover` distinctly | `WarrenQuinnActor.swift:140-149` | All non-disconnect events collapse to `.initial`; failover/reconnecting transitions invisible to actor state. |
| F16 | MED | No `EventConnecting` handling | `WarrenQuinnAdapter.swift:560-580`, header `EventConnecting=7` | The FFI defines `EventConnecting` but the Swift bridge has no case for it (falls through `default: return`), so first-attempt events are dropped. |
| F17 | LOW | Lockdown UI control absent / misrouted | `ios/.../IncludeAllNetworks/*` | iOS exposes `includeAllNetworks` (Mullvad concept) but there is no Warren lockdown-mode setting feeding the (missing) blocking interface. |
| F18 | LOW | NAT-PMP status not pollable on iOS | header (no `warren_tunnel_natpmp_status`) | iOS gets NAT-PMP only via transient events; no status read like Android `WarrenJni.getNatPmpStatus()`. |
| F19 | LOW | Wallet bind failure is silent at start | `WarrenQuinnTunnelImplementation.swift:117-127`, `WarrenQuinnActor.swift:174-177` | Missing wallet → `start()` logs and returns; no `setErrorState`, so UI shows nothing (compounds F3). |

Wallet / account / voucher / subscription / support-report / relay-list: **present and wired** on iOS (see "Feature parity" section). No blocker there.

---

## Detail

### F1, BLOCKER: iOS TUN sets no DNS servers (DNS leak)

`PacketTunnelProvider.initialTunnelNetworkSettings()` (`ios/PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift:263-285`) builds `NEPacketTunnelNetworkSettings` with `ipv4Settings`/`ipv6Settings` and default routes but **never sets `settings.dnsSettings`**. There is a correct pattern in the codebase, `TunnelInterfaceSettings.asTunnelSettings()` (`ios/PacketTunnelCore/Actor/Protocols/TunnelAdapterProtocol.swift:63-76`) sets `NEDNSSettings(servers:)` + `matchDomains = [""]`, but it belongs to the **unused** WireGuard adapter path (`TunnelAdapterProtocol` is referenced only by `PacketTunnelActor`/`PacketTunnelActorReducer`, not by the Warren path). Android forces an in-tunnel resolver (`WarrenTunInterfacePlan.kt:104,205-214`, default `10.66.0.1`) so DNS can never leak. On iOS, with no `dnsSettings`, the system keeps using the underlying network's resolver → **plaintext DNS leak outside the tunnel**.
**Action:** In the Warren path, set `settings.dnsSettings = NEDNSSettings(servers:[...])` with `matchDomains=[""]`, sourced from the resolved DNS config (custom servers or the exit forwarder), mirroring `resolveDnsServers` in `WarrenTunInterfacePlan.kt`.

### F2, BLOCKER: No kill-switch / lockdown blocking interface on tunnel drop

Android's adapter keeps a blackhole `blockingFd` up when the tunnel drops under lockdown (`WarrenQuinnAdapter.kt:71-74,244-295`, plan at `WarrenTunInterfacePlan.kt:88-102`) and only tears it down once a new tunnel is confirmed (`exitBlockingMode` after a successful connect). iOS `WarrenQuinnActor` has **no blocking-interface concept at all**: on a Rust-side disconnect it just fires `applyEvent(.disconnected)` (`WarrenQuinnActor.swift:143-148,229-243`) and the NEPacketTunnelProvider can return traffic to the physical interface. There is no flap detector, no lockdown reconnect loop, no fail-closed behaviour.
**Action:** Implement a fail-closed path: keep `setTunnelNetworkSettings` with a full-capture / no-DNS "blocking" config installed on unexpected drop (NE keeps routing into the dead tunnel = drop), gated by a Warren lockdown setting; add flap detection + bounded reconnect mirroring `onSessionDown`/`scheduleLockdownReconnect`.

### F3, BLOCKER: `setErrorState` is a no-op

`WarrenQuinnActor.setErrorState(reason:)` (`ios/PacketTunnelCore/Actor/WarrenQuinnActor.swift:326-331`) only logs. The blocked-state plumbing is otherwise complete and waiting: `PacketTunnelProvider.startDeviceCheckInner()` calls `implementation.actor.setErrorState(reason:)` on a failed device check (`PacketTunnelProvider.swift:345,356`); `BlockedStateErrorMapper` (`BlockedStateErrorMapper.swift:18-77`) maps real errors to `BlockedStateReason`; `TunnelManager` reads `observedState.blockedState` to drive UI (`ios/WarrenVPN/TunnelManager/TunnelManager.swift:873`) and `TunnelState` has a `.error(BlockedStateReason)` case (`ios/WarrenVPN/TunnelManager/TunnelState.swift:90,127-128`). Because the actor never transitions to `ObservedState.error(...)`, none of this can ever fire on the Warren path.
**Action:** In `setErrorState`, set `currentState = .error(ObservedBlockedState(reason:...))`, yield it on `observedStatesContinuation`, and engage the F2 blocking interface (revoked wallet / no relays / offline should fail closed).

### F4, BLOCKER: Connected state never reported to UI

`WarrenQuinnActor.applyEvent` maps `.connected`/`.reconnecting`/`.failover` → `ObservedState.initial` (`WarrenQuinnActor.swift:138-149`), with an explicit `C.4.3.Z TODO` admitting it lacks the `ObservedConnectionState` payload. `ObservedState.initial` has nil `connectionState` (`ios/PacketTunnelCore/Actor/ObservedState+Extensions.swift:34-47`). The main app derives the entire connection UI (relay name, IP, attempt count, key rotation, reachability) from `observedState.connectionState` (`TunnelManager.swift:225,470,582,876`). Result: even when the Quinn tunnel is genuinely up, the app cannot show "Connected" with details, it shows the equivalent of "initial/connecting".
**Action:** Have `WarrenQuinnTunnelImplementation.startStatsBroadcastTask` (or the event callback) build an `ObservedConnectionState` (selected relay + assigned IP + endpoint) and push `ObservedState.connected(...)`/`.connecting(...)`/`.reconnecting(...)` into the actor.

### F5, HIGH: IPv6 always routed, no blackhole choice

`PacketTunnelProvider.swift:277-282` unconditionally assigns `gatewayAddressIpV6` + `NEIPv6Route.default()`. Android assigns a v6 address only when `enableIpv6` is true and otherwise installs `::/0` with **no** v6 address to blackhole stray IPv6 (`WarrenTunInterfacePlan.kt:79-84,119-123`, default `enableIpv6=false` per `WarrenTunnelConfig.kt:36`). iOS assigns an address regardless, so the leak-control posture differs from Android/desktop default.
**Action:** Make the v6 address assignment conditional on a Warren IPv6 setting; keep `::/0` as a route to blackhole when disabled.

### F6, HIGH: DNS config never marshaled to tunnel

`WarrenTunnelConfig` (Swift, `WarrenQuinnAdapter.swift:31-68`) and the FFI struct (`warren_rust_runtime.h:121-156`) have **no DNS field at all**. The actor never reads `dnsSettings` from settings for the Warren path. Android carries a full `DnsConfig` (custom servers + 6 content-blocking flags, `WarrenTunnelConfig.kt:82-98`).
**Action:** Add DNS to the Swift config + (eventually) FFI, or at minimum apply DNS at the NE layer (F1). Content-blocking flags need to reach the exit (FFI/config extension).

### F7, HIGH: Routing hardcoded; no allow-LAN split

Routes are static `0.0.0.0/0` + `::/0` (`PacketTunnelProvider.swift:273,281`). No allow-LAN RFC1918/link-local exclusion exists (Android `ipv4RoutesExcluding` + re-add of exit DNS /32, `WarrenTunInterfacePlan.kt:111-123,144-168`). `bypassCidrs` is also dropped (see F9).
**Action:** Compute routes from settings (allow-LAN + bypass CIDRs) at `initialTunnelNetworkSettings` time.

### F8 / F9, HIGH: NAT-PMP and bypass CIDRs hardcoded off

`WarrenQuinnActor.start` hardcodes `natPmpEnabled: false` ("deferred (needs V9 schema)") and `bypassCidrs: []` ("settings-driven, deferred") at `WarrenQuinnActor.swift:216-217`. Grep confirms **no `natPmp`/`bypassCidr` keys anywhere in `WarrenSettings`/`WarrenTypes`**. The FFI fully supports both (`warren_rust_runtime.h:146-155`) and the Swift marshaling pins them (`WarrenQuinnAdapter.swift:397-398,428-453`), only the settings schema + UI + the `start()` wiring are missing. Android exposes both (`WarrenTunnelConfig.kt:21-29`).
**Action:** Add a settings schema version (V9) with NAT-PMP + bypass-CIDR fields, UI, and read them in `start()`.

### F10, HIGH: relay-change reconnect is a no-op

`WarrenQuinnActor.reconnect(to:reconnectReason:)` (`WarrenQuinnActor.swift:313-317`) logs `"reconnect called (C.4.3 scaffold no-op)"`. This is the path the main app drives via IPC (`TunnelManager.reconnectTunnel` → `tunnel.reconnectTunnel(to:)`, `TunnelManager.swift:214-234`) for "select new relay" / location change. The only working reconnect is the same-relay `adapter.reconnect()` triggered on network-path satisfied (`WarrenQuinnActor.swift:294-311`). Changing exit/entry relay from the UI does nothing.
**Action:** Implement `reconnect(to:)`: re-marshal the new `selectedRelays` into a `WarrenTunnelConfig`, `adapter.stop()` then `adapter.start(newConfig)`.

### F11, MED: DAITA spec is presence-only

`WarrenQuinnActor.swift:201-208` builds `WarrenDaitaSpec(machineSeedHex: "", padding: 0)` whenever DAITA is enabled, the comment notes only presence matters (exit picks the machine via SetupAck). Functionally this is a boolean and matches the documented design, but it diverges from Android which carries `padding_machine` + `normalize_packets` (`WarrenTunnelConfig.kt:67-71`). Low functional risk given the exit-selects model, but no client-side machine pinning.
**Action:** Acceptable for now; revisit if client-side machine selection is desired for parity.

### F12, MED: obfuscation indicator dead

`WarrenAppGroupEvents` publishes `obfuscationActive` read from `WarrenAppGroupKey.obfuscationActive` (`ios/WarrenVPN/View controllers/Tunnel/WarrenAppGroupEvents.swift:43,86-89`), but the producer `WarrenQuinnTunnelImplementation.broadcastEvent` (`WarrenQuinnTunnelImplementation.swift:211-225`) has no branch that writes that key, only failover + NAT-PMP keys are written. The obfuscation indicator is therefore always false.
**Action:** Write `obfuscationActive` from the connect/event path (M4.0 HTTP/3 mimicry is always-on per `TunnelObfuscationTypes.swift:77`, so it may simply be set true while connected).

### F13, MED: MTU not set on Warren path

The Warren `initialTunnelNetworkSettings` never sets `settings.mtu`. Only the unused `TunnelInterfaceSettings.asTunnelSettings()` sets `mtu = 1280` (`TunnelAdapterProtocol.swift:70`). Android clamps a config-driven MTU into `[576, 1280]` (`WarrenTunInterfacePlan.kt:131-138`). iOS leaves MTU at the system default, which may exceed the Warren QUIC floor and black-hole oversized encapsulated packets.
**Action:** Set `settings.mtu = 1280` (or config-driven, clamped) on the Warren network settings.

### F14, MED: sleep/wake correctness depends on Rust pause/resume

`onSleep`/`onWake` (`WarrenQuinnActor.swift:270-291`) call `adapter.pause()`/`resume()` → `warren_tunnel_pause/resume`. The header documents these as inbound-pump pause only (`warren_rust_runtime.h:326-347`). If the peer idle-times-out during suspension, the actor has no explicit resume-or-reconnect decision (comment hand-waves "reconnect on next packet attempt"). Not verifiable from Swift; flagged for runtime validation.
**Action:** Validate on-device that a long background suspension resumes or cleanly reconnects; add an explicit reconnect-on-wake fallback if not.

### F15 / F16, MED: event taxonomy loss

`applyEvent` collapses `connecting`/`reconnecting`/`failover` into `.initial` (`WarrenQuinnActor.swift:140-149`) (subset of F4). Separately, the Swift event bridge (`WarrenQuinnAdapter.swift:560-580`) has cases for `EventConnected/Disconnected/Reconnecting/Failover/NatPmp*` but **no case for `EventConnecting` (=7)** defined in the header (`warren_rust_runtime.h:34-42`); it hits `default: return` and is silently dropped, so the "first attempt vs recovering" distinction the FFI intends is lost.
**Action:** Add an `EventConnecting` case to the bridge and a corresponding `WarrenTunnelEvent.connecting`; map to `ObservedState.connecting(...)`.

### F17, LOW: no Warren lockdown-mode setting

iOS surfaces `includeAllNetworks` (Mullvad's lockdown-ish concept; UI under `ios/WarrenVPN/Coordinators/Settings/IncludeAllNetworks/`) but nothing feeds a Warren kill-switch. With F2 unimplemented there is no consumer anyway.
**Action:** Add a Warren lockdown setting and wire it to the F2 blocking interface.

### F18, LOW: no NAT-PMP status read on iOS

Android polls `WarrenJni.getNatPmpStatus()` (`WarrenQuinnAdapter.kt:145`) for a live mapping/idle JSON. iOS has only transient `natPmp*` events (`WarrenQuinnAdapter.swift:567-577`); no `warren_tunnel_natpmp_status` FFI exists in the header. UI can show a port on the mapped event but cannot poll current state.
**Action:** Add a status FFI or persist the last event into App Group for steady-state display.

### F19, LOW: silent start failure when wallet/relays missing

`WarrenQuinnTunnelImplementation.setUp` logs and continues if the wallet cannot be loaded (`WarrenQuinnTunnelImplementation.swift:117-127`); `WarrenQuinnActor.start` then logs+returns on missing seed/adapter/relays (`WarrenQuinnActor.swift:170-181`) without any `setErrorState`. Combined with F3/F4, a tunnel that fails to start shows no error to the user.
**Action:** Call `setErrorState(.deviceLoggedOut)`/appropriate reason on missing wallet, and a relay-selection reason on missing `selectedRelays`.

---

## Tunnel config marshaling: iOS vs Android (summary)

| Config field | Android (`WarrenTunnelConfig`/plan) | iOS Warren path | Gap |
|---|---|---|---|
| Exit pubkey + endpoint | yes | yes (`WarrenQuinnActor.swift:186-189`) | OK |
| Wallet signing seed | yes (mnemonic) | yes (seed via Keychain, `:119-122`) | OK |
| Multi-hop entry hop | yes | yes (`:190-199`) | OK |
| DAITA | machine + normalize | presence-only placeholder | F11 |
| DNS (custom + blocking) | yes | **none** | F1/F6 |
| Allow-LAN split routes | yes | **none (hardcoded 0/0)** | F7 |
| IPv6 enable/blackhole | yes (default off) | **always on** | F5 |
| MTU | clamped config | **unset** | F13 |
| NAT-PMP enable/params | yes | **hardcoded false** | F8 |
| Bypass CIDRs | yes | **hardcoded []** | F9 |
| Lockdown / kill-switch | yes (blocking FD + flap) | **none** | F2/F17 |
| Obfuscation indicator | n/a (state flag) | UI reads, never written | F12 |

## FFI surface: capabilities with no iOS equivalent

The Warren FFI (`warren_rust_runtime.h`) exposes: `warren_tunnel_start/stop/pause/resume/reconnect/status`, `set_event_callback`, `set_outbound_callback`, `inject_inbound_packet`, and wallet helpers (`generate_mnemonic`, `seed_from_mnemonic`, `derive_pubkey`, `pubkey_ss58`, `sign`). Events: Connected/Disconnected/Reconnecting/Connecting/Failover/NatPmp{Mapped,Renewed,Failed}.
Missing vs Android JNI surface:
- **NAT-PMP status read**, Android `getNatPmpStatus()` polled; no `warren_tunnel_natpmp_status` (F18).
- **Blocked-state / lockdown**, no FFI; handled host-side on Android, absent on iOS (F2).
- **`EventConnecting`**, defined in header but not consumed by Swift (F16).
- **DNS / allow-LAN / IPv6 / MTU**, these are host-side (NE layer) on iOS by design, but currently unimplemented at that layer (F1/F5/F6/F7/F13).

## Reconnection / handover / kill-switch: iOS vs Android

- **Network handover:** iOS reconnects on `NWPath` `.unsatisfied -> .satisfied` only (`WarrenQuinnActor.swift:294-311`), via same-relay `adapter.reconnect()`. Android registers a `NetworkCallback`, detects underlying-network change, and does a full teardown+reconnect with a 15s `Backoff::HANDSHAKE` grace (`WarrenQuinnAdapter.kt:320-404`). iOS is coarser (relies on iOS dropping path to unsatisfied) and has no grace-aligned re-handshake.
- **Relay-change reconnect:** broken on iOS (F10).
- **Kill-switch on drop:** absent on iOS (F2); Android fail-closed with flap detector.
- **Leak protection (the Android P0 set: IPv6 blackhole, in-tunnel DNS, kill-switch blackhole-TUN):** iOS has **none** of the three (F5, F1, F2).

## Blocked-state path trace (how far from working)

`setErrorState` no-op (F3) → actor never enters `ObservedState.error` (F4) → IPC observed-state never carries `blockedState` → `TunnelManager.swift:873` `blockedState` always nil → `TunnelState.error(...)` (`TunnelState.swift:90`) never produced → no UI. Everything **downstream** of the actor is implemented (mapper, IPC, TunnelState, UI). The single missing link is the actor producing `ObservedState.error`. Same story for connected state (F4): downstream UI ready, actor never emits it.

## Wallet / account / subscription / support-report / relay-list (parity check)

These are **present and wired** on iOS, no blocker:
- **Wallet:** `WarrenWallet.swift` (FFI: generate/seed/pubkey/ss58/sign), `WarrenWalletKeychain.swift` (Keychain, shared with extension), `WarrenWalletInteractor.swift`, `WarrenWalletIdentityView.swift`, `WarrenWalletEraseViewController.swift` (create/restore/erase).
- **Voucher redemption:** `ProfileVoucherCoordinator.swift`, `RedeemVoucherViewController.swift`, `RedeemVoucherInteractor.swift`, `TunnelManager/RedeemVoucherOperation.swift`.
- **Account:** `AccountViewController.swift`, `TunnelManager/SetAccountOperation.swift` (create/login/delete/device).
- **Subscription/payments:** StoreKit FFI present (`mullvad_ios_init_storekit_payment` / `check_storekit_payment`, header `:1161-1189`) + `ios/WarrenVPN/StorePaymentManager`.
- **Support report:** `mullvad_ios_send_problem_report` (header `:1068-1071`) + `RustProblemReportRequest.swift`.
- **Relay list:** `mullvad_ios_get_relays` (header `:798-801`) + `RelayCacheTracker`.

Minor note: these run through the retained Mullvad REST/account/device proxies; functional parity is fine. The gaps are concentrated entirely in the **tunnel data/control plane** (F1-F19), not the account/wallet plane.
