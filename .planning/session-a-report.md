# Session A — Cross-platform parity report (partial)

> Status: **A.1 delivered**, A.2/A.3/A.4 **paused on poka directive** until warren-core DAITA WIP daita.rs resolves.

**Verdict**: A.1 GO (code path complete + tests + smoke build, daemon-side macOS Mac smoke E2E deferred to poka manual since session A agent doesn't drive sudo route installs on the user's live host).

---

## A.1 — macOS daemon wiring + smoke E2E — DELIVERED

### Stack câblée

1. **Cross-OS façade** — `talpid-warren-tunnel/src/default_route_split.rs` rewritten as a thin per-OS dispatcher:
   - `target_os = "linux"`: in-crate impl preserved in `default_route_split/linux.rs` (the previous local Linux-only file, byte-for-byte) so the existing 10 unit tests on `ip rule` shape still anchor the recipe.
   - `target_os = "macos"`: `pub use warren_client::default_route_split_macos::DefaultRouteSplitGuard;` (warren-core port, host-route exception + `0.0.0.0/1` + `128.0.0.0/1` via `route add -interface <tun>`).
   - `not(any(linux, macos, windows))`: `default_route_split/stub.rs` returns `Err` from `install` so the operator log surfaces the absent routing while the `default_route_guard: Option<...>` field still type-checks.
   - 1 new facade test `api_surface_matches_lib_rs_call_site` pins the `install(Ipv4Addr, &str) -> Result<Self>` shape so an upstream signature drift in the warren-core macOS port fails at compile time inside this crate (not at the lib.rs call site).

2. **Cfg dispatch in `lib.rs`** — both the single-hop (line 683) and multi-hop (line 1061) install paths now use `#[cfg(any(linux, macos))]` (will expand to `linux, macos, windows` once A.2 lands). The body is OS-agnostic since the façade exposes identical signatures. Cleanup branch tightened to `#[cfg(not(any(linux, macos)))]` so non-supported targets retain `default_route_guard: None`.

3. **pfctl killswitch coexistence (A.1.2)** — **already wired pre-Session A**. The `FirewallPolicy::Connecting.peer_endpoints` (consumed by `talpid-core/src/firewall/macos.rs`) is populated from `BackendParams::Warren.get_next_hop_endpoints()` in `backend_params.rs:50-65`:
   - Single-hop: every candidate IP of the exit (`exit_addr.ip_addrs()`).
   - Multi-hop: only the relay endpoint.

   No code change needed. The connection state machine already authorizes the Warren client → exit UDP socket through pf when entering Connecting, exactly like it does for WireGuard peer endpoints.

4. **LaunchDaemon plist path Warren-branded** — `mullvad-daemon/src/macos_launch_daemon.rs:14` had `DAEMON_PLIST_PATH = "/Library/LaunchDaemons/net.mullvad.daemon.plist"` while the M4.H.D pkg-scripts (`dist-assets/pkg-scripts/{pre,post}install`) install the plist at `/Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist`. Fixed Rust constant + module rustdoc + the postinstall cross-reference comment that pointed at a non-existent `warren-daemon/src/...` (the actual crate is `mullvad-daemon/`). Without this fix, `SMAppService::statusForLegacyURL(...)` would always return `NotFound` on macOS 13+ even when the daemon is running.

### Validation

- `cargo check --workspace`: PASS (at A.1 commit time; subsequent poka WIP DAITA daita.rs broke this for A.2 onward — see Pause section below).
- `cargo test -p talpid-warren-tunnel`: PASS (34/34 = 33 existing local Linux tests now under `mod linux` + 1 new facade test).
- `cargo test --workspace` on macOS host at commit time: PASS (543 / 0 fail / 8 ignored). The 9-test delta vs M4.H.G's 552 is structural: the 10 Linux-impl unit tests moved into `mod linux` which is `cfg(target_os = "linux")`, so they no longer run on macOS host but still anchor the Linux build. Net change: -10 Linux-gated + 1 cross-OS facade test = -9 on macOS host, expected.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo fmt --check`: PASS (no diff once nightly-only feature warnings filtered).
- `bash scripts/dev/smoke-build.sh`: **26/26 PASS**.

### Décisions tactiques §0.5 retenues A.1

- Façade design via cfg-gated `pub use` (not a wrapper enum) keeps the `default_route_guard: Option<DefaultRouteSplitGuard>` field type identical across OS targets and means lib.rs install branches stay a single `#[cfg(any(...))]` block instead of a per-OS fanout. Future Windows port slots into the same place with one extra `pub use` line.
- The warren-core macOS port (`default_route_split_macos`) is wired **in addition to** the existing `build_warren_tunnel_routes_macos` ifscope dance via `talpid-routing::add_routes`. Both coexist: talpid-routing posts the bypass `<exit_ip>/32 via DefaultNode` first, the warren-core port adds the more specific `<exit_ip>/32 -interface <default_iface>` (= host route) — the second install hits `File exists` and is tolerated. The `0.0.0.0/1` + `128.0.0.0/1` from the warren-core port then wins over talpid-routing's `0.0.0.0/0 dev <tun>` ifscope route on prefix-length match. No route table corruption observed in cross-OS test fixtures; behaviour pending smoke E2E on a live Mac (see caveats).
- Stub `install` returns `Err` rather than `Ok(())` so a future port not yet wired surfaces immediately in the operator log instead of silently dropping default-route capture.

### Smoke E2E A.1.3 — DEFERRED to poka manual

The agent does not drive sudo route installs against the user's live host network nor exercise the launchd flow against `warren-exit-1` prod. Mac smoke checklist staged for poka:

1. `cargo build --release` warren-app on Mac → produces `warren-daemon` + `Warren VPN.app`.
2. `launchctl load -w /Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist` (postinstall already wires this).
3. UI Electron connect to `warren-exit-1` (FR Hetzner prod, 91.99.122.154).
4. DNS leak: `dig @1.1.1.1 example.com` → should resolve via tunnel.
5. WebRTC leak: `https://browserleaks.com/webrtc` → IP exit shown.
6. `curl ifconfig.me` → exit IP, not host.
7. `--bypass-cidr 192.168.0.0/16` flag (M4.H.G) → SSH inbound from LAN preserved while tunnel is up.
8. NAT-PMP qBittorrent: open external port + telnet from outside.
9. Suspend/resume Mac → auto-reconnect within 15s (M4.E.D + M4.H.G `Backoff::HANDSHAKE` 15s ceiling).

### Commits pushed origin/main (warren-app)

- `06aeb145fc` refactor(talpid-warren-tunnel): make default_route_split a cross-OS facade (linux/macos/stub)
- `e408e8736e` feat(talpid-warren-tunnel): dispatch macOS through default_route_split facade (A.1.1)
- `466404acdb` fix(mullvad-daemon): align macOS LaunchDaemon plist path with Warren packaging

No warren-core commits required for A.1 (the macOS port pre-existed in warren-core HEAD `68617cde`+ which is already pinned).

---

## A.2 — Windows daemon wiring + smoke E2E — PAUSED

### Reason

`warren-core` working tree has 5 dirty files mid-DAITA wiring (poka WIP, ahead of origin/main on commit `b4c9faa`):

- `Cargo.lock` (maybenot 2.2.2 + maybenot-machines 1.0.1 added)
- `Cargo.toml`
- `crates/warren-tunnel/Cargo.toml`
- `crates/warren-tunnel/src/error.rs`
- `crates/warren-tunnel/src/lib.rs`
- `crates/warren-tunnel/src/daita.rs` (UNTRACKED)

The untracked `daita.rs:303` calls `.map(daita_action_from_maybenot)` over an iterator of `&TriggerAction`, but `daita_action_from_maybenot` accepts `TriggerAction` by value. `cargo check --workspace` fails with `E0631` / `E0599`. The fix is mid-flight — the surrounding rustdoc was updated in real time during this session to describe a "destructure each one by reference and copy out primitive fields" approach, indicating poka is actively refactoring the function.

### Pre-staged A.2 in working tree (not committed)

I staged the trivial cross-OS plumbing for Windows in case poka wants to reuse it post-DAITA unblock. These edits sit uncommitted in the working tree (§0.0 INVIOLABLE prevented discarding them):

- `talpid-warren-tunnel/src/default_route_split.rs` — add `#[cfg(target_os = "windows")] pub use warren_client::default_route_split_windows::DefaultRouteSplitGuard;` + tighten the stub fallback `cfg(not(any(linux, macos, windows)))`.
- `talpid-warren-tunnel/src/lib.rs` — broaden the install cfg blocks (both single-hop line 683 + multi-hop line 1061) to `#[cfg(any(linux, macos, windows))]`, add `#[cfg(target_os = "windows")] let routes: Vec<RequiredRoute> = Vec::new();` (let the warren-core PowerShell port own all Windows routing, no talpid-routing double-install).

These are safe no-ops on macOS host (cfg expands to existing behaviour). They activate only on `target_os = "windows"`.

### Resume conditions

- poka commits + pushes the DAITA fix (or another resolution that makes `cargo check --workspace` pass on warren-core HEAD).
- Re-run `cargo check -p talpid-warren-tunnel` to confirm the WIP edits still compile against the new warren-core HEAD.
- Run Windows-target build (`cargo check --target x86_64-pc-windows-msvc` cross OR native on Windows VM).
- WFP killswitch coexistence: same `BackendParams::Warren.get_next_hop_endpoints()` flows through `FirewallPolicy::Connecting.peer_endpoints` consumed by `talpid-core/src/firewall/windows/`. No code change expected (mirror of macOS pf finding).
- Windows service install + WinTUN driver + smoke E2E checklist same as A.1.3 (defer live smoke to poka).

---

## A.3 — Auto-update prod-grade — PAUSED

Depends on A.2 completion per Session A brief sequencing. Resume after DAITA WIP unblocked.

Pre-flight reading already done on `mullvad-update/` crate; will pick up from:
- URL update server (decision tactique: GitHub Releases API vs self-hosted CDN — leaning GH Releases per M4.H.D pipeline + 0 EUR infra).
- Ed25519 signing key (CASE 4 ESCALATION if Warren updates key not yet provisioned; CSC signing key from M4.H.D is distinct).
- Single channel `beta` until 1.0 stable.

---

## A.4 — Pinning pubkey exit TOFU + UI warning — PAUSED

Depends on A.2 + A.3 completion. Resume after DAITA WIP unblocked.

Pre-flight design notes:
- Storage: sqlite warren-tunnel (re-use existing warren-api-client sqlite infra) keyed by `exit_id`.
- Mismatch → refuse connect + emit event UI via gRPC.
- New endpoint `/v1/incidents/pubkey-mismatch` (warren-api stub, log-only).
- UI `WarrenPubKeyWarning.tsx` modal Warren-branded + i18n FR/EN, 3 CTAs (Trust / Reject / Report).
- Settings "Reset pinned exit keys" CTA with confirmation modal.

---

## Critères GO ULTIMATE Session A — Status

- ✅ A.1 GO (code + tests + clippy + fmt + smoke-build 26/26; live Mac smoke deferred to poka).
- ⏸ A.2 PAUSED (DAITA WIP blocker).
- ⏸ A.3 PAUSED (depends on A.2).
- ⏸ A.4 PAUSED (depends on A.2 + A.3).
- ✅ `cargo test -p talpid-warren-tunnel`: 34 PASS at A.1 commit time.
- ✅ `cargo test --workspace`: 543 PASS at A.1 commit time (broke post-A.1 commit due to poka WIP, unrelated to my changes).
- ✅ `cargo clippy --workspace -D warnings`: PASS at A.1 commit time.
- ✅ `cargo fmt --check`: PASS.
- ✅ `scripts/dev/smoke-build.sh`: 26/26 PASS.
- ✅ Pas de régression Linux : Linux dispatch path unchanged (façade `mod linux` is byte-for-byte the previous local file).
- ✅ Working tree warren-core inchangé sur `d3_allowlist_dynamic.rs` (file committed since brief writing in `478a5f5`, not touched).
- ⚠ Working tree warren-core has 5 newer dirty files (DAITA WIP, post-brief) — preserved untouched per §0.0 INVIOLABLE.

**Verdict**: GO PARTIEL (A.1 only). A.2/A.3/A.4 pending poka WIP DAITA resolution. Resume directive received via AskUserQuestion ("Pause A.2/A.3/A.4, A.5 rapport partiel A.1").

---

## Doctrine

- §0.0 INVIOLABLE git : **RESPECTÉ**. ZÉRO destructive command. Discovered poka WIP daita.rs blocker via read-only `git status`, escalated via AskUserQuestion rather than touch the WIP. M4.H.F incident not reproduced.
- §0.5 autonomy : tactical decisions applied (façade design, plist path fix, stub vs Ok). Escalated on poka WIP conflict per spirit of "Si tu touches [poka WIP] comme effet de bord, escalade" (brief §0.0 closing note).
- English-only code comments : respected.
- No em-dash : respected.
- Push warren-core + warren-app au fil de l'eau : A.1 commits pushed (3 commits warren-app origin/main).

## Effort + cost

- Wall-clock: ~1.5h (under brief 2-3j A.1 budget).
- Hetzner cost: 0.00 EUR (no bench).
- Commits: 3 warren-app pushed origin/main. 0 warren-core (no warren-core changes required for A.1).
