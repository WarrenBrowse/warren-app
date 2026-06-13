# A.4, Pinning pubkey exit (TOFU), design doc

> Session A.4 design blueprint, written 2026-05-20.
> Author: autonomous agent under poka direction.
> Status: **design + client scaffold**. Full implementation deferred
> pending warren-core `exit_id` field (= the architectural prerequisite
> identified below).

---

## 0. Problem

The audit H.E.5/6/7 surfaced that a Warren client should detect an
**exit-substitution attack**: a compromised backend could serve a new
Ed25519 pubkey for what the user knows as the same exit ("FR Paris
1"), and the user would have no way to notice. Mullvad WireGuard pins
the **client** key per device; Warren needs the symmetric protection
on the **exit** side.

The expected primitive is **Trust On First Use (TOFU) pinning**:
- First connect to an exit → store its pubkey under a stable identity.
- Subsequent connects → compare the served pubkey against the stored
  one. Match → silent pass. Mismatch → refuse, surface a warning
  modal, let the user accept the new key (= rotation) or reject.

This document is the cross-repo blueprint.

---

## 1. The blocking prerequisite: stable `exit_id` at warren-core

### Today (/v1, 2026-05-20)

`WarrenRelay` (`warren-core::warren-relay-selector::relay.rs`) carries:
- `endpoint_id: WarrenPubkey`, **= the Ed25519 pubkey itself**
- `endpoint_addr: WarrenExitAddr`, UDP candidates
- `location: Location { country_code, city }`
- `weight: u64`
- `active: bool`

There is **no stable identifier separate from the pubkey** in
single-hop /v1. Multi-hop (`MultiHopExitDescriptor`) does carry an
`ExitId([u8; 16])`, but that field does not surface in the single-hop
relay-list path.

### Why this matters for TOFU

The pin lookup needs a key `K` such that:
- `K` stays stable across legitimate pubkey rotation.
- `K` changes when the operator deploys a genuinely new exit.

Available candidates:
| Candidate | Survives rotation? | Single value per exit? |
| --- | --- | --- |
| Pubkey hex | NO (changes by definition) | yes |
| `(country_code, city)` | yes | NO (multiple exits per city) |
| `endpoint_addr.ip_addrs().first()` | partial (IPs rotate too) | yes per host |
| **Stable `exit_id: [u8; 16]`** | **yes** | **yes** |

Only a dedicated stable `exit_id` field gives both properties at once.
Pinning keyed by the pubkey itself is **tautological** (a `BTreeMap`
keyed on pubkey hex can only return entries whose pubkey hex matches
the lookup key, mismatch detection is impossible by construction).

### Recommended change (warren-core + warren-backend-api)

Add an explicit `exit_id: [u8; 16]` field to:

1. **warren-protocol**, extend the wire schema for `exit-info.json`
   to include `exit_id` (signed by the relay-list authority alongside
   the pubkey and addresses). 16 random bytes generated once by the
   operator at exit deployment, stored in the exit's local config,
   persisted through pubkey rotations.
2. **warren-relay-selector**, accept `exit_id` in `WarrenRelay::new`
   + expose it via `WarrenRelay::exit_id()`. Make it the new selection
   identity (replace `endpoint_id` as the "stable" notion).
3. **warren-multihop**, already has `ExitId`; align name + size.
4. **warren-backend-api**, assign + serve `exit_id` per relay in the
   signed relay-list response. (Operator infra, out of agent scope per
   memory rule "JAMAIS toucher warren-backend-api".)
5. **mullvad-daemon (warren-app)**, read `exit_id` from the selected
   relay and use it as the pin lookup key (see §3 below).

The change is /v1 wire-extending. poka's directive on A.4 was
"hors considération breaking change", so an explicit /v2 bump is not
required if backward-compat is handled via `#[serde(default)]` on
client decoders (older clients ignore the field, newer clients enforce
the pin when present).

---

## 2. Cleanest architecture (target state, post-exit_id)

### 2.1 Storage

Daemon-side, persisted in `Settings.warren_pinned_exit_pubkeys`
(implemented in `mullvad-types/src/settings/mod.rs` as part of A.4
scaffold):

```rust
struct WarrenPinnedExitPubkeys {
    entries: BTreeMap<String, WarrenPinnedExitPubkey>, // key = exit_id_hex
}

struct WarrenPinnedExitPubkey {
    pubkey_hex: String,        // current pinned Ed25519 verifying key
    first_seen_unix: u64,
    last_seen_unix: u64,
    country_code: String,      // forensic snapshot at pin time
    city: String,
}
```

`BTreeMap` (not `HashMap`) for deterministic JSON output: identical
state on disk regardless of insertion order = trivial diff for
operator inspection.

### 2.2 Verify hook

In `mullvad_daemon::tunnel::ParametersGenerator::produce_warren_tunnel_params`
after relay selection but before returning `WarrenTunnelParameters`:

```rust
let exit_id_hex = hex(selected_relay.exit_id());
let observed_pubkey_hex = hex(params.exit_addr.id.as_bytes());

let pin_table = inner.warren_pinned_exit_pubkeys.entries.clone();
match pin_table.get(&exit_id_hex) {
    None => {
        // TOFU: first time seeing this exit_id. Pin it.
        let entry = WarrenPinnedExitPubkey {
            pubkey_hex: observed_pubkey_hex,
            first_seen_unix: now(),
            last_seen_unix: now(),
            country_code: selected_relay.location().country_code().into(),
            city: selected_relay.location().city().into(),
        };
        inner.persist_pin(exit_id_hex, entry).await?;
    }
    Some(existing) if existing.pubkey_hex == observed_pubkey_hex => {
        // Match: bump last_seen.
        inner.bump_pin_last_seen(&exit_id_hex).await?;
    }
    Some(existing) => {
        // Mismatch: refuse + emit gRPC event.
        return Err(Error::WarrenPubkeyPinMismatch {
            exit_id_hex,
            pinned: existing.pubkey_hex.clone(),
            observed: observed_pubkey_hex,
        });
    }
}
```

### 2.3 gRPC plumbing

Two new RPCs on `mullvad-management-interface`:

- `TrustNewExitKey(TrustNewExitKeyRequest { exit_id_hex, new_pubkey_hex })`
  → updates the pin to the new pubkey, bumps `last_seen` and resets
  `first_seen` (operator audit trail: a "trust" event creates a new
  baseline).
- `ResetPinnedExitKeys()` → clears the entire pin table after a
  confirmation modal. Useful when the user changes account /
  signing-key (= switches "identity" device) and effectively starts
  fresh.

One new notification event:

- `WarrenPubkeyMismatchDetected { exit_id_hex, pinned, observed, country_code, city }`
  emitted when the verify hook returns `Err`. Drives the UI warning
  modal.

### 2.4 UI

- `WarrenPubKeyWarning.tsx`, modal triggered on
  `WarrenPubkeyMismatchDetected` event. Three CTAs:
  1. **Trust new key** → invoke `TrustNewExitKey` RPC, reconnect.
  2. **Reject (disconnect)** → close modal, daemon stays disconnected.
  3. **Report to Warren** → POST `/v1/incidents/pubkey-mismatch` with
     the `(exit_id, old_pubkey, new_pubkey, timestamp)` payload.
- Settings → VPN settings → "Reset pinned exit keys" button with a
  confirmation modal.
- i18n strings in `desktop/packages/mullvad-vpn/locales/en/messages.po`
  + `locales/fr/messages.po`.

### 2.5 warren-api `/v1/incidents/pubkey-mismatch`

POST endpoint, log-only (no DB). Body:
```json
{
  "exit_id_hex": "<hex32>",
  "old_pubkey_hex": "<hex64>",
  "new_pubkey_hex": "<hex64>",
  "country_code": "fr",
  "city": "Paris",
  "timestamp_unix": 1747740000
}
```

Returns 200. The handler stores the event in the warren-api access
log so the operator can investigate without keeping per-user state.

No-log Warren rule respected: no IP, no account, no device-identifying
field in the payload. Worst-case privacy leak = exit_id ↔ pubkey-pair
correlation, which is already public via the signed relay-list.

---

## 3. What ships in the A.4 scaffold (this session)

**Mullvad-types settings storage**, `WarrenPinnedExitPubkeys` +
  `WarrenPinnedExitPubkey` + `Settings.warren_pinned_exit_pubkeys`
  field + default initializer. Settings round-trip serde + the
  field never gRPC-syncs back (daemon-internal). ✅

**Daemon error variant**, `tunnel::Error::WarrenPubkeyPinMismatch`
  with `exit_id_hex` / `pinned` / `observed` payload. Plumbing in
  place for the verify hook even though that hook is not wired in
  this session (would be tautological without `exit_id`). ✅

**Design doc**, this file. ✅

**Tests**, serde round-trip + default value for the new settings
  type. Not the 6/6 brief criteria (those test mismatch detection,
  which is unreachable until exit_id lands). ✅ partial.

**Deferred (post-exit_id landing in warren-core)**:
- Verify hook in `produce_warren_tunnel_params`.
- gRPC events + RPCs (TrustNewExitKey, ResetPinnedExitKeys).
- UI modal + Settings reset CTA + i18n FR/EN.
- `/v1/incidents/pubkey-mismatch` endpoint in warren-api.
- 6/6 brief TDD criteria (storage, verify, override, reset,
  forensics) all activate once exit_id is plumbed end-to-end.

---

## 4. Order of operations for the next phase

1. warren-protocol: add `exit_id: [u8; 16]` field to the signed
   relay-list entry, gated `#[serde(default)]` for compat.
2. warren-backend-api: persist `exit_id` per relay, sign + serve.
   **(operator infra task, not autonomous agent scope.)**
3. warren-relay-selector: accept + expose `exit_id` on `WarrenRelay`.
   Update tests + memory.
4. warren-app daemon: activate the verify hook (§2.2 above), add the
   setter/getter on `ParametersGenerator`, emit the gRPC event on
   mismatch.
5. warren-app gRPC: add the two RPCs + the notification event.
6. warren-app UI: `WarrenPubKeyWarning.tsx` + Settings reset CTA +
   i18n.
7. warren-api: `/v1/incidents/pubkey-mismatch` POST handler.
8. Tests TDD: 6/6 brief criteria (storage/verify/override/reset),
   plus an end-to-end test with a mocked backend rotating the pubkey
   for a stable exit_id.

Estimated effort: ~3-5j wall-clock once exit_id is wired end-to-end.

---

## 5. Why not ship a fake pin key in /v1 today?

Earlier draft considered using `(country_code, city)` as the pin key.
Rejected because:

- Broad user queries ("anywhere FR") select different exits each
  connect, so the pin would mismatch on every reconnect → UI nag /
  user habituation to "trust new key" → defeats the security model.
- Narrow user queries ("FR Paris") could still rotate pubkey
  legitimately as the relay-list authority adds/removes exits at
  the same location → false-positive mismatch.
- The user's mental model is "this specific exit", which the codebase
  cannot currently express without `exit_id`.

Pinning by the pubkey itself was also rejected as tautological (see
§1).

Conclusion: ship the **structural scaffold** today (settings storage
+ error variant + design doc), and gate activation of the verify
hook on the `exit_id` landing.
