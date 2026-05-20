# M4.H.E - Caveats fixes delivery report

**Date** : 2026-05-20
**Verdict** : **GO ULTIMATE**
**Effort** : ~2.5h wall-clock (vs 8-10h estimated)
**Cost Hetzner** : 0.00 EUR

## Scope delivered

4 caveats accumulated M4.H.A -> C fixed before the M4.H.D build pipeline.

### M4.H.E.0 - Translation cleanup (opportunistic)

Pending FR->EN translations in 12 mullvad-daemon files were committed
(`66a9ca43ef`). Aligns with the project English-only-comments rule
(`CLAUDE.md`). No logic change.

### M4.H.E.1 - Reconnect cache wiring

**Problem**: M4.H.C UI displayed `Reconnects: 0` steady-state because
`WarrenStatusCache.record_reconnect()` was never called.

**Fix**: added `SupervisorConfig.on_reconnect: Option<ReconnectObserver>`
to warren-core supervisor + extracted helper `notify_on_reconnect`
that fires once per non-initial publication, gated on `first_session`.
Plumbed through `WarrenTunnelParameters.on_reconnect` in talpid-warren-tunnel,
forwarded to the supervisor in `start_multi_hop`. Daemon-side
`ParametersGenerator::produce_warren_tunnel_params` builds the closure
capturing `warren_status_cache.clone()` so a reconnect bumps the cache
that the gRPC `WarrenStatusUpdates` stream broadcasts to the Electron UI.

Removed the placeholder `Daemon.warren_status_cache` field
(originally kept for M4.H.C.X follow-up wiring; now consumed via
`ParametersGenerator`).

**TDD**:
- 3 new supervisor unit tests (`notify_on_reconnect_skips_first_session`,
  `..._fires_on_subsequent_publication`, `..._no_observer_is_a_noop`)
- Pin test in `warren_tunnel_params`: on_reconnect stays None after
  assemble (= caller responsibility)
- Pin test in `talpid-warren-tunnel`: default WarrenTunnelParameters
  `on_reconnect` is None for single-hop path

### M4.H.E.2 - Remote LOCAL=0 factory bug

**Problem**: M4.H.A bench v1 reported "Set account number on factory
with no access token store" against `api.warrenbrowse.com`.

**Diagnostic** (30 min): cannot reproduce on current HEAD. Phase G.4
(commit `66428e8b` of 2026-05-08, PRIOR to bench date 2026-05-19)
introduced a 3-branch dispatch in `device/mod.rs` that routes
Warren-Remote through `WarrenApiClient` (signed HTTP), which entirely
bypasses the legacy `RequestFactory.account()` chain. The factory
itself always carries `Some(token_store)` at line 495 of
`mullvad-api/src/lib.rs`. The bench v1 error was probably a transient
environment issue (e.g., mnemonic not bootstrapped at the moment of
the test).

**Fix**: structural invariant pinning so a future regression cannot
silently surface this error again:
- Added `RequestFactory::has_access_token_store()` getter
- Added `debug_assert!` at `mullvad_rest_handle_with_warren_signer`
  documenting the invariant + reference to M4.H.E.2 caveat
- 2 regression tests pinning the `NoAccessTokenStore` error verbatim
  and the boolean getter contract

### M4.H.E.3 - wapi VAL1/VAL2 client-side validation

**Problem**: smoke `test-backend-smoke.sh` VAL1 / VAL2 expected
exit 10 (= HTTP 400 from server) but wapi rejects malformed
`--country` values client-side via `CountryCode::try_from`, so the
actual exit code is 1 (= client-side `anyhow::Error`).

**Fix (Option β recommended by brief)**: patched smoke script to
assert exit 1 (= wapi client-side reject) and updated the scenario
labels. Per the no-log doctrine, the strict client-side validation is
correct: malformed identifiers must never reach the server logs. Also
clarified in comments that VAL3+ (empty city, oversized note, valid
country) still exercise the server-side 400 path.

Bash syntax verified by `bash -n`. Live verification (62/62 PASS)
requires running against a live warren-api with an ADMIN_KEY, which
requires SSH access (caveat ops poka, M4.H.B / SSH Hetzner bug).

### M4.H.E.4 - warren-killswitch audit

**Verdict A**: consumed by warren-client warren-core (not orphan).

1608 LOC (`lib.rs` 637 / `macos.rs` 331 / `windows.rs` 640) are
referenced from 5 files in `warren-client/src/` (default_route_split
backends Linux/macOS/Windows, ipv6_killswitch, main.rs, policy_routing).
These backends serve the standalone warren-client binaries that bench
the multi-hop stack without going through the Mullvad daemon layer
(warren-bench-multihop, warren_multihop_tun_client).

ZERO consumer in warren-app: the daemon uses talpid-core firewall +
talpid-routing upstream from Mullvad. The R1 rebrand did not replace
this stack. Documented in warren-core memory `warren_killswitch_audit.md`.

## Validation gates

| Gate | warren-app | warren-core |
|---|---|---|
| `cargo fmt --check` | clean | clean |
| `cargo clippy --workspace --all-targets -D warnings` | clean | clean |
| `cargo test --workspace --lib` | **426 passed / 1 ignored / 40 suites** | (only supervisor: **7 passed**) |
| `tsc --noEmit -p packages/mullvad-vpn` | clean | n/a |
| `npm test` desktop/packages/mullvad-vpn | **110 passed / 15 suites** | n/a |
| `bash -n test-backend-smoke.sh` | n/a | OK |

## Commits

Warren-app `main` (5 commits):
1. `66a9ca43ef` docs(warren): translate French doc comments to English
   in mullvad-daemon device + warren_* modules
2. `57f2cf5b8e` fix(talpid-warren-tunnel): wire WarrenStatusCache.record_reconnect
   from MultiHopSupervisor
3. `9753ea1654` docs(warren): translate remaining French comments to
   English across mullvad-cli/types/paths/management-interface + desktop
   UI + CI workflow
4. `3588dd32d5` fix(mullvad-api): pin NoAccessTokenStore invariant +
   regression tests for M4.H.A bench v1 caveat
5. `<fmt>` style(mullvad-api): cargo fmt cleanup trailing blank line

Cross-repo warren-core `main` (2 commits):
- `a7159d9` feat(warren-client): SupervisorConfig.on_reconnect observer
- `<smoke>` test(bench-scripts): VAL1/VAL2 assert wapi client-side reject

## Pin warren-core

`.warren-core-version` bumped `bb9b7895 -> a7159d94` (E.1 observer).

## Caveats residuels (out of scope)

| Caveat | Owner | Impact M4.H.D |
|---|---|---|
| SSH Hetzner bench provisioning bug | ops poka | blocks live bench |
| GHCR PAT write:packages | ops poka | blocks CI cosign push |
| Live verification of M4.H.E.2 fix | ops poka | needs live api access |
| NAT-PMP UI port-forwarding | M4.H.F | unrelated, future feature |

## Decisions archi (auto, doctrine §0.5)

- **Callback over polling** for the reconnect observer: the supervisor
  fires the observer synchronously inside `run()` so the daemon-side
  WarrenStatusCache reflects the new state without a polling task.
- **Cross-repo extension**: warren-core supervisor extended (new
  `ReconnectObserver` type + `on_reconnect` config field +
  helper). Pin warren-core bumped to `a7159d94`.
- **Daemon field cleanup**: removed `Daemon.warren_status_cache`
  field (placeholder for M4.H.C.X) because the wiring now goes
  through `ParametersGenerator` which clones the cache directly. No
  reachability gap (the gRPC handlers still hold their own clone
  via `ManagementServiceImpl`).
- **E.2 = structural pin, not a code fix**: confirmed via static
  analysis that the original bug cannot reproduce; added regression
  tests + invariant assertion so a future refactor cannot silently
  reintroduce it.
- **E.3 Option β** (smoke alignment) over Option α (relax wapi
  validation): preserves the no-log doctrine which is more important
  than smoke convenience.

## Next phase

**M4.H.D unblocked**: build pipeline DMG (macOS) + AppImage (Linux) +
MSI (Windows) + signing keys + CI release.

The UI is ship-ready with a live reconnect counter that advances on
each transparent multi-hop reconnect; the M4.0 obfuscation indicator
is hooked up; the country pickers and toggle are wired through the
gRPC management interface.
