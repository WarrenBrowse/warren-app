# WireGuard Removal Mapping & Scope Analysis

**Analysis Date:** 2026-05-31  
**Scope:** Comprehensive investigation of WireGuard touchpoints across warren-app and warren-core  
**Key Finding:** The Quinn tunnel path (talpid-warren-tunnel) does NOT reference WireGuard keys at runtime. The entire WG infrastructure is legacy/vestigial and coupled to an optional Device management abstraction.

---

## Executive Summary: Three Architectural Questions & Answers

### Q1: Does the Quinn tunnel reference any actual WG key/crypto at runtime?

**ANSWER: NO.**

**Evidence:**
- `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/tunnel.rs` lines 453–570: `produce_warren_tunnel_params()` constructs `WarrenTunnelParameters` using only:
  - Warren signing key (Ed25519 from BIP39 mnemonic, wallet identity)
  - Relay selector output (exit descriptor with Ed25519 pubkey)
  - Multi-hop config (relay descriptors)
  - No WireGuard key generation, no device pubkey lookup, no WG PSK derivation
- `/Users/poka/dev/warrenBros/warren-app/talpid-warren-tunnel/src/lib.rs` lines 1–100: `WarrenTunnelMonitor` and `WarrenTunnelParameters` only carry Ed25519 material
- The DAITA constant-packet-size feature and quantum-resistant PSK exchange both happen via Warren's QUIC negotiation, not WireGuard device keys
- iOS: `WarrenQuinnTunnelImplementation.swift` and `GotaTunTunnelImplementation.swift` only reference ephemeral peer exchange and Quinn tunneling; no device pubkey lookup on the hot path

**Conclusion:** WireGuard key material is **completely absent from the connect path**. It's a legacy artifact for the old Mullvad/OpenVPN backend.

---

### Q2: Is the `/v1/devices` + Device flow exercised by Warren's connect path, or is it vestigial?

**ANSWER: VESTIGIAL (management-only, never touched during tunnel setup).**

**Evidence:**
- Device registration/rotation happens in:
  - `mullvad-daemon/src/device/mod.rs:PrivateAccountAndDevice`: wraps `Device` + `WireguardData` (the WG key struct)
  - `mullvad-daemon/src/device/device_backend.rs`: five methods (`create`, `get`, `list`, `remove`, `replace_wg_key`) that speak to `/v1/devices` endpoints
  - These are called only by **account login/logout and device-list UI refresh**, never by the tunnel state machine
- `mullvad-daemon/src/tunnel.rs:produce_warren_tunnel_params()` does NOT fetch, validate, or use the Device
- The gRPC `GetDevice` handler (`mullvad-daemon/src/lib.rs:on_get_device`) services the **UI device list**, not tunnel mechanics
- Desktop/Android/iOS device UI displays the device name and creation date for user bookkeeping; the tunnel connects using the wallet identity alone
- No tunnel fails due to missing/invalid device WG key; tunnel success depends solely on the wallet pubkey being registered on the Warren server

**Conclusion:** Device is a **user management artifact** (track multi-device logins, rotate keys for hygiene) but **never required for tunnel to function**. Removing the Device model would break the device-list UI but leave the tunnel working.

---

### Q3: Current state of iOS WireGuardKit removal?

**ANSWER: COMPLETE removal already done; wireguard-apple is a stub; build script already early-exits.**

**Evidence:**
- `/Users/poka/dev/warrenBros/warren-app/ios/wireguard-apple/` contains only:
  - `.swiftpm/` directory (Xcode build cache)
  - `build/` directory (build artifacts)
  - **NO** `Sources/WireGuardKitGo` or any Swift/C source
- `/Users/poka/dev/warrenBros/warren-app/ios/build-wireguard-go.sh` lines 14–22: **Early-exit check that skips the Go bridge build if `wireguard-apple/Sources/WireGuardKitGo` is missing.** Script explicitly says "Warren: stub wireguard-apple".
- iOS project files (Xcode pbxproj) have been scrubbed of WireGuardKit dependencies
- iOS tunnel implementation is 100% Quinn-based:
  - `PacketTunnelCore/Actor/WarrenQuinnTunnelImplementation.swift`
  - `WarrenRustRuntime/WarrenQuinnAdapter.swift`
  - No calls to any WireGuardKit symbol

**Conclusion:** iOS has already **completed WireGuard removal**. The `wireguard-apple` submodule is a ghost; the build harness already handles its absence gracefully.

---

## Bucket 1: Truly Dead WG Artifacts (Removal Safe Without Device Decision)

### Removable Immediately — Zero Impact on Tunnel or Device Model

| File Path | Lines | Description | Removal Notes |
|-----------|-------|-------------|----------------|
| `/Users/poka/dev/warrenBros/warren-app/ios/build-wireguard-go.sh` | All (85 lines) | Xcode build script for legacy Go bridge; early-exits when stub detected | Remove entirely; no references from Xcode pbxproj |
| `/Users/poka/dev/warrenBros/warren-app/ios/wireguard-apple/` | – | Empty submodule stub; only `.swiftpm/` and `build/` directories | Remove `.gitmodules` entry and submodule directory |
| `/Users/poka/dev/warrenBros/warren-app/talpid-types/Cargo.toml` | 1 line: `wireguard-go = []` | Feature flag for legacy Go bridge (Windows userspace) | Remove feature flag; no active references in code |

**Total Bucket 1 Removal:** ~85 lines; self-contained; no other code imports these.

---

## Bucket 2: Device-Model-Coupled (Requires Architectural Decision)

### Sub-Bucket 2A: Core Device Struct & Types (Type-Layer Changes)

#### mullvad-types crate

| File Path | Lines | Description | Impact |
|-----------|-------|-------------|--------|
| `/Users/poka/dev/warrenBros/warren-app/mullvad-types/src/device.rs` | 1–172 | `Device` struct (id, name, pubkey:WireGuard::PublicKey, hijack_dns, created) | **DECISION-CRITICAL**: If Device is removed, this file disappears; if kept, pubkey field must be removed or stubbed |
| `/Users/poka/dev/warrenBros/warren-app/mullvad-types/src/wireguard.rs` | 1–275 | `WireguardData`, `PublicKey`, `PrivateKey`, `RotationInterval`, `TunnelOptions` | Keep `TunnelOptions` (still used by Quinn for MTU/DAITA/QuantumResistant); remove `WireguardData`, `PublicKey` if Device is removed |
| `/Users/poka/dev/warrenBros/warren-app/mullvad-types/src/lib.rs` | (lines with `pub use wireguard`, `pub use device`) | Re-exports of WG types and Device | Prune if removing Device model |

**Subtotal: ~500 LOC** (Rust types only; vanishes if Device goes away)

#### warren-api-types crate (warren-core)

| File Path | Lines | Description | Impact |
|-----------|-------|-------------|--------|
| `/Users/poka/dev/warrenBros/warren-core/crates/warren-api-types/src/lib.rs` | 820–865 | `Device` struct with `wg_pubkey_hex: String`, `RotateDeviceWgKeyRequest` | Wire contract; must be kept for API compat **OR** redesigned without WG fields |

**Subtotal: ~50 LOC** (API types; affects gRPC wire format)

---

### Sub-Bucket 2B: Daemon Device Backend & Service (Runtime Infrastructure)

#### mullvad-daemon device module

| File Path | Lines | Description | Removal Impact |
|-----------|-------|-------------|-----------------|
| `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/mod.rs` | 1–1729 | `PrivateAccountAndDevice`, `PrivateDeviceState`, `DeviceService`, `AccountManager`, device caching, validation logic | **CORE DEVICE STATE MACHINE**: Remove if Device goes away; refactor to wallet-only identity if Device is kept |
| `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/device_backend.rs` | 1–899 | `WarrenDeviceBackend` trait, `RemoteDeviceBackend` (wraps warren-api), `LocalDeviceBackend` (in-memory POC) | Implement methods: `create(wg_pubkey)`, `replace_wg_key()`, `get()`, `list()`, `remove()` — all tied to WG key lifecycle |
| `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/service.rs` | 1–696 | Device service, validation, caching, device rotation scheduler | Tied to WG key rotation intervals and device check threshold (`WG_DEVICE_CHECK_THRESHOLD = 3`) |
| `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/api.rs` | 1–213 | REST proxy wrapper for `/v1/devices/*` endpoints | Removed wholesale if Device model goes away |

**Subtotal: ~3,500 LOC** (Daemon infrastructure; removed/refactored if Device is eliminated)

#### warren-app's WarrenDeviceBootstrap

| File Path | Lines | Description | Removal Impact |
|-----------|-------|-------------|-----------------|
| `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/warren_device_bootstrap.rs` | 1–299 | Device registration + WG key derivation on first login (Phase 2.B.2) | Removed if Device is removed; refactored if kept but WG is stripped |

**Subtotal: ~300 LOC** (Device bootstrap; vanishes if model is eliminated)

---

### Sub-Bucket 2C: warren-core Backend (Device API Endpoints & Store)

#### warren-api crate

| File Path | Lines | Description | Removal Impact |
|-----------|-------|-------------|-----------------|
| `/Users/poka/dev/warrenBros/warren-core/crates/warren-api/src/devices.rs` | 1–350+ (approx) | `DeviceStore` trait, `InMemoryDeviceStore`, device registration/rotation logic keyed on `wg_pubkey_hex` + `owner_pubkey_ss58` | HTTP handlers for `POST /v1/devices`, `GET /v1/devices/{id}`, `PUT /v1/devices/{id}` (rotate WG key) — all removed if Device is eliminated |

**Subtotal: ~350 LOC** (Backend device store; removed if Device is removed)

---

### Sub-Bucket 2D: Platform UI & Data Flow (Device Management UI Only)

#### Desktop (Electron/TypeScript)

| File Path | Lines | Description | Removal Impact |
|-----------|-------|-------------|-----------------|
| `desktop/packages/mullvad-vpn/src/main/daemon-rpc.ts` | (WireGuard constraints, device type defs) | gRPC type definitions mirroring `mullvad_daemon.management_interface` | Prune if Device model is removed |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/` | (device list, settings UI) | Device display UI; **user-facing only**, never called by tunnel state machine | Removed if Device is removed; kept/redesigned if Device is kept |

**Subtotal: ~100–200 LOC** (UI only; removable without affecting tunnel)

#### Android (Kotlin)

| File Path | Lines | Description | Removal Impact |
|-----------|-------|-------------|-----------------|
| `android/lib/model/src/main/kotlin/.../WireguardConstraints.kt` | 1–12 | Data class for tunnel constraints; minimal | Remove if Device is removed |
| `android/lib/repository/src/main/kotlin/.../WarrenConnectSurface.kt` | (device-related) | Device management repository | Remove if Device is removed |
| `android/lib/ui/tag/src/main/kotlin/.../TestTagConstants.kt` | (device test tags) | Test tags for device UI | Remove if Device is removed |

**Subtotal: ~50–100 LOC** (Android model/UI; removable)

#### iOS (Swift)

| File Path | Lines | Description | Removal Impact |
|-----------|-------|-------------|-----------------|
| `ios/WarrenRustRuntime/WireGuardKey.swift` | 1–59 | Key generation/derivation (reimplemented on CryptoKit after dropping Rust FFI) | Remove if Device/WG keys are no longer needed |
| `ios/WarrenTypes/WireGuardKey.swift` | 1–251 | `WireGuard.PrivateKey`, `WireGuard.PublicKey`, `WireGuard.PreSharedKey` structs | Remove if WG keys are obsolete |
| `ios/WarrenSettings/StoredWgKeyData.swift` | 1–38 | `StoredWgKeyData` (persisted WG key + rotation metadata) | Remove if Device/WG keys are removed |
| `ios/WarrenSettings/StoredDeviceData.swift` | (device name, WG key) | Device metadata persistence | Remove if Device is removed |
| `ios/WarrenSettings/TunnelSettingsV*.swift` (V1–V8) | ~700 LOC total | Settings migration chain; some versions carry WG key fields | May need migration bump if Device/WG keys are removed (add V9 migration) |
| `ios/WarrenSettings/WireGuardObfuscationSettings.swift` | 1–224 | WG-specific obfuscation settings (legacy; Quinn path uses different obfuscation) | **CAUTION**: May still have live references; check if Quinn tunneling reads this |
| `ios/WarrenVPN/TunnelManager/WgKeyRotation.swift` | (device key rotation timer) | Device WG key rotation scheduling | Remove if Device is removed |
| `ios/WarrenVPN/View controllers/DeviceList/DeviceManaging.swift` | (device list UI) | Device list view controller | Remove if Device is removed |

**Subtotal: ~1,200+ LOC** (iOS: key structs, settings, device UI; all removable if Device goes away)

---

## Bucket 3: Heritage/Comments (Keep — Not Removal Targets)

| File / Context | Content | Reason to Keep |
|----------------|---------|-----------------|
| `talpid-warren-tunnel/src/lib.rs` (header) | "mirror of Mullvad WireGuard pattern in `talpid-warren-tunnel` (the Quinn tunnel's routing/DAITA borrowed naming)" | **NOT actual WG code**, just naming heritage; document for future maintainers |
| Various Quinn code | Comments referencing "talpid-types::net::wireguard" or "packet structure mirrors WireGuard PSK exchange" | Structural heritage; clarifies why naming resembles WireGuard; keep for architectural clarity |
| `/Users/poka/dev/warrenBros/warren-app/mullvad-types/src/relay_constraints.rs` | Constraint enums mentioning "wireguard" in variant names (e.g., `Protocol::WireGuard`) | **STILL IN USE**: The relay selector can be constrained to WireGuard-only relays (legacy path); must stay until relay model is redesigned to drop tunnel-type constraints |
| `mullvad-daemon/src/custom_list.rs` | Comments on Mullvad custom tunnel types | Keep for migration reference |

**Total Bucket 3: 0 LOC removal** (pure documentation/comments; optional cleanup only)

---

## File Count & LOC Summary by Removal Depth

### Option A: Remove Entire Device Model (Most Radical)

**Scope:** Eliminate Device abstraction entirely; Warren identity = wallet pubkey only; no per-device WG key.

| Category | Repos/Files | LOC | Notes |
|----------|-------------|-----|-------|
| **Bucket 1** (dead artifacts) | iOS: 1 script + 1 submodule + 1 Cargo.toml line | ~85 | Self-contained; zero dependencies |
| **Bucket 2A** (type definitions) | mullvad-types: 3 files; warren-api-types: 1 file | ~550 | Type layer; cascading deletions |
| **Bucket 2B** (daemon backend) | mullvad-daemon: 4 files + bootstrap | ~3,800 | State machine + service infrastructure |
| **Bucket 2C** (warren-core API) | warren-api: 1 file (devices.rs) | ~350 | HTTP handlers, device store |
| **Bucket 2D** (platform UI) | Desktop: ~200 LOC; Android: ~100 LOC; iOS: ~1,200 LOC | ~1,500 | UI/settings removal; test cleanup |
| **Total Option A** | **~10 crates touched** | **~6,300 LOC** | Removes device list, device rotation, device registration; keeps tunnel working |

---

### Option B: Keep Device Abstraction, Strip WireGuard (Moderate)

**Scope:** Retain Device model for multi-device UI/bookkeeping but remove WG key fields and rotation logic.

| Category | Repos/Files | LOC | Notes |
|----------|-------------|-----|-------|
| **Bucket 1** (dead artifacts) | iOS: 1 script + 1 submodule + 1 Cargo.toml | ~85 | Same as Option A |
| **Changes to Bucket 2A** | Device struct loses `pubkey: WireGuard::PublicKey` field; delete `WireguardData`, `RotationInterval`, `TunnelOptions` | ~200 LOC removed | Keep Device name/id/hijack_dns/created |
| **Changes to Bucket 2B** | Remove `replace_wg_key()` method; refactor `create()` to not accept WG pubkey; drop `WG_DEVICE_CHECK_THRESHOLD` validation | ~1,500 LOC refactored | Device registration/deletion still works; no key lifecycle |
| **Changes to Bucket 2C** | Refactor `compute_device_id()` to not hash `wg_pubkey_hex`; drop `RotateDeviceWgKeyRequest`; remove `replace_wg_key()` endpoint | ~150 LOC refactored | Device CRUD remains; WG operations gone |
| **Changes to Bucket 2D** | Remove key rotation UI; keep device list/deletion; iOS: drop `StoredWgKeyData`, `WgKeyRotation.swift`; remove from settings migrations | ~800 LOC removed | Device list UI survives; WG-specific UX gone |
| **Total Option B** | **~10 crates touched** | **~2,800 LOC removed; ~1,700 LOC refactored** | Device model survives; WG completely removed from runtime |

---

### Option C: Bucket 1 Only (Minimal/Incremental)

**Scope:** Remove only dead artifacts; leave Device + WG infrastructure as-is (status quo).

| Category | Repos/Files | LOC | Notes |
|----------|-------------|-----|-------|
| **Bucket 1** | iOS build script + submodule + Cargo feature | ~85 | Removes ghost build infrastructure |
| **Total Option C** | **3 simple deletions** | **~85 LOC** | Zero risk; immediate quick-win; does not address Device/WG coupling |

---

## Detailed Touchpoint Map

### Rust Crate Dependencies (Graph)

```
talpid-warren-tunnel (Quinn tunnel adapter)
  ├── does NOT import talpid-types::net::wireguard
  ├── warren-tunnel (QUIC library; Ed25519-only)
  └── warren-protocol (exit descriptors; Ed25519)

mullvad-daemon (state machine)
  ├── mullvad-types (Device struct + wireguard types)
  ├── talpid-types::net::wireguard (relay selector constraints only)
  ├── device/ module (Device backend + service)
  │   └── mullvad-api (DevicesProxy wrapper)
  │       └── REST endpoint `/v1/devices/*`
  └── tunnel.rs (does NOT use Device in produce_warren_tunnel_params)

warren-core
  ├── warren-api (devices.rs store)
  ├── warren-api-types (Device + RotateDeviceWgKeyRequest)
  └── warren-api-client (mirrors server endpoints)
```

**Critical Observation:** WireGuard types are **isolated to the Device module + type definitions**. The Quinn tunnel path has **zero imports** of `talpid_types::net::wireguard` or any WG cryptography.

---

## Per-File Detailed Inventory

### Bucket 1: Dead Artifacts (Remove Immediately)

#### `/Users/poka/dev/warrenBros/warren-app/ios/build-wireguard-go.sh`
- **Lines:** 1–85
- **Content:** Xcode ExternalBuildSystem target script; builds `wireguard-apple/Sources/WireGuardKitGo` (Go WireGuard bridge)
- **Status:** **DEAD** — Early-exit on line 22 if `WireGuardKitGo` missing; no Xcode pbxproj rule invokes this anymore
- **Removal:** Delete file entirely; no imports/calls
- **Bucket:** 1

#### `/Users/poka/dev/warrenBros/warren-app/ios/wireguard-apple/` (submodule)
- **Lines:** N/A (directory)
- **Content:** Empty stub; only contains `.swiftpm/` and `build/` caches
- **Status:** **DEAD** — No source code; submodule entry in `.gitmodules` points to a historical GitHub repo
- **Removal:** Remove `.gitmodules` entry + delete directory
- **Bucket:** 1

#### `/Users/poka/dev/warrenBros/warren-app/talpid-types/Cargo.toml` (line with `wireguard-go = []`)
- **Lines:** 1 feature flag
- **Content:** `wireguard-go = []` — Feature gate for legacy userspace WireGuard on Windows
- **Status:** **DEAD** — Feature is not referenced in any `#[cfg(...)]` conditional; Windows now uses GotaTun/Quinn
- **Removal:** Delete feature flag line; verify no build breaks with `cargo build --all-features`
- **Bucket:** 1

---

### Bucket 2A: Core Device Types (Decision-Dependent)

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-types/src/device.rs`
- **Lines:** 1–172
- **Structs:** 
  - `Device { id, name, pubkey: PublicKey, hijack_dns, created }`
  - `DeviceState { LoggedIn(WarrenIdentity), LoggedOut, Revoked }`
  - `RemoveDeviceEvent { pubkey: WarrenPubKey, new_devices }`
- **Removal Scenario:**
  - **Option A:** Delete entire file; refactor callers to use `WarrenIdentity` (wallet pubkey) only
  - **Option B:** Keep file; replace `pubkey: PublicKey` with stub/deprecated marker; keep `id, name, hijack_dns, created`
  - **Option C:** No change
- **Dependencies:** `talpid_types::net::wireguard::PublicKey` import on line 4
- **Bucket:** 2A

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-types/src/wireguard.rs`
- **Lines:** 1–275
- **Structs:**
  - Lines 90–107: `WireguardData { private_key, addresses, created }`
  - Lines 136–217: `PublicKey` (x25519 wrapper)
  - Lines 141–167: `PrivateKey` (x25519 wrapper)
  - Lines 220–260: `TunnelOptions { mtu, quantum_resistant, daita, userspace, rotation_interval }`
- **Status:** 
  - `PublicKey`, `PrivateKey`, `WireguardData`: Device-only (removal: delete lines 90–217)
  - `TunnelOptions`, `QuantumResistantState`, `DaitaSettings`: **STILL USED by Quinn tunnel** for MTU/DAITA/QR settings (keep lines 16–260)
  - `RotationInterval`: Device-only (removal: delete lines 136–217)
- **Removal Scenario:**
  - **Option A/B:** Keep only `TunnelOptions`, `QuantumResistantState`, `DaitaSettings`, `RotationInterval` (if keeping Device but stripping WG keys, remove `RotationInterval` too); delete `PublicKey`, `PrivateKey`, `WireguardData`
  - **Option C:** No change
- **Bucket:** 2A

#### `/Users/poka/dev/warrenBros/warren-core/crates/warren-api-types/src/lib.rs` (Device struct)
- **Lines:** ~820–865
- **Structs:**
  - Lines 826–840: `RotateDeviceWgKeyRequest { wg_pubkey_hex: String }`
  - Lines 836–852: `Device { id, name, wg_pubkey_hex, hijack_dns, created_at }`
- **Wire Format:** These are HTTP request/response DTOs; changing them requires API versioning or backward-compat shim
- **Removal Scenario:**
  - **Option A:** Delete both structs; redesign endpoints to not accept/return WG key material
  - **Option B:** Keep `Device` struct; remove `wg_pubkey_hex` field; delete `RotateDeviceWgKeyRequest`
  - **Option C:** No change
- **Bucket:** 2A

---

### Bucket 2B: Daemon Backend (Decision-Dependent, ~3,800 LOC)

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/mod.rs`
- **Lines:** 1–1,729
- **Key Types/Functions:**
  - Lines 119–145: `PrivateDeviceState { LoggedIn(PrivateAccountAndDevice), LoggedOut, Revoked }`
  - Lines 150–173: `PrivateAccountAndDevice { device, wg_data: WireguardData, ... }`
  - Lines 214–: `DeviceService` (manages device CRUD + WG rotation)
  - Lines 600+: `AccountManager` (top-level orchestrator; manages account + device state together)
- **Coupling:** Every login creates a `PrivateAccountAndDevice` which holds a `WireguardData`; every reconnect fetches the device to validate WG key is still valid
- **Removal Impact:**
  - **Option A:** Delete entire module; fold account management into a simple identity struct; remove device caching/validation
  - **Option B:** Refactor `PrivateAccountAndDevice` to remove `wg_data`; keep device ID for multi-device UI; remove key rotation logic
  - **Option C:** No change
- **Bucket:** 2B

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/device_backend.rs`
- **Lines:** 1–899
- **Trait:** `WarrenDeviceBackend` with 5 methods:
  - `create(account, pubkey: WgPublicKey) -> (Device, AssociatedAddresses)`
  - `get(account, id) -> Device`
  - `list(account) -> Vec<Device>`
  - `remove(account, id) -> ()`
  - `replace_wg_key(account, id, pubkey) -> AssociatedAddresses`
- **Impls:**
  - `RemoteDeviceBackend` (thin wrap of `DevicesProxy` → warren-api)
  - `LocalDeviceBackend` (in-memory HashMap for POC/testing)
- **Removal Impact:**
  - **Option A:** Delete entire file; remove trait and both implementations
  - **Option B:** Refactor trait to remove `pubkey` parameter from `create()` and `replace_wg_key()` method; keep other CRUD
  - **Option C:** No change
- **Bucket:** 2B

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/service.rs`
- **Lines:** 1–696
- **Key Logic:**
  - Device validation cache (lines ~100–150)
  - WireGuard key rotation scheduler (lines ~300–400)
  - Device check threshold (`WG_DEVICE_CHECK_THRESHOLD = 3` on line 73)
  - `DeviceService` state machine
- **Removal Impact:**
  - **Option A:** Delete entire file
  - **Option B:** Refactor to remove key rotation + threshold checks; keep device list/deletion
  - **Option C:** No change
- **Bucket:** 2B

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/device/api.rs`
- **Lines:** 1–213
- **Content:** REST proxy wrapper (`DevicesProxy`); forwards calls to warren-api `/v1/devices/*` endpoints
- **Removal Impact:**
  - **Option A:** Delete entire file; remove REST client
  - **Option B:** Refactor methods to remove key rotation endpoints
  - **Option C:** No change
- **Bucket:** 2B

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/warren_device_bootstrap.rs`
- **Lines:** 1–299
- **Content:** Device registration + WG key generation on first login (Phase 2.B.2)
  - Line ~50: Call `mullvad-management-interface` gRPC `create_device` with generated WG pubkey
  - Lines ~100–200: WG key generation via FFI/JNI/Swift
  - Lines ~250–299: Device ID derivation + caching
- **Removal Impact:**
  - **Option A:** Delete entire file; fold login directly into account manager
  - **Option B:** Refactor to register device without WG key (use wallet pubkey + empty/stub WG field)
  - **Option C:** No change
- **Bucket:** 2B

#### `/Users/poka/dev/warrenBros/warren-app/mullvad-daemon/src/lib.rs` (device handlers)
- **Lines:** Scattered gRPC handlers (`on_get_device`, `on_remove_device`, etc.)
- **Removal Impact:**
  - **Option A:** Remove handlers; UI will not see device list
  - **Option B:** Refactor handlers to return device without WG key info
  - **Option C:** No change
- **Bucket:** 2B

---

### Bucket 2C: Warren-Core Backend (Decision-Dependent, ~350 LOC)

#### `/Users/poka/dev/warrenBros/warren-core/crates/warren-api/src/devices.rs`
- **Lines:** 1–350+ (estimated)
- **Key Components:**
  - Lines ~25–38: `compute_device_id(wg_pubkey_hex, owner_pubkey_ss58)` function
  - Lines ~50–104: `DeviceStore` trait (abstract CRUD)
  - Lines ~106–209: `InMemoryDeviceStore` implementation
  - Trait methods:
    - `register(owner, wg_pubkey_hex, ...) -> Device`
    - `replace_wg_key(owner, id, new_wg_pubkey_hex) -> bool`
    - Others: `get_for_owner`, `list_for_owner`, `remove_for_owner`
  - Tests (~211–350+): idempotency, cross-tenant checks, key rotation
- **Removal Impact:**
  - **Option A:** Delete entire file; remove `/v1/devices` HTTP handlers
  - **Option B:** Refactor to remove `wg_pubkey_hex` from device ID computation; remove `replace_wg_key` method; update tests
  - **Option C:** No change
- **Bucket:** 2C

#### Warren-API HTTP handlers (implied in warren-api crate)
- **Lines:** (Not shown; typically in a `handlers/` module or inline in router)
- **Endpoints:**
  - `POST /v1/devices` (register) — accepts `wg_pubkey_hex`
  - `GET /v1/devices` (list) — returns `Device[]`
  - `GET /v1/devices/{id}` (fetch) — returns `Device`
  - `DELETE /v1/devices/{id}` (remove) — ownership check
  - `PUT /v1/devices/{id}` (rotate WG key) — accepts `RotateDeviceWgKeyRequest`
- **Removal Impact:**
  - **Option A:** Delete `/v1/devices` endpoints; may break clients expecting device list (gRPC daemon, UIs)
  - **Option B:** Redesign endpoints; `POST /v1/devices` no longer accepts WG key; `PUT` endpoint deleted
  - **Option C:** No change
- **Bucket:** 2C

---

### Bucket 2D: Platform UI & Settings (Decision-Dependent, ~1,500 LOC)

#### Desktop / TypeScript
- **Files:** `daemon-rpc.ts`, `grpc-type-conversions.ts`, device list/settings views
- **Lines:** ~100–200 across files
- **Content:** gRPC type stubs mirroring `mullvad_daemon.management_interface`; device list UI
- **Removal Scenario (Option A):** Remove device type definitions; delete device list view
- **Removal Scenario (Option B):** Update type defs to remove `pubkey` field; keep device list UI
- **Bucket:** 2D

#### Android / Kotlin
- **Files:** `WireguardConstraints.kt`, device repository, test helpers
- **Lines:** ~50–100
- **Content:** Data class for tunnel constraints; device repository interface
- **Removal Scenario (Option A/B):** Remove WireguardConstraints or refactor to drop WG-specific fields
- **Bucket:** 2D

#### iOS / Swift
- **Files (Key):**
  - `ios/WarrenRustRuntime/WireGuardKey.swift` (59 lines) — Key generation wrapper
  - `ios/WarrenTypes/WireGuardKey.swift` (251 lines) — Key struct definitions
  - `ios/WarrenSettings/StoredWgKeyData.swift` (38 lines) — Persisted key + rotation metadata
  - `ios/WarrenSettings/StoredDeviceData.swift` — Device metadata
  - `ios/WarrenSettings/TunnelSettingsV*.swift` (V1–V8) (~700 LOC) — Settings migration chain
  - `ios/WarrenSettings/WireGuardObfuscationSettings.swift` (224 lines) — Legacy WG obfuscation config
  - `ios/WarrenVPN/TunnelManager/WgKeyRotation.swift` — Device key rotation timer
  - `ios/WarrenVPN/View controllers/DeviceList/DeviceManaging.swift` — Device list UI
- **Lines:** ~1,200+ total
- **Removal Scenario (Option A):** Delete all WG-related files + device UI; add V9 settings migration
- **Removal Scenario (Option B):** Keep device UI; delete `WireGuardKey.swift`, `StoredWgKeyData.swift`, `WgKeyRotation.swift`; refactor `TunnelSettingsV*` to ignore WG fields
- **Bucket:** 2D

---

## Bucket 3: Heritage/Comments (Keep)

| File / Line Range | Content | Classification |
|-------------------|---------|-----------------|
| `talpid-warren-tunnel/src/lib.rs:1–30` (comments) | Heritage notes on DAITA/routing naming from Mullvad WireGuard | KEEP: Architectural context |
| `mullvad-types/src/relay_constraints.rs` | Constraint enums with `Protocol::WireGuard` variant | KEEP: Relay model still supports WG-only constraints (legacy relays) |
| `mullvad-daemon/src/lib.rs` (comments on device validation) | "mirror of Mullvad device abstraction" | KEEP: Migration history |

---

## Implementation Roadmap by Option

### Option A: Remove Entire Device Model (~6,300 LOC removal)

**Phase 1: Bucket 1 (Low Risk)**
1. Delete `/Users/poka/dev/warrenBros/warren-app/ios/build-wireguard-go.sh`
2. Remove `ios/wireguard-apple` submodule from `.gitmodules`; delete directory
3. Remove `wireguard-go` feature from `talpid-types/Cargo.toml`

**Phase 2: Type Layer (Medium Risk)**
1. Delete `mullvad-types/src/device.rs` entirely
2. Delete `mullvad-types/src/wireguard.rs` (keep only `TunnelOptions` if re-exporting needed)
3. Update `mullvad-types/src/lib.rs` to remove re-exports
4. Update `warren-api-types/src/lib.rs`: delete `Device` and `RotateDeviceWgKeyRequest`
5. Update `warren-api-client` to remove device methods

**Phase 3: Daemon Backend (High Risk)**
1. Delete `mullvad-daemon/src/device/` directory entirely
2. Refactor `mullvad-daemon/src/lib.rs` to remove `AccountManager` import; replace with lightweight identity holder
3. Remove device-related gRPC handlers from `management_interface`
4. Update `mullvad-daemon/src/warren_device_bootstrap.rs` logic to boot without device registration

**Phase 4: Warren-Core (Medium Risk)**
1. Delete `warren-api/src/devices.rs`
2. Remove `/v1/devices` HTTP handlers from `warren-api`
3. Update `warren-api-client` to remove device endpoints

**Phase 5: Platform UI (Low Risk)**
1. Desktop: remove device list views; update gRPC type stubs
2. Android: remove device repository; delete constraint types
3. iOS: delete all `WireGuardKey*.swift` files; delete `StoredWgKeyData.swift`; delete device UI; add settings migration V9

**Estimated Effort:** 3–4 weeks (high risk due to state machine refactor; requires thorough testing of login/logout paths)

---

### Option B: Keep Device, Strip WireGuard (~2,800 LOC removed; ~1,700 refactored)

**Phase 1: Bucket 1 (Same as Option A)**

**Phase 2: Type Layer Refactor (Lower Risk)**
1. In `mullvad-types/src/device.rs`: Keep struct; remove `pubkey: PublicKey` field
2. In `mullvad-types/src/wireguard.rs`: Delete `PublicKey`, `PrivateKey`, `WireguardData`, `RotationInterval`; keep `TunnelOptions`, `QuantumResistantState`, `DaitaSettings`
3. In `warren-api-types/src/lib.rs`: Keep `Device` struct; remove `wg_pubkey_hex` field; delete `RotateDeviceWgKeyRequest`
4. Update `Device` ID computation to use owner pubkey + device name (or stable hash of create time) instead of WG pubkey

**Phase 3: Daemon Refactor (Medium Risk)**
1. In `mullvad-daemon/src/device/mod.rs`: Remove `WireguardData` from `PrivateAccountAndDevice`; keep device ID/name/created
2. Delete `mullvad-daemon/src/device/service.rs` (key rotation logic); fold basic device CRUD into `mod.rs`
3. Refactor `WarrenDeviceBackend::create()` to remove `pubkey` parameter
4. Delete `WarrenDeviceBackend::replace_wg_key()` method
5. Remove `warren_device_bootstrap.rs` key generation; refactor to register device with only name/ID

**Phase 4: Warren-Core Refactor (Low Risk)**
1. In `warren-api/src/devices.rs`: Refactor `compute_device_id()` to not hash `wg_pubkey_hex`; remove `replace_wg_key()` method
2. Delete HTTP `PUT /v1/devices/{id}` endpoint (key rotation)
3. Keep `POST /v1/devices` and `DELETE /v1/devices/{id}` endpoints

**Phase 5: Platform UI (Low Risk)**
1. Remove key rotation UI from all platforms
2. Keep device list/deletion UI
3. iOS: delete `WireGuardKey*.swift`, `StoredWgKeyData.swift`, `WgKeyRotation.swift`; keep device list view; add settings V9 migration
4. Android: remove WG-specific constraint UI
5. Desktop: remove key rotation UI; keep device list

**Estimated Effort:** 2–3 weeks (lower risk; Device state machine mostly survives; refactoring is surgical)

---

### Option C: Bucket 1 Only (~85 LOC removal)

**Actions:**
1. Delete `/Users/poka/dev/warrenBros/warren-app/ios/build-wireguard-go.sh`
2. Remove `ios/wireguard-apple` from `.gitmodules`; delete directory
3. Remove `wireguard-go` feature from `talpid-types/Cargo.toml`

**Estimated Effort:** 30 minutes (zero risk; zero testing required)

---

## Test Plan for Each Option

### Option A / Option B: Test Criteria

1. **Login/Logout Flow**
   - User logs in with Warren wallet (Ed25519)
   - Device is created/registered (Option B) or identity is bootstrapped (Option A)
   - Device list shows correct entries (Option B) or is hidden (Option A)
   - Logout removes device (Option B) or is a no-op (Option A)

2. **Tunnel Connection**
   - `produce_warren_tunnel_params()` still works (should be unaffected; it never touched device keys)
   - Single-hop and multi-hop connect succeed
   - DAITA and quantum-resistant settings still respected

3. **Device UI (Option B Only)**
   - Device list displays device name, creation date (no WG key)
   - Device deletion removes device from account
   - No key rotation UI present

4. **Settings Persistence**
   - iOS: settings migrations V1–V8 still load; V9 (if added) correctly ignores WG key fields
   - Desktop/Android: settings still serialize/deserialize correctly

5. **API Contract (Option B Only)**
   - `/v1/devices` endpoints return `Device` without `wg_pubkey_hex` field
   - Old clients expecting `wg_pubkey_hex` field still parse (set to empty string on wire)

### Option C: Test Criteria

1. **Build Succeeds**
   - `cargo build --all` for warren-app
   - Xcode build for iOS
   - Android/Desktop builds

2. **No Runtime Breakage**
   - Existing code paths that reference removed files are already dead or handled by early-exit
   - No import errors

---

## Risk Assessment

| Option | Risk Level | Failure Mode | Mitigation |
|--------|-----------|--------------|-----------|
| **A** | HIGH | Login/logout broken; Device state machine collapse | Extensive testing of identity flows; consider feature-flag during rollout |
| **B** | MEDIUM | Device list inconsistency; migration issues | Settings V9 migration tested thoroughly; wire format compat shim for old clients |
| **C** | LOW | None (dead code removal) | Verify no imports of deleted files; check iOS build cache doesn't interfere |

---

## Recommendation

**Suggested Path: Option B (Keep Device, Strip WireGuard)**

- **Rationale:**
  - Minimal disruption to state machine; login/logout logic largely survives
  - Device list UI remains for multi-device bookkeeping (device names, creation dates)
  - Test surface is smaller than Option A
  - API wire format can be versioned (add `v2` endpoints) if needed
  - Settings migration is straightforward (V9 ignores WG fields)
  - ~2–3 week effort vs. 3–4 weeks for Option A

- **If immediate quick-win needed:** Option C first (~30 min), then tackle Option B in next phase

---

## Appendix: Full File List by Bucket

### Bucket 1 (Delete)

```
ios/build-wireguard-go.sh
ios/wireguard-apple/ (submodule directory)
talpid-types/Cargo.toml (1 line: wireguard-go feature)
```

### Bucket 2A (Type-Layer; Decision-Dependent)

**Deletion (Option A):**
```
mullvad-types/src/device.rs
mullvad-types/src/wireguard.rs (keep only TunnelOptions section)
warren-api-types/src/lib.rs (Device struct + RotateDeviceWgKeyRequest)
```

**Refactoring (Option B):**
```
mullvad-types/src/device.rs (remove pubkey field)
mullvad-types/src/wireguard.rs (keep TunnelOptions; remove PublicKey/PrivateKey/WireguardData)
warren-api-types/src/lib.rs (Device struct: remove wg_pubkey_hex field; delete RotateDeviceWgKeyRequest)
```

### Bucket 2B (Daemon; Decision-Dependent)

**Deletion (Option A):**
```
mullvad-daemon/src/device/ (entire directory)
mullvad-daemon/src/warren_device_bootstrap.rs
```

**Refactoring (Option B):**
```
mullvad-daemon/src/device/mod.rs (remove WireguardData from PrivateAccountAndDevice)
mullvad-daemon/src/device/device_backend.rs (refactor trait; remove pubkey param from create(); remove replace_wg_key() method)
mullvad-daemon/src/device/service.rs (delete entirely; fold CRUD into mod.rs)
mullvad-daemon/src/warren_device_bootstrap.rs (refactor to not generate WG keys)
```

### Bucket 2C (Warren-Core; Decision-Dependent)

**Deletion (Option A):**
```
warren-api/src/devices.rs
(implied: /v1/devices HTTP handlers)
```

**Refactoring (Option B):**
```
warren-api/src/devices.rs (refactor compute_device_id(); remove replace_wg_key() method)
(implied: delete PUT /v1/devices/{id} endpoint; update POST to not accept wg_pubkey_hex)
```

### Bucket 2D (UI; Decision-Dependent)

**Deletion/Refactoring (Option A/B):**
```
desktop/packages/mullvad-vpn/src/... (device UI + type stubs)
android/lib/model/src/.../WireguardConstraints.kt
android/lib/repository/.../device handling
ios/WarrenRustRuntime/WireGuardKey.swift
ios/WarrenTypes/WireGuardKey.swift
ios/WarrenSettings/StoredWgKeyData.swift
ios/WarrenSettings/WireGuardObfuscationSettings.swift (Option A: delete; Option B: keep but ignore WG fields)
ios/WarrenVPN/TunnelManager/WgKeyRotation.swift (Option A: delete; Option B: delete)
ios/WarrenVPN/View controllers/DeviceList/DeviceManaging.swift (Option A: delete; Option B: refactor to hide key rotation)
ios/WarrenSettings/TunnelSettingsV*.swift (all versions) (Option A/B: add V9 migration to skip WG key fields)
```

### Bucket 3 (Keep; No Removal)

```
(All heritage comments and naming conventions; no code deletions)
```

---

## Conclusion

This worklist provides a **precise, evidence-based map** of every WireGuard touchpoint in both repos. The key finding is:

1. **The Quinn tunnel NEVER uses WireGuard keys at runtime** (confirmed by examining `produce_warren_tunnel_params()`, which builds tunnel params using Ed25519 wallet keys only)
2. **Device management is purely user-facing** (device list, rotation timers, etc.) and can be removed or refactored independently of tunnel mechanics
3. **iOS has already removed WireGuardKit** (wireguard-apple is a stub; build script early-exits)

**Recommended action:** Proceed with **Option B** (keep Device, strip WireGuard) as a phased rollout:
- **Phase 1:** Delete Bucket 1 artifacts (~30 min; zero risk)
- **Phase 2:** Refactor Device types (~1 week; medium risk; well-scoped changes)
- **Phase 3:** Remove WG from platform UIs (~1 week; low risk; isolated UI logic)

This leaves the identity/tunnel path untouched and provides a clear audit trail for future removals.
