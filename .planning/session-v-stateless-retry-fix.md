# Session V — Loopback test helpers loop over StatelessRetryIssued

> Status : **GO ULTIMATE** — full warren-tunnel test suite green post-fix
> Date : 2026-05-22
> Cost réel : **0 EUR** (in-process only, "hors bench" respecté)
> §0.0 INVIOLABLE git respecté. Production warren-exit-1 + warren-backend-api intacts.

---

## TL;DR

Session N+ added `incoming.retry()` for stateless retry on un-validated remotes (AUDIT-2026-05-21 P1 Phase3.19, prod-side defence against UDP source spoofing). The accept loop in `serve_forever` already swallows the `StatelessRetryIssued` marker and continues; but the in-process **test helpers** `accept_one_handshake_for_test`, `accept_and_capture_one_datagram`, `echo_one_datagram_with` didn't — they returned the marker as an error, leaving loopback test clients (whose first packet always lands on an unvalidated remote address) hitting a spurious `connect timeout` after the retry token round-trip.

Session V routes all three helpers through a new private `handshake_only_retrying` async fn that loops silently over `StatelessRetryIssued` (with the same 30 s outer timeout as before) so they now mirror production behaviour.

---

## Bug surface

| Test suite | Failing tests | Failure mode |
|---|---|---|
| `crates/warren-tunnel/tests/e2e_handshake.rs` | `client_can_send_datagram_through_tunnel`, `datagram_round_trips_bidirectionally` | `exit accept: StatelessRetryIssued` + downstream `connect timeout` |
| `crates/warren-tunnel/tests/daita_sustained_stress.rs` | `daita_pump_survives_5s_low_pps`, `daita_pump_survives_5s_mid_pps` | `exit handshake failed: StatelessRetryIssued` |
| `crates/warren-tunnel/tests/d3_allowlist_dynamic.rs` | `close_connections_for_terminates_a_live_session_for_revoked_pubkey` | handshake timeout (cascading from accept fail) |

Root cause for all three : `incoming.remote_address_validated()` returns `false` for the first dial on a loopback test pair, so `handshake_only` issues a retry token and returns `Err(StatelessRetryIssued)`. The honest client retries, but the test helper has already returned the error to the caller.

---

## Fix (commit warren-core `f0be037`)

`crates/warren-tunnel/src/exit.rs` :

- New private `async fn handshake_only_retrying(&self) -> Result<(quinn::Connection, WarrenPubkey)>` — wraps `handshake_only` in a retry loop bounded by the same 30 s outer deadline. Returns on first non-retry result (success or other error); silently continues on `StatelessRetryIssued`.
- `accept_one_handshake_for_test`, `accept_and_capture_one_datagram`, `echo_one_datagram_with` now call `handshake_only_retrying` instead of `handshake_only`.

Production code paths (`serve_forever`, `accept_one`) are unchanged — the retry-loop logic was already in their accept loops.

---

## Validation

| Test suite | Pre-V | Post-V |
|---|---|---|
| `e2e_handshake` | 6/8 PASS (2 retry-failures) | **8/8 PASS** |
| `daita_sustained_stress` (excl. `--ignored`) | 0/2 PASS (StatelessRetryIssued) | **2/2 PASS** |
| `d3_allowlist_dynamic` | 2/3 PASS (1 timeout) | **3/3 PASS** |
| Full warren-tunnel test suite | 87+ passing with 2-3 flaky | **All test suites GREEN** |
| `cargo clippy --release -p warren-tunnel --tests -- -D warnings` | CLEAN | CLEAN |

---

## Pin warren-app

`.warren-core-version` : `067d21c` (Session U) → `f0be037` (Session V).

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : targeted test-helper fix, no over-extension into production retry semantics (already correct)
- §0.6 worktree skipped (single targeted change)

## Next steps (hors bench)

1. **Pump-side blocking enforcement** : queue real packets during `BlockOutgoing` window (full Tamaraw "block + pad" defense property — currently `bypass: true` lets real packets through alongside padding).
2. **Multi-hop IP negotiation v1 multi-client** : replace POC `10.66.0.2/24` hardcoded in `run_multi_hop_with_tun`.
3. **DAITA UI/docs announce** : surface defense status in desktop UI + landing page.
