# Warren Identity PubKey SS58 Migration Worklist

**Scope**: Migrate the Warren user/wallet identity public key from 64-character hex string to SS58 format (Substrate address format, network prefix 13295).

**Underlying Key Type**: Ed25519 32-byte public key derived from BIP39 mnemonic.

**Critical Distinction**:
- **(A) Warren wallet/user identity Ed25519 pubkey** ← IN SCOPE. Used in X-Warren-PubKey auth header, subscription store, /v1/register, vouchers redeemed_by, admin pin pubkey.
- **(B) WireGuard device pubkey** (Curve25519, base64/hex) ← OUT OF SCOPE. Must remain untouched.
- **(C) Exit/relay infrastructure Ed25519 pubkeys** ← FLAGGED for separate review. Exit descriptor keys, relay pubkeys, HPKE keys.

---

## WARREN-CORE (Backend Rust)

### Type Definitions & Validation

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-api-types/src/lib.rs` | 27 | `const PUBKEY_HEX_LEN: usize = 64` - Hex validation constant | A |
| `crates/warren-api-types/src/lib.rs` | 36-38 | `ValidationError::InvalidPubkey(String)` - Validation error enum variant | A |
| `crates/warren-api-types/src/lib.rs` | 149-153 | `pub struct PubkeyHex(String)` - Newtype wrapper for validated hex pubkey | A |
| `crates/warren-api-types/src/lib.rs` | 155-219 | `impl PubkeyHex` - All conversion/display impls (as_str, TryFrom, Display, etc.) | A |

### Auth & Request Verification

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-identity/src/auth.rs` | 29 | `pub const HEADER_PUBKEY: &str = "X-Warren-PubKey"` - Header name constant | A |
| `crates/warren-identity/src/auth.rs` | 45-55 | `pub struct RequestSignature { pubkey_hex: String, ... }` - Request signature with hex pubkey field | A |
| `crates/warren-identity/src/auth.rs` | 63-68 | `pub struct WarrenIdentity { pubkey: VerifyingKey, pubkey_hex: String }` - Identity with hex pubkey | A |
| `crates/warren-identity/src/auth.rs` | 73-100+ | Error enum (`AuthError`) including `InvalidPubkey`, `PubkeyNotOnCurve` | A |

### API Request/Response Types

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-api-types/src/lib.rs` | 450-490 | `/v1/register` request body `RegisterRequest { pubkey_hex: PubkeyHex, voucher_secret: ... }` | A |
| `crates/warren-api-types/src/lib.rs` | 520-556 | Subscription response types with `active_pubkeys: Vec<PubkeyHex>`, `removed: Vec<PubkeyHex>` | A |
| `crates/warren-api-types/src/lib.rs` | 564-575 | `CrlRevocation { pubkey_hex: PubkeyHex, ... }` - CRL revocation entry | A |
| `crates/warren-api-types/src/lib.rs` | 592 | `admin_pubkey_hex: String` in CRL response | A |
| `crates/warren-api-types/src/lib.rs` | 602-640 | `EnrolledExit` and enrollment response with `pubkey_hex: PubkeyHex` | A |
| `crates/warren-api-types/src/lib.rs` | 656-670 | Device response types with `owner_pubkey_hex: PubkeyHex` | A |
| `crates/warren-api-types/src/lib.rs` | 726, 736, 748 | Various types with `wg_pubkey_hex: String` (WireGuard, OUT OF SCOPE) | B |
| `crates/warren-api-types/src/lib.rs` | 763, 787 | Exit & relay request types with `pubkey_hex: PubkeyHex` | C |
| `crates/warren-api-types/src/lib.rs` | 860 | `PortForward { owner_pubkey_hex: PubkeyHex, wg_pubkey_hex: String, ... }` | A + B |
| `crates/warren-api-types/src/lib.rs` | 866 | `wg_pubkey_hex: String` in PortForward (WireGuard) | B |
| `crates/warren-api-types/src/lib.rs` | 902 | `Voucher { redeemed_by_pubkey_hex: Option<PubkeyHex>, ... }` | A |
| `crates/warren-api-types/src/lib.rs` | 955, 957 | Incident types with `client_pubkey_hex: Option<PubkeyHex>`, `exit_pubkey_hex: PubkeyHex` | A + C |
| `crates/warren-api-types/src/lib.rs` | 1025 | Incident response with `client_pubkey_hex: Option<PubkeyHex>` | A |
| `crates/warren-api-types/src/lib.rs` | 1234, 1244 | Enrollment token response with `redeemed_by: Option<PubkeyHex>`, `created_by: PubkeyHex` | A |
| `crates/warren-api-types/src/lib.rs` | 1278 | Device registration with `owner_pubkey_hex: PubkeyHex` | A |
| `crates/warren-api-types/src/lib.rs` | 1331 | Exit response with `exit_pubkey_hex: PubkeyHex` | C |
| `crates/warren-api-types/src/lib.rs` | 1359-1362 | Pubkey change records `old_pubkey_hex: String`, `new_pubkey_hex: String` | A |
| `crates/warren-api-types/src/lib.rs` | 1421 | Route update with `exit_pubkey_hex: PubkeyHex` | C |

### Database Schema

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-api/migrations/20260525_000001_initial_schema.up.sql` | 36, 49 | `enrolled_exits(pubkey_hex)` - Exit identity column | C |
| `crates/warren-api/migrations/20260525_000001_initial_schema.up.sql` | 78 | `vouchers(redeemed_by_pubkey_hex)` - Voucher redemption tracking | A |
| `crates/warren-api/migrations/20260525_000001_initial_schema.up.sql` | 86 | `subscriptions(pubkey_hex)` - User subscription registry (PRIMARY KEY) | A |
| `crates/warren-api/migrations/20260525_000001_initial_schema.up.sql` | 102, 114 | `subscription_events(pubkey_hex)` and `audit_log(admin_pubkey_hex)` | A |

### Configuration & Logging

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-config/src/lib.rs` | 226 | `pub const LOG_PREFIX_LEN: usize = 8` - Truncation for log privacy | A |
| `crates/warren-config/src/lib.rs` | 240-246 | `pub fn log_prefix(s: &str) -> &str` - Truncate pubkey to first 8 hex chars for logging | A |
| `crates/warren-config/src/lib.rs` | 641-659 | Tests for `log_prefix` function | A |

### Key Derivation

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-identity/src/lib.rs` | 224-243 | `pub fn derive_node_key(seed: &[u8; 32]) -> SigningKey` - HKDF derivation of Ed25519 signing key | A |
| `crates/warren-identity/src/lib.rs` | 467-489 | Vector tests `seed_ab_produces_known_pubkey`, `seed_zero_produces_known_pubkey` | A |
| `crates/warren-identity/src/lib.rs` | 531-554 | BIP39 vector tests with known hex pubkey outputs | A |

### Admin & Exit-Specific Code

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-config/src/lib.rs` | 221-226 | Admin pubkey references in no-log documentation | A |
| `crates/warren-admin/src/handlers.rs` | 74 | `state.admin_pubkey_hex` field access | A |
| `crates/warren-exit/src/main.rs` | 758 | Admin pubkey logging with `log_prefix` | A |

### Tests (E2E, Unit, Wire Format)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-api-types/tests/auth_critical_pubkey_validation.rs` | - | Critical pubkey validation tests | A |
| `crates/warren-api-types/tests/wire_format_lock.rs` | 140, 144 | CRL wire format with `admin_pubkey_hex` | A |
| `crates/warren-api-client/tests/register_e2e.rs` | - | Registration E2E with pubkey_hex | A |
| `crates/warren-api-client/tests/enroll_e2e.rs` | 30, 36 | Enrollment E2E with `admin_pubkey_hex` | A |
| `crates/warren-api-client/tests/admin.rs` | 32 | Admin test with hex pubkey | A |
| `crates/warren-exit/tests/log_privacy.rs` | - | Log privacy verification (pubkey truncation) | A |
| `crates/warren-api/tests/log_privacy.rs` | - | API log privacy tests | A |

### Supporting Files

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `crates/warren-identity/src/bin/warren_pubkey_derive.rs` | 11+ | CLI tool for deriving pubkey from mnemonic (uses hex output) | A |
| `crates/warren-api-client/src/lib.rs` | 57, 62, 103 | CRL verification with `admin_pubkey_hex` validation | A |

---

## WARREN-APP (Client Applications)

### Core Types (Rust)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `mullvad-types/src/warren_pubkey.rs` | 20 | `pub const PUBKEY_HEX_LEN: usize = 64` - Hex validation constant | A |
| `mullvad-types/src/warren_pubkey.rs` | 34-36 | `pub struct WarrenPubKey(pub(crate) String)` - Newtype wrapper | A |
| `mullvad-types/src/warren_pubkey.rs` | 49-56 | `ParseError` enum (InvalidLength, NonHex) | A |
| `mullvad-types/src/warren_pubkey.rs` | 59-87 | `impl WarrenPubKey` - from_str, to_bytes, from_bytes, as_str | A |
| `mullvad-types/src/warren_pubkey.rs` | 90-105 | `impl FromStr` for WarrenPubKey | A |
| `mullvad-types/src/warren_pubkey.rs` | 113-242 | Comprehensive unit tests (serde, display, bytes roundtrip, etc.) | A |

### API & Daemon Communication (Rust)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `mullvad-api/src/warren_auth.rs` | - | Signer with `pubkey_hex` field; signing logic for X-Warren-PubKey header | A |
| `warren-jni/src/wallet.rs` | - | JNI binding for pubkey_hex_from_mnemonic (FFI boundary) | A |
| `warren-jni/src/android_jni.rs` | - | Android JNI layer for wallet pubkey operations | A |

### Daemon RPC Types (TypeScript)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts` | 39 | `export type AccountNumber = string` (legacy, to be deprecated) | A |
| `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts` | 42-47 | `export type WarrenPubKey = string` - 64-char hex type alias | A |

### Desktop (TypeScript/React)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `desktop/packages/mullvad-vpn/src/renderer/lib/pubkey.ts` | 1-2 | Constants: `WARREN_PUBKEY_HEX_LEN = 64`, `WARREN_PUBKEY_GROUP_SIZE = 8` | A |
| `desktop/packages/mullvad-vpn/src/renderer/lib/pubkey.ts` | 4-8 | `export function formatWarrenPubKey(pubkey: string)` - Formats into 8x8 groups | A |
| `desktop/packages/mullvad-vpn/src/shared/utils.ts` | - | `export function isWarrenPubKey(s: unknown): boolean` - Validation predicate | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/WarrenPubKeyLabel.tsx` | - | Display component for pubkey label | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/RedeemVoucher.tsx` | - | Voucher redemption form with pubkey display | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/RedeemVoucherStyles.tsx` | - | Styling for voucher component (pubkey-related) | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/account/AccountView.tsx` | - | Account view displaying Warren pubkey | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/account/components/warren-pubkey-row/WarrenPubKeyRow.tsx` | - | Account row component for pubkey display | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/account/components/warren-pubkey-row/index.ts` | - | Export for WarrenPubKeyRow | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/login/LoginView.tsx` | - | Login view with pubkey input/display | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/ExpiredAccountErrorViewStyles.tsx` | - | Error view styling (references pubkey) | A |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/expired-account-error/ExpiredAccountErrorView.tsx` | - | Expired account error view | A |
| `desktop/packages/mullvad-vpn/src/renderer/features/warren-pubkey-warning/components/WarrenPubKeyWarning.tsx` | - | Warning modal for pubkey mismatch | A |
| `desktop/packages/mullvad-vpn/src/renderer/features/warren-pubkey-warning/index.ts` | - | Export for WarrenPubKeyWarning feature | A |
| `desktop/packages/mullvad-vpn/src/renderer/app.tsx` | - | Main app mounting WarrenPubKeyWarning modal | A |
| `desktop/packages/mullvad-vpn/src/renderer/redux/account/actions.ts` | - | Account action creators (pubkey-related) | A |
| `desktop/packages/mullvad-vpn/src/renderer/redux/account/reducers.ts` | - | Account reducer handling pubkey state | A |
| `desktop/packages/mullvad-vpn/test/unit/warren-pubkey.spec.ts` | All | Unit tests for `formatWarrenPubKey` and `isWarrenPubKey` | A |
| `desktop/packages/mullvad-vpn/test/unit/warren-pubkey-warning.spec.ts` | - | Tests for warning modal | A |

### gRPC / Proto Definitions

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `mullvad-management-interface/proto/management_interface.proto` | 679 | Comment referencing WarrenPubKeyWarning modal | A |
| `mullvad-management-interface/proto/management_interface.proto` | 702, 704 | `pinned_pubkey_hex` and `observed_pubkey_hex` fields (exit key rotation detection) | C |
| `mullvad-management-interface/proto/management_interface.proto` | 721 | `new_pubkey_hex` for exit pubkey update | C |
| `mullvad-management-interface/proto/management_interface.proto` | 750, 751 | `old_pubkey_hex`, `new_pubkey_hex` in pubkey change message | A |

### Android (Kotlin)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `android/lib/model/src/main/kotlin/com/warrenbrowse/vpn/lib/model/wallet/WalletIdentity.kt` | - | Data class holding wallet pubkey (WalletPubkeyHex) | A |
| `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnAdapter.kt` | - | Adapter using pubkey_hex from JNI | A |
| `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt` | - | Tunnel config using pubkey | A |
| `android/app/src/main/kotlin/com/warrenbrowse/vpn/jni/WarrenJni.kt` | - | JNI wrapper for pubkey operations | A |
| `android/app/src/main/kotlin/com/warrenbrowse/vpn/jni/WarrenJniBridgeImpl.kt` | - | JNI bridge implementation | A |
| `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/connect/RelayCatalog.kt` | - | Relay catalog (exit pubkey references) | C |
| `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/connect/RelayInfo.kt` | - | Relay info structure (exit pubkey) | C |

### iOS (Swift)

| File | Line(s) | Description | Category |
|------|---------|-------------|----------|
| `ios/WarrenVPN/Extensions/String+AccountFormatting.swift` | - | Formatting extension for account number → pubkey display | A |
| `ios/WarrenVPN/Extensions/String+Helpers.swift` | - | Helper extensions (truncation, validation) | A |
| `ios/WarrenVPN/Screens/Account/Components/AccountTextField.swift` | - | Input field for account/pubkey | A |
| `ios/WarrenVPN/Screens/Account/Components/AccountNumberRow.swift` | - | Display row for account number/pubkey | A |

---

## Scope Summary

### File Count by Category (Approximate)

| Category | Warren-Core | Warren-App | Total | Notes |
|----------|-------------|-----------|-------|-------|
| **(A) Warren identity pubkey** | 35+ | 30+ | 65+ | IN SCOPE: migration target |
| **(B) WireGuard device pubkey** | 5 | 2 | 7 | OUT OF SCOPE: leave untouched (base64/Curve25519) |
| **(C) Exit/relay infrastructure pubkey** | 12 | 8 | 20 | FLAGGED: review separately (exit descriptor, relay keys) |
| **Type definitions/validation** | 5 | 4 | 9 | `PubkeyHex`, `WarrenPubKey` newtypes |
| **Auth & request verification** | 3 | 2 | 5 | `X-Warren-PubKey` header, RequestSignature |
| **API types/wire format** | 12 | 8 | 20 | Request/response DTOs |
| **Database schema** | 4 | - | 4 | SQL migrations (subscriptions, vouchers, audit log) |
| **Logging & config** | 3 | 2 | 5 | `log_prefix`, truncation to 8 chars |
| **Tests (unit/E2E)** | 8 | 6 | 14 | Validation, wire format, E2E |
| **UI/Display components** | - | 15+ | 15+ | Desktop React, Android Kotlin, iOS Swift |

### Migration Checklist

#### Phase 1: Core Rust Codec & Types
- [ ] Implement SS58 codec in `warren-protocol` (or new crate)
  - Encode: `[u8; 32]` → SS58 string (network prefix 13295)
  - Decode: SS58 string → `[u8; 32]`
  - Validation: Reject non-SS58 or wrong network prefix
- [ ] Create newtype wrapper(s) to replace `PubkeyHex` with `WarrenAddress` (or similar)
- [ ] Update `warren-api-types` validation layer
- [ ] Update `warren-config::log_prefix` if needed (SS58 is longer than hex)

#### Phase 2: HTTP API & Persistence
- [ ] Migrate `X-Warren-PubKey` header to accept SS58
  - Start with backward-compat: accept both hex and SS58, normalize internally
  - Set deadline to deprecate hex support
- [ ] Update SQL migrations to handle hex → SS58 conversion
- [ ] Update `/v1/register`, voucher endpoints, subscription store
- [ ] Update admin pubkey configuration to accept SS58
- [ ] Update audit log, CRL, all database columns

#### Phase 3: Client-Side (Desktop TS, Android, iOS)
- [ ] Implement SS58 codec in TypeScript (`@polkadot/util-crypto` or equivalent)
- [ ] Update `WarrenPubKey` type in daemon-rpc-types
- [ ] Update display formatting: 64-char hex → SS58 groups
- [ ] Update input validation: accept SS58, normalize from hex if legacy
- [ ] Android JNI: call Rust codec to convert mnemonic → SS58
- [ ] iOS: same via FFI binding

#### Phase 4: Testing & Deprecation
- [ ] E2E tests with both hex and SS58 requests
- [ ] Vector tests for known mnemonics → SS58 output
- [ ] Deprecation timeline: hex support sunset after migration window
- [ ] Documentation: explain why SS58, how to migrate, backward-compat window

---

## Key Assumptions

1. **Network prefix 13295** is locked (Substrate naming standard for Warren; immutable once deployed).
2. **Underlying Ed25519 key is unchanged** — only the string representation changes.
3. **Log privacy** (truncation to 8 chars) may need to account for SS58 length (≈50 chars vs 64 hex).
4. **Backward compatibility window** required before sunsetting hex support (estimated 1-2 release cycles).
5. **Exit/relay pubkeys** (Category C) use separate deployment / signing pipeline; coordinate separately.

---

**Generated**: 2026-05-31  
**Status**: Ready for implementation review

