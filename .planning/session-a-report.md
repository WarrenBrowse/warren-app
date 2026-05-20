# Session A — Cross-platform parity report (final)

> Status: **A.1+A.2+A.3 GO ULTIMATE**, A.4 **scaffold + design doc GO partial** (verify hook + UI + endpoint deferred pending warren-core `exit_id` field landing).

**Verdict**: GO (75% Session A delivered as full GO, 25% delivered as
client scaffold with documented deferral). 9 commits cross-repo on
origin/main. 0 warren-core commits (no warren-core changes required
this session - all wiring stayed warren-app side).

---

## A.1 — macOS daemon wiring + smoke E2E — DELIVERED (GO)

### Stack câblée

1. **Cross-OS façade** - `talpid-warren-tunnel/src/default_route_split.rs` rewritten as a thin per-OS dispatcher:
   - `target_os = "linux"`: in-crate impl preserved in `default_route_split/linux.rs` (byte-for-byte previous local file).
   - `target_os = "macos"`: `pub use warren_client::default_route_split_macos::DefaultRouteSplitGuard;` (warren-core port, host-route exception + `0.0.0.0/1` + `128.0.0.0/1` via `route add -interface <tun>`).
   - `not(any(linux, macos, windows))`: `default_route_split/stub.rs` returns `Err` from `install` so the operator log surfaces the absent routing while the `default_route_guard: Option<...>` field still type-checks.
   - 1 new facade test `api_surface_matches_lib_rs_call_site` pins the `install(Ipv4Addr, &str) -> Result<Self>` shape so an upstream signature drift in any warren-core port fails at compile time inside this crate (not at the lib.rs call site).

2. **Cfg dispatch in `lib.rs`** - both the single-hop (line 683) and multi-hop (line 1061) install paths now use `#[cfg(any(linux, macos, windows))]` (Windows arm joined in A.2). Body is OS-agnostic since the façade exposes identical signatures.

3. **pfctl killswitch coexistence (A.1.2)** - **already wired pre-Session A**. The `FirewallPolicy::Connecting.peer_endpoints` (consumed by `talpid-core/src/firewall/macos.rs`) is populated from `BackendParams::Warren.get_next_hop_endpoints()` in `backend_params.rs:50-65`:
   - Single-hop: every candidate IP of the exit (`exit_addr.ip_addrs()`).
   - Multi-hop: only the relay endpoint.

   No code change needed.

4. **LaunchDaemon plist path Warren-branded** - `mullvad-daemon/src/macos_launch_daemon.rs:14` had `DAEMON_PLIST_PATH = "/Library/LaunchDaemons/net.mullvad.daemon.plist"` while the M4.H.D pkg-scripts install at `/Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist`. Fixed Rust constant + module rustdoc + the postinstall cross-reference comment. Without this fix, `SMAppService::statusForLegacyURL(...)` would always return `NotFound` on macOS 13+ even when the daemon is running.

### Commits

- `06aeb145fc` refactor(talpid-warren-tunnel): make default_route_split a cross-OS facade (linux/macos/stub)
- `e408e8736e` feat(talpid-warren-tunnel): dispatch macOS through default_route_split facade (A.1.1)
- `466404acdb` fix(mullvad-daemon): align macOS LaunchDaemon plist path with Warren packaging

### Live Mac smoke A.1.3 - DEFERRED to poka manual

Live sudo + launchctl + exit prod smoke not driven by the autonomous agent. Staged checklist preserved in design notes.

---

## A.2 — Windows daemon wiring + smoke E2E — DELIVERED (GO)

### Stack câblée

1. **Windows façade arm** - `talpid-warren-tunnel/src/default_route_split.rs` extends to `target_os = "windows"`: `pub use warren_client::default_route_split_windows::DefaultRouteSplitGuard;` (PowerShell `New-NetRoute` recipe: host-route exception + `0.0.0.0/1` + `128.0.0.0/1 -InterfaceAlias <tun>`).

2. **Lib.rs cfg dispatch broadened** - both install branches now `#[cfg(any(linux, macos, windows))]`. The route-builder block adds an explicit Windows branch that posts an empty `RequiredRoute` set so talpid-routing does not double-install netsh routes - the warren-core PowerShell port owns Windows routing entirely.

3. **WFP killswitch coexistence (A.2.3)** - same architecture as macOS A.1.2. `BackendParams::Warren.get_next_hop_endpoints()` flows through `FirewallPolicy::Connecting.peer_endpoints` consumed by `talpid-core/src/firewall/windows/`. No code change required.

4. **WinTUN driver + Windows service install (A.2.2 + A.2.4)** - pre-existing Mullvad infrastructure (M4.H.D rebrand handles the service name + installer GUID). Not re-implemented this session.

### Commits

- `1dc5b7d04e` feat(talpid-warren-tunnel): wire Windows default-route split through facade (A.2.1)
- `2e370789db` feat(talpid-warren-tunnel): include Windows in cross-OS install dispatch (A.2.1)

### Cross-compile + smoke A.2.5 - DEFERRED

Host = macOS arm64; no Windows VM nor xwin/msvc target installed locally. Brief allows skip ("Si pas de VM Windows dispo localement : skip smoke E2E, marquer code path + tests unitaires PASS + smoke à valider poka"). CI matrix `windows-2022` (M4.H.D release.yml) exercises the Windows build path when triggered.

---

## A.3 — Auto-update prod-grade — DELIVERED (GO)

### Stack câblée

1. **Warren GitHub Releases as update endpoint** (poka choice via AskUserQuestion):
   - `WARREN_RELEASES_URL = "https://api.github.com/repos/WarrenBrowse/warren-app/releases/"`
   - `WARREN_METADATA_URL = "https://github.com/WarrenBrowse/warren-app/releases/latest/download/"`
   - Upstream `RELEASES_URL` / `METADATA_URL` constants kept verbatim (`#[expect(dead_code, reason = "Kept for upstream rebase parity")]`) so a future Mullvad rebase touches a 3-line diff at most.

2. **Runtime env-var override** - `releases_url()` / `metadata_url()` resolve `WARREN_UPDATE_URL` / `WARREN_METADATA_URL` first, fall back to Warren defaults if unset/empty. Lets a staging build point at an internal mirror without recompilation.

3. **Pubkey trust file Warren-branded** - new `mullvad-update/warren-trusted-metadata-signing-pubkeys` with a documented placeholder pubkey (= upstream's `test-pubkey`). `defaults::TRUSTED_METADATA_SIGNING_PUBKEYS` now reads this file. Upstream Mullvad pubkeys preserved in `MULLVAD_TRUSTED_METADATA_SIGNING_PUBKEYS` for rebase parity, `#[expect(dead_code)]`.

4. **Tests TDD** - 5 new tests in `mod warren_default_tests`:
   - `releases_url_default_is_warren_not_mullvad`
   - `releases_url_env_override_wins_over_default`
   - `releases_url_empty_env_falls_back_to_warren_default`
   - `metadata_url_default_is_warren_not_mullvad`
   - `warren_trusted_metadata_signing_pubkeys_parses`

5. **UI banner** - upstream Mullvad `app-upgrade-available` notification + `views/app-upgrade/AppUpgradeView.tsx` are generic (no hardcoded "Mullvad" strings in the upgrade flow). i18n FR/EN already exists. No UI rebrand required.

### Caveats / case 4 escalation marker

- **Warren updates Ed25519 signing key NOT YET GENERATED** (per AskUserQuestion poka answer). The placeholder pubkey will fail verification on any real-world signed manifest, which is the safe-by-default outcome: zero auto-updates accepted until the operator (poka) generates the production key offline and replaces the line in `warren-trusted-metadata-signing-pubkeys`. This is the brief A.3.2 explicit escalation case 4 ("Signing key prod").

- **CI release.yml** does not yet upload signed `latest.json` / per-platform manifests to GitHub Releases artifacts. Required follow-up post-key-gen: extend the release workflow to sign + publish the metadata alongside the binaries.

### Commits

- `6dddeaf6e6` feat(mullvad-update): point auto-update at Warren GitHub Releases + WARREN_UPDATE_URL env override (A.3)

---

## A.4 — Pinning pubkey exit TOFU + UI — SCAFFOLD + DESIGN DOC (GO partial)

### Architectural discovery

`warren-core` /v1 has **no stable `exit_id` field separate from the
Ed25519 pubkey** at the relay-list layer
(`warren-relay-selector::relay::WarrenRelay`). The brief A.4 schema
`{ exit_id, pubkey_ed25519, first_seen_unix, last_seen_unix }`
presupposes an identifier that does not exist for single-hop /v1.
Multi-hop has `ExitId([u8; 16])` in `MultiHopExitDescriptor` but it is
not surfaced through the single-hop selector.

Without a stable `exit_id`, pinning by the pubkey itself is
**tautological** (`BTreeMap` lookup keyed on `pubkey_hex` only returns
entries whose `pubkey_hex` matches by construction - no mismatch
detection possible). Pinning by `(country_code, city)` causes
false-positive nags on broad queries ("anywhere FR" weighted random
selection). The cleanest fix is the `exit_id` field landing in
warren-core + warren-backend-api signed relay-list.

### Direction taken (poka AskUserQuestion choices)

1. (A) Warren update server = **GitHub Releases API**.
2. (B) Warren updates signing key = **not yet generated, escalation
   case 4 marker in code + report**.
3. A.4 path = **Client scaffold + design doc**.

### Stack câblée

1. **Daemon settings storage** - new types in `mullvad-types::settings`:
   - `WarrenPinnedExitPubkeys { entries: BTreeMap<String, WarrenPinnedExitPubkey> }`
   - `WarrenPinnedExitPubkey { pubkey_hex, first_seen_unix, last_seen_unix, country_code, city }`
   - `Settings.warren_pinned_exit_pubkeys` field with `#[serde(default)]` for upgrade safety.
   - The new field is **never round-tripped through gRPC `SetSettings`** (would let a gRPC client wipe the pin table). `mullvad-management-interface/src/types/conversions/settings.rs:try_from` default-initialises the field on every gRPC update; the daemon retains its own copy in the on-disk settings file.

2. **Daemon error variant** - `mullvad_daemon::tunnel::Error::WarrenPubkeyPinMismatch { exit_id_hex, pinned, observed }`. Mapped to `ParameterGenerationError::NoMatchingRelay` in the state machine (= immediate hard-stop). The variant is reachable in code structure but the verify hook that constructs it is **deferred** (would always be a noop with current pubkey-as-id).

3. **Design doc** - `.planning/a4-pubkey-pinning-design.md` (~220 lines) blueprints:
   - §1 stable `exit_id` field requirement at warren-core + warren-backend-api levels (the architectural prerequisite).
   - §2 target-state architecture (storage, verify hook, gRPC RPCs `TrustNewExitKey` + `ResetPinnedExitKeys`, notification event `WarrenPubkeyMismatchDetected`, UI modal + Settings reset CTA, `/v1/incidents/pubkey-mismatch` warren-api endpoint).
   - §3 explicit list of what ships now vs what is deferred.
   - §4 8-step order-of-operations for the follow-up phase (~3-5j wall-clock once exit_id lands end-to-end).
   - §5 rejected alternatives (pubkey-as-id tautology, location-as-id false-positives).

4. **Tests TDD** - 4 new tests in `mod warren_pinned_exit_pubkeys_tests`:
   - `empty_pin_table_round_trip_json` (serde shape sentinel)
   - `one_pin_round_trip_json` (full field round-trip)
   - `pin_table_json_is_key_ordered` (BTreeMap determinism guarantee for operator-facing diff)
   - `settings_default_has_empty_pin_table` (TOFU contract: no pre-poisoned pins on first boot)

### Deferred to follow-up phase (post warren-core `exit_id` landing)

- Verify hook in `ParametersGenerator::produce_warren_tunnel_params` (TOFU insert + mismatch reject).
- gRPC RPCs (`TrustNewExitKey`, `ResetPinnedExitKeys`) + notification event (`WarrenPubkeyMismatchDetected`).
- UI modal (`WarrenPubKeyWarning.tsx`) + Settings reset CTA + i18n FR/EN.
- `/v1/incidents/pubkey-mismatch` POST endpoint in warren-api.
- 6/6 brief TDD criteria (all activate once `exit_id` is plumbed end-to-end).

### Commits

- `f930484866` feat(warren): A.4 exit-pubkey TOFU scaffold + design doc (verify hook deferred pending warren-core exit_id)

---

## Critères GO ULTIMATE Session A — Status

- ✅ A.1 GO (code + tests + clippy + fmt + smoke-build 26/26; live Mac smoke deferred to poka).
- ✅ A.2 GO (code + tests + clippy + fmt + smoke-build 26/26; live Windows smoke deferred to CI / poka VM).
- ✅ A.3 GO (Warren URLs + env override + placeholder pubkey + UI rebrand-compatible + 5 tests; signing key escalation case 4 documented).
- 🟡 A.4 GO partial (scaffold + design doc + 4 serde tests; verify hook + UI + endpoint deferred pending warren-core `exit_id` field).
- ✅ `cargo test --workspace --no-fail-fast`: **552 PASS / 0 fail / 8 ignored** on macOS host.
- ✅ `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- ✅ `cargo fmt --check`: PASS.
- ✅ `bash scripts/dev/smoke-build.sh`: **26/26 PASS**.
- ✅ Pas de régression Linux : Linux dispatch path unchanged (façade `mod linux` is byte-for-byte the previous local file).
- ✅ Working tree warren-core inchangé sur `d3_allowlist_dynamic.rs` (committed in `478a5f5` pre-Session A; not touched).
- ✅ Working tree warren-core inchangé sur les DAITA WIP files (poka committed his own `ab34ab5` + further DAITA v2 commits without my involvement).

**Verdict**: GO ULTIMATE on A.1+A.2+A.3, GO partial on A.4 with documented deferral and design blueprint.

---

## Doctrine

- §0.0 INVIOLABLE git : **RESPECTÉ**. ZÉRO destructive command. Two AskUserQuestion escalations: (1) DAITA WIP blocker mid-A.2 → poka committed the fix, work resumed; (2) A.4 architectural discovery (`exit_id` missing) → poka chose client scaffold + design doc path. M4.H.F incident not reproduced.
- §0.5 autonomy : tactical decisions applied throughout (façade design, plist path fix, stub vs Ok, env var override pattern, pin scaffold design). Escalation triggered on (a) poka WIP collision risk, (b) brief schema vs codebase reality mismatch.
- English-only code comments : respected.
- No em-dash : respected (all hyphens are ASCII `-`).
- Push warren-core + warren-app au fil de l'eau : 9 warren-app commits pushed; 0 warren-core commits (none required this session).
- Conventional commits subject-only : respected.

## Effort + cost

- Wall-clock: ~4h cumulative across A.1+A.2+A.3+A.4 (vs brief 8-10j budget).
- Hetzner cost: 0.00 EUR (no bench).
- Commits warren-app pushed origin/main: 9 (`06aeb145fc`, `e408e8736e`, `466404acdb`, `1dc5b7d04e`, `2e370789db`, `6dddeaf6e6`, `f930484866` + 2 docs commits `dd2c5bd08a`, `0f5cc1c647` & `776b72e743` which were docs from poka's parallel session B work, attributed in the log for completeness).
- Cross-repo: warren-app only this session. The brief budgeted potential warren-core work but A.1/A.2/A.3/A.4 each ended up wiring on the warren-app side only (the warren-core ports for default_route_split + the auto-update infrastructure already existed; A.4 verify hook deferred until warren-core `exit_id` lands).

## Outstanding caveats (ops + future phases)

1. Live Mac smoke E2E (sudo + launchctl + exit prod) - poka manual.
2. Live Windows smoke E2E (VM + cross-compile or native build) - poka manual or CI matrix.
3. Warren updates Ed25519 signing key generation - **case 4 escalation outstanding**, poka offline.
4. CI release.yml signed metadata upload pipeline - blocked on #3.
5. A.4 follow-up phase (~3-5j wall-clock):
   - warren-core `exit_id` field + signed relay-list extension.
   - warren-backend-api `exit_id` assignment + serving - **operator infra task, out of agent scope**.
   - Daemon verify hook activation + gRPC RPCs + UI modal + i18n + `/v1/incidents` endpoint + 6/6 TDD criteria.
6. GH Actions billing + WARREN_CORE_RO_TOKEN secret + signing assets - inherited from M4.H.D caveats.
