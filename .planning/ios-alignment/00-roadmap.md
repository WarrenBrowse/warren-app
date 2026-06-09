# iOS alignment roadmap (vs Android + desktop Electron)

Consolidated from 4 parallel audits (2026-06-09/10):
- [01-functional.md](01-functional.md) — tunnel data/control plane completeness
- [02-security.md](02-security.md) — seed/keychain/FFI/leak protection
- [03-uiux.md](03-uiux.md) — flow + branding parity
- [04-idiomatic.md](04-idiomatic.md) — dead code, naming, Swift idioms, policy

## Validation story

- iOS DOES have a real test target (`PacketTunnelCoreTests`) + simulator schemes → **true local TDD is possible** (like Android gradle), once the build is green.
- Gate command (once unblocked): `xcodebuild build -scheme PacketTunnelCore -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -configuration Debug`.
- Per global rule: never run device builds that need signing; simulator builds are fine. No Flutter here (this is Xcode/Swift + a Rust FFI crate `warren-ios`).

## #0 BLOCKER — iOS does not build + is half-migrated (PRECONDITION FOR EVERYTHING)

Root cause: commit `dbaa5fa232` (refactor(device): remove Device model + WireGuard keys from warren-app daemon, collapse to wallet-identity-only login state) removed `mullvad-api/src/device.rs` (`DevicesProxy`, 221 lines) and migrated daemon/desktop/Android to a **wallet-identity-only** model. **iOS was not migrated** and still uses the Mullvad device/account API:
- Rust FFI `warren-ios/src/api_client/device.rs` imports the now-deleted `mullvad_api::DevicesProxy` → `E0432` → the whole iOS build (any target depending on `WarrenRustRuntime`) is RED.
- Swift side still calls the device FFI (`ios/WarrenREST/MullvadAPI/MullvadApiRequestFactory.swift`, header `warren_rust_runtime.h`) and routes `.loggedOut` to the legacy account-number `LoginViewController`. This login would not work against Warren's wallet backend anyway.

Two strategies (DECISION REQUIRED — see chat):
- **(A) Bridge to green now**: make `warren-ios/device.rs` self-contained (vendor a minimal `DevicesProxy` into the warren-ios crate, do NOT re-pollute the cleaned `mullvad-api`). Smallest path to a green build + validatable tree; leaves the stale Mullvad login in place to be migrated later. Hours.
- **(C) Start the real wallet-login migration**: make the wallet (Create / Restore-by-phrase) the iOS login like desktop/Android, delete the Mullvad device/account FFI+Swift+UI (which removes the broken `device.rs` entirely). Correct end-state and the user's actual goal; large UI+FFI effort, needs device testing. Days.
- (B rejected: re-adding `device.rs` to `mullvad-api` re-pollutes the deliberately de-deviced daemon crate.)

## Phase 1 — tunnel data/control plane (BLOCKER, security-critical, pure-Swift)

Concentrated in `PacketTunnelProvider.initialTunnelNetworkSettings()` + `WarrenQuinnActor`. All fail-closed (worse case = no traffic, never a leak). Validatable by simulator build + PacketTunnelCore tests.
1. **In-tunnel DNS** — set `NEDNSSettings(servers:["10.66.0.1"])` + `matchDomains=[""]` (Android `EXIT_DNS_RESOLVER`; default v4 route already covers it). Fixes the DNS leak. (`PacketTunnelProvider.swift:263`)
2. **IPv6 blackhole when v6 disabled** — stop unconditionally assigning a v6 addr+default route; blackhole instead (Android default-off). (`PacketTunnelProvider.swift:277`)
3. **Kill switch / fail-closed** — assess `includeAllNetworks` / blackhole-on-drop for NEPacketTunnelProvider; keep traffic captured on tunnel drop/crash/sleep (Android blackhole-TUN). (`PacketTunnelProvider`, `WarrenQuinnActor.onSleep`)
4. **Wire `setErrorState`** → emit `ObservedState.error(BlockingData)` (chain downstream is already built: BlockedStateErrorMapper → IPC → UI). Unblocks all error UI incl. device-check failures. (`WarrenQuinnActor.swift:326`)
5. **Report real `.connected`** — `applyEvent(.connected/.reconnecting/.failover)` must build a real `ObservedConnectionState` (selectedRelay+IP) instead of `.initial`, so the UI shows Connected. Needs the connection details threaded from `WarrenQuinnTunnelImplementation` stats up to the actor. (`WarrenQuinnActor.swift:130`)
6. **`reconnect(to:)` relay-change** — re-marshal + restart (currently no-op). (`WarrenQuinnActor.swift:313`)

## Phase 2 — config marshaling (HIGH, needs FFI/Rust changes)

The Swift `WarrenTunnelConfig` + FFI struct lacks fields Android wires:
7. DNS servers + content-blocking flags into the config/FFI (custom DNS dropped today).
8. NAT-PMP toggle (hardcoded false) + V9 settings schema + App-Group `obfuscationActive`/port producer.
9. Bypass / split-tunnel CIDRs (hardcoded []).
10. Allow-LAN route split; MTU on the Warren path; `EventConnecting` bridged.

## Phase 3 — UI/UX parity (HIGH/MED) — overlaps #0(C)

11. Wallet-as-login (Create / Restore-by-phrase), drop account-number login. (ties to #0 strategy C)
12. Logout = wipe wallet + history (today only `unsetAccount()`).
13. Mnemonic UX: show directly + copy button (today blurred hold-to-reveal, no copy). Matches Warren doctrine.
14. Feature chips: always-on QUIC + collapsed DAITA_MULTIHOP (Android parity).
15. Account view → wallet/SS58 identity (not Mullvad account number).
16. Onboarding: persist multihop/DAITA toggles + mandatory backup gate.
17. `allow-external-DNS` advanced toggle (desktop/CLI have it).
18. NAT-PMP port display (dead scaffold today).

## Phase 4 — idiomatic / cleanup (HIGH-policy / MED / LOW, mostly build-light)

19. Typography policy: em-dash `—` in 4 `.rb` build scripts (5x) + en-dash `–` in `WarrenWalletIdentityView.swift:18`. (HARD rule)
20. `InfoPlist.strings` (16 locales) still "Mullvad VPN" / "MullvadVPN" (home-screen + Settings app name). en/Base + main xcstrings already clean.
21. Delete dead Mullvad-era files dropped from targets: 11 `PacketTunnelActor*.swift`, PostQuantum/ephemeral-peer, `TunnelPinger.swift`; verify zero refs first. `TunnelMonitor`/`WgStats` still compiled-but-unused → care.
22. Stale `PBXGroup path = MullvadVPN` in `project.pbxproj`.
23. False header comment `WarrenWalletKeychain.swift:8` ("NOT yet wired"); `WarrenMnemonicDisplayView` same. Scaffold/`C.4.3 TODO` step-narration (no-narration rule).
24. Design-system `.mullvad*` API (fonts/colors, 200+ refs) → rename-only, load-bearing, separate tested refactor.

## Security items folded into the phases

- DNS leak + kill switch = Phase 1.1/1.3 (also 02-security HIGH).
- Keychain: wallet seed has no access group yet the extension reads it (02-security HIGH); `WhenUnlockedThisDeviceOnly` blocks background reads (MED); biometric gate not bound to the item via `SecAccessControl` (MED). → a focused Keychain pass (Phase 1.5-adjacent).
- FFI callback UAF window on `stop()` + `deinit` retain leak (MED); seed not zeroized (MED) → FFI hardening pass.
