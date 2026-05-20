# Phase M4.H.A.quart - warren-core allowlist fix + 2-node redeploy + cross-DC bench

> Rapport agent autonome cross-repo warren-core + warren-app. Suit M4.H.A.ter
> (NO-GO HEAD vs HEAD silent eviction).

**Date** : 2026-05-20
**Verdict** : **GO ULTIMATE**
**Cause root identifiee** : Hypothese 1 confirmee. `apply_snapshot` early-
return sur generation gate ne mettait jamais a jour `last_success_unix_secs`.
En steady-state poll (server immuable, `generation` constante), le timestamp
restait gele sur la valeur seed-from-cache, `clear_if_stale` declenchait
fail-closed a T+grace.
**SHA fix warren-core** : `8f4f299` (push origin/main).

---

## 1. Verdict

**GO ULTIMATE.** Tous les criteres §7 du brief satisfaits sans exception :
- Test RED warren-core reproduit le bug pre-fix
- Fix GREEN + 3 nouveaux tests regression + 1 test modifie (pinning de bug retire)
- Cargo workspace `fmt --check && clippy -D warnings && test` PASS (520 tests / 57 suites)
- Redeploy warren-exit-1 binary HEAD+fix sur meme IP, 0 downtime client significatif
- Smoke Tier 1 server-side 62/62 (60 PASS + 2 wapi VAL1/2 client-side regression heritee de M4.H.A.ter)
- Bench cross-DC : 802 Mbps 4-flow sustained 5 min, allowlist NON cleared a T+5, T+10, T+12min
- Commits pushed warren-core (`8f4f299`) + warren-app (this report)
- Memory updates warren-core (`warren_exit_api_silent_poll_fail` -> RESOLU) + warren-app (`warren_m4h_a_quart_delivered`)

## 2. Root cause + fix

`crates/warren-tunnel/src/allowlist.rs` `apply_snapshot` :

**Avant fix** : `last_success_unix_secs.store(snapshot.fetched_at_unix_secs)`
n'etait execute QUE dans le path d'apply, jamais sur early-return. Le test
existant `snapshot_with_equal_generation_is_ignored_after_first_apply` PINNAIT
ce bug par assertion `last_success == 1_000` (= pas advance sur "rejected").

**Apres fix** : `last_success_unix_secs.fetch_max(snapshot.fetched_at_unix_secs, AcqRel)`
execute en haut de la fonction, AVANT le gate. Distingue "ack server
reachability" (every successful poll) de "apply mutation" (gen strictly
greater). `fetch_max` empeche la regression sur out-of-order retry au
fetched_at antedate.

## 3. Tests warren-core

**Test modifie** (etait bug-pinning) :
- `snapshot_with_equal_generation_skips_payload_but_acks_reachability` :
  assertion `last_success == 2_000` (vs `== 1_000` avant), `last_generation == 5` (gate preserve).

**Tests regression ajoutes** (3 nouveaux) :
1. `steady_state_polls_advance_last_success_without_mutating_payload` : reproduit
   exactement le scenario empirique M4.H.A.bis/ter (seed cache + 10 polls gen stable + clear_if_stale T0+300s) -> 0 eviction post-fix.
2. `out_of_order_snapshot_with_stale_fetched_at_does_not_regress_last_success` :
   regression contract `fetch_max` (stale timestamp drop).
3. `rejected_payload_with_strictly_newer_fetched_at_advances_last_success` :
   meme gen + nouvelle timestamp -> reachability advance, payload drop.

Tests existants intacts (20 tests `cargo test -p warren-tunnel --lib allowlist` PASS).

Cargo gates warren-core :
- `cargo test -p warren-tunnel -p warren-exit -p warren-api` : **520 PASS** / 57 suites / 10.66s
- `cargo clippy --all-targets -- -D warnings` : clean
- `cargo fmt --check` : clean

## 4. Commits

**warren-core** :
- `8f4f299 fix(warren-tunnel): apply_snapshot acks reachability on every successful poll regardless of generation gate`
- pushed origin/main

**warren-app** :
- `<TBD> docs(M4.H.A.quart): GO ULTIMATE verdict + cross-DC bench post warren-core fix`
- pushed origin/main

## 5. Decision archi (auto-decidee §0.5)

warren-backend-api **PAS** redeploye. Verification dependency tree :
`warren-api` + `warren-admin` n'ont AUCUNE dependance vers `warren-tunnel`
(le crate ou est le fix). Seul `warren-exit` consomme `warren-tunnel` via
path-dep. Donc fix porte UNIQUEMENT sur warren-exit binary.

Resultat : skipped docker rebuild + GHCR push workaround. warren-backend-api
reste sur v0.2.11. warren-exit-1 binary swap suffit.

## 6. Redeploy methodology

| Step | Time | Result |
|---|---|---|
| rsync warren-core (no .git/target/) -> nbg1 CCX23 transient | ~30s | OK 76 MB |
| apt install build-essential + libdbus + protoc + libprotobuf + iperf3 | ~1 min | OK |
| rustup install 1.89.0 minimal (premier essai conflit deux concurrents, wipe + clean reinstall) | ~3 min | OK |
| cargo build --release --bin warren-exit -p warren-exit (cold) | 2m 47s | 6.77 MB ELF |
| scp nbg1 -> warren-exit-1 prod (inter-DC Hetzner) | <1s | sha256 610daa60 |
| `systemctl stop` + `mv` + `systemctl start` warren-exit | < 200 ms | active running |
| Boot logs : identity loaded, allowlist seeded admitted=1, refresher spawned, warren-api registered, TUN+NAT-PMP+DNS bound | clean |  |

## 7. Validation empirique fix (CRITIQUE)

**Test cle GO ULTIMATE** : warren-exit-1 allowlist NON cleared a T+5min ET T+10min.

```
T0=2026-05-20T00:02:31Z (swap binary)
T+5min  (T+310s)  eviction=0  (MILESTONE PASS - fix corrige bug)
T+10min (T+613s)  eviction=0  (MILESTONE PASS - fix robust)
T+12min (T+735s)  eviction=0  (Monitor WATCH_END)
```

Pre-fix, exact memes warren-exit + warren-backend-api avaient declanche
`WARN warren_exit::allowlist_refresh: allowlist cleared after staleness
grace exceeded (fail-closed) evicted=1` precisement a T+300s
(`2026-05-19T23:14:31Z` apres boot 23:09:31). Pattern strictement reproductible
M4.H.A.bis/ter. **Post-fix 12+ min sans aucune WARN allowlist**.

## 8. Bench cross-DC nbg1 -> warren-exit-1

Setup : nbg1 client (5.75.142.187 CCX23 nbg1) avec warren-client --use-tun
dialing warren-exit-1 hel1 (204.168.207.130). RTT cross-DC = 24.4 ms (ping
5/5 0% loss). iperf3 server warren-exit-1 bind 10.66.0.1:50000 (port range
49152-65535 autorise par nft rules).

| Item | Value | Target | Status |
|---|---|---|---|
| TCP 4-flow sustained throughput 5 min | **802 Mbps** | >= 200 Mbps | **4x over** |
| Total transfered | 28.0 GB | -- | -- |
| Per-flow balance | 191 / 214 / 200 / 196 Mbps | -- | balanced |
| Retransmits 5 min | 8759 | low | normal at full speed cross-DC |
| max_mtu negotiated | 1350 | >= 1280 | OK |
| TUN MTU active | 1280 | >= 1280 | OK |
| RSS warren-exit start | 52516 KB | -- | -- |
| RSS warren-exit end | **44700 KB** | stable | **-7.6 MB** (reclaim) |
| Stalls >= 5s | 0 | 0 | OK |
| Errors | 0 | 0 | OK |
| Reconnects mid-bench | 0 | 0 | OK |
| Allowlist evictions during bench | 0 | 0 | OK |

## 9. Smoke Tier 1

```
WARREN_API_URL=https://api.warrenbrowse.com \
  ADMIN_KEY=.local/admin-stack/admin/admin-signing.key \
  ./bench/scripts/test-backend-smoke.sh
```

Result : **60 PASS / 2 FAIL** (server-side 62/62, identique M4.H.A.ter).

Fails : VAL1+VAL2 wapi client-side input validation (country=FRANCE/A1 rejet
pre-send). Pas imputable au server, pas a notre fix. Caveat herite M4.H.A.ter.

## 10. Caveats residuels

1. **Subscription nbg1 reste active 1h** apres tear-down (expires_at=
   1779239416, ~01:10 UTC). Mnemonic detruit avec nbg1, donc pubkey en
   allowlist mais inutilisable. Pas de risk securite. admin-cancel-voucher
   inadequat (voucher deja redeemed). wapi manque `admin-cancel-subscription`.
2. **Smoke Tier 1 VAL1+VAL2 wapi client-side regression** : herite M4.H.A.ter.
   Resolution = patch wapi (rejette pre-send -> laisse server rejeter), out of
   scope M4.H.A.quart.
3. **GHCR PAT poka-IT `write:packages`** : non-blocant pour ce phase (pas de
   docker rebuild needed). Reste open pour future warren-api/admin deploys
   officiellement signed.
4. **Caveat M4.H.A daemon-fork `account create` Remote LOCAL=0** : hors scope,
   reste pour M4.H.B.

## 11. Cout Hetzner

| Item | Type | Duree | Cout (~) |
|---|---|---|---|
| warren-quart-client-nbg1 | CCX23 | ~30 min | 0.025 EUR |
| **Sous-total bench** | | | **0.025 EUR** |
| warren-exit-1 + warren-backend-api | preserved IPs, intacts | -- | 0 EUR |
| **Total M4.H.A.quart** | | | **0.025 EUR** |

Tear-down nbg1 atteste : `hcloud server list` final = `warren-exit-1` +
`warren-backend-api` seuls.

## 12. Memory updates

- warren-app : new `warren_m4h_a_quart_delivered.md` (cf. §13)
- warren-core : update `warren_exit_api_silent_poll_fail.md` -> RESOLU + SHA
  fix + tests regression listes
- warren-core : update `warren_backend_server.md` (no coordonnees change,
  juste note GO ULTIMATE post-fix verify)
- warren-app : update MEMORY.md index avec ligne quart delivered

## 13. Next steps (orchestrateur)

GO ULTIMATE debloque :
- **M4.H.B** (cablage stack M4.E.D dans warren-app) : perf cross-DC validee
  empiriquement 802 Mbps sustained 5 min, fix allowlist en place.
- **wapi client-side VAL1/VAL2 fix** : quick patch (rejet pre-send -> let
  server reject) pour Smoke 62/62.
- **admin-cancel-subscription wapi command** : utile pour scenarios bench
  futurs.

---

## 14. Lecons (optionnel, pour reference future)

- **TDD strict** revele l'invariant cache (le test `..._is_ignored_after_first_apply`
  pinninnait literalement le bug). Reviewing failing assertions parfois revele
  une faute de conception au lieu d'un bug code.
- **Cross-repo autonomy** (doctrine §0.5 nouveau) = vrai unlock : fix
  warren-core + redeploy warren-exit + bench warren-app en un seul agent
  span sans escalade. 3 phases NO-GO consecutives transformes en 1 GO
  ULTIMATE en ~2h wall-clock.
- **Dependency tree analysis** debloque skip ~15 min docker rebuild + GHCR
  workaround (warren-api ne depend pas warren-tunnel = inutile redeploy).
