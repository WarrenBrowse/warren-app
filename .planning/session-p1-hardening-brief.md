# Session P1-Hardening, Audit residus P1 + pre-prod GA hardening

> Brief d'agent autonome warren-core (surface principale) + warren-app (mineur).
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Grosse session : ~22 items P1 à attaquer par priorité descendante. 4-6 semaines wall-clock budget.

**Effort estimé** : wall-clock 4-6 semaines (gros volume).
**Coût Hetzner** : ~0.10 EUR (smoke tests éventuels post-fix).
**Pré-conditions** :
- warren-app `main` HEAD `eced6c8613+`
- warren-core `main` HEAD `fed1c88+`
- docs/AUDIT-2026-05-21.md ~630 lignes structuré P0/P1/P2 lisible

**Objectif** : clôturer les items P1 résiduels de l'audit pre-prod 2026-05-21 pour permettre l'ouverture commerciale GA. Phase 1 (10/10 P0) + Phase 2 (8/9) + Phase 3 (8/8 P1) + Phase 4 (P1+P2) + Phase 5 external-blockers (5/5) déjà livrés. RESTE ~22 items P1 + 1 item Phase 2 (/metrics + root CancellationToken).

Sous-phases (séquentielles autonomes, par priorité descendante) :

1. **P1.0, Setup worktree warren-core + warren-app dédiés** (~30 min)
2. **P1.1, Phase 2 résidus : /metrics Prometheus + root CancellationToken warren-exit** (~3-5j)
3. **P1.2, Sécurité P1 résidus (6 items)** (~5-7j)
4. **P1.3, Architecture P1 résidus (7 items)** (~5-7j)
5. **P1.4, Qualité Rust P1 résidus (~11 items)** (~7-10j)
6. **P1.5, Tests P1 ciblés (9 items)** (~3-5j)
7. **P1.6, Performance P1 (2 items)** (~2-3j)
8. **P1.7, Production readiness P1 (résidus 17 items)** (~7-10j)
9. **P1.8, Rapport final + memory + cleanup** (~1j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard. Préserver fichiers modified/untracked.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si :
1. Secret leak
2. Coût Hetzner > 0.30 EUR
3. Breaking change /v1 wire format ou gRPC
4. Signing key prod touchée
5. **Spécifique P1-hardening** : si un item P1 nécessite redeploy prod warren-exit-1 + warren-backend-api lockstep (cf. Session E pattern), escalader pour coordination

Décisions tactiques agent autorisées :
- Ordre de traitement intra-priorité (sec → archi → qualité OU par crate par cohérence diff)
- Test pattern : TDD strict RED → GREEN par item, ou batch puis tests fin sous-phase
- Skip items résiduels déjà partiellement livrés Phase 2/3/4 (lire memory `warren_audit_2026_05_21` pour status à jour)

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-p1-hardening main
cd ../warren-core-p1-hardening

# Plus worktree warren-app si modif warren-app nécessaire
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-p1-hardening main
```

Cleanup en fin :
```bash
git worktree remove ../warren-core-p1-hardening
git worktree remove ../warren-app-p1-hardening
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-core
git fetch origin
git worktree add ../warren-core-p1-hardening main
cd ../warren-core-p1-hardening
git log --oneline -5

# Read audit doc + memory récents
cat docs/AUDIT-2026-05-21.md  # ~630 lignes structuré
ls /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-core/memory/  # status à jour
```

Lire en parallèle `warren_audit_2026_05_21.md` + `warren_phase5_external_blockers_done.md` (memory warren-core) pour status à jour. Phases 1-5 livrées partiellement, ne pas redoubler.

---

## 2. Items P1 à attaquer (extraits docs/AUDIT-2026-05-21.md §1-§7)

### P1.2, Sécurité (6 items)

1. **bind explicite** : warren-api bind `0.0.0.0` vs `127.0.0.1` config explicite
2. **legacy SNI** : drop legacy SNI patterns inutiles dans warren-tls
3. **enroll rate-limit** : per-IP token bucket sur `/v1/enroll` warren-api
4. **AEAD nonce assert** : warren-multihop assert nonce non-zero pour debug builds
5. **DAITA `Box::leak`** : warren-tunnel daita.rs supprimer leak intentionnel + lifecycle propre
6. **TLS resolver expect** : warren-tls expect → unwrap_or_else avec context

### P1.3, Architecture (7 items)

1. **ExitId dual** : nettoyage cross-crates post-Session E (verifier pas de duplication ExitId)
2. **TUNNEL_IDLE_TIMEOUT mort** : code mort warren-tunnel à supprimer
3. **pub bench** : warren-client::bench module exposition publique vs internal
4. **DTO `String→PubkeyHex`** : Phase 2 a fait 6 fields wire-critiques, finir les 4-5 restants
5. **short_pubkey dup** : helper `short_pubkey` dupliqué cross-crates, factoriser warren-protocol
6. **fdwa stale** : code `fdwa` (firewall daita?) potentiellement stale à audit/clean
7. **E2E CI gap** : test E2E warren-client + warren-exit pas dans CI matrix

### P1.4, Qualité Rust (11 items)

1. **lock-across-await** ios_tun + real_tun (cf. Session 5.B `IosTun::pair()` déjà fait, vérifier real_tun reste)
2. **clone ciphertext multihop** : warren-multihop hot path, Bytes::clone OK mais audit
3. **format!/clone pump** : warren-tunnel pump hot path allocations
4. **thiserror erase** : `#[error(...)]` strings sans `{}` génèrent code mort
5. **anti-stringly relay-selector signed** : warren-relay-selector signed JSON, typer plus fort
6. **anyhow leak** : `anyhow::Result` dans public API warren-* crates, migrer vers `thiserror` typed errors
7. **256 unwrap()** : audit cross-crates, prioriser hot paths
8. **dead_code annotations** : audit `#[allow(dead_code)]` cross-crates
9. **deprecated tokio APIs** : warren-tunnel + warren-client (tokio 1.x latest patterns)
10. **clippy nursery opt-in** : activer warnings nursery selectifs
11. **pedantic clippy gradual** : activer pedantic groups via #[allow] explicite

### P1.5, Tests (9 items ciblés)

1. **proptest natpmp-protocol malformed packets** : DÉJÀ FAIT Phase 4 fea1eeb (vérifier)
2. **warren-tls ALPN mismatch test** : DÉJÀ FAIT Phase 3 (vérifier)
3. **warren-wapi 6 smoke tests** : DÉJÀ FAIT Phase 3
4. **hpke_vectors_v1_reverse SHA-256 anchor** : DÉJÀ FAIT Phase 3
5. **Killswitch Drop rollback contract test** : DÉJÀ FAIT Phase 4
6. **DaitaState concurrent stress** : reproduire Session F finding in-process (existe via daita_sustained_stress.rs Session G)
7. **AE.5 deadlock regression test** : test `parking_lot::Mutex::lock()` dans `tracing!` macro single-thread → assert pas de double lock
8. **warren-relay-selector property test** : geo filter + weight selection invariants
9. **warren-natpmp-server allocator monotonic** : DÉJÀ FAIT 47e5f9f

### P1.6, Performance (2 items)

1. **PGO build profile** : DÉJÀ FAIT Phase 5 (257721b pgo-gen/pgo-use + docs/27)
2. **Quinn datagram buffer 8MiB recv + 4MiB send** : DÉJÀ FAIT Phase 5 (aa0627c)

### P1.7, Production readiness (résidus 17 items)

1. **`/metrics` Prometheus** : warren-api expose Prometheus text-exposition (skeleton d5224ad fait, finir)
2. **root CancellationToken unifié warren-exit** : shutdown propre cross-tasks
3. **structured logs JSON** : warren-api + warren-exit + warren-client output JSON sous flag `--log-json`
4. **health check `/healthz` warren-api** : DÉJÀ FAIT
5. **rate-limit /v1/incidents** : per-IP token bucket
6. **graceful shutdown warren-exit** : SIGTERM → close existing sessions + reject new (Phase 2 partial)
7. **back-pressure metric warren-tunnel** : queue depth Quinn datagram channel
8. **systemd hardening warren-api** : DÉJÀ FAIT (B.2 phase historic)
9. **logrotate warren-api + warren-exit** : config systemd
10. **prometheus scrape config** : docs/deploy
11. **alertmanager rules** : warren-api 5xx rate, warren-exit session count, etc.
12. **backup retention warren-api** : DÉJÀ FAIT cron-backup-warren-api script Phase 3
13. **smoke prod CI hourly** : GitHub Actions cron call test-backend-smoke.sh
14. **incident runbook docs** : docs/RUNBOOK-INCIDENTS.md create
15. **opensearch / loki ingestion** : centralisation logs prod
16. **warren-exit horizontal scale** : multi-instance behind LB plan (M5+ defer)
17. **rate-limit BTCPay webhook** : si M4.H.I lancé

---

## 3. Plan d'exécution (séquentiel par sous-phase)

```
P1.0 Worktree setup (30 min)
P1.1 Phase 2 résidus /metrics + root CancellationToken (3-5j)
P1.2 Sécurité 6 items (5-7j)
P1.3 Architecture 7 items (5-7j)
P1.4 Qualité Rust 11 items (7-10j)
P1.5 Tests P1 ciblés résidus (3-5j)
P1.6 Perf P1 (DÉJÀ FAIT, verify only) (0.5j)
P1.7 Production readiness 17 items (7-10j)
P1.8 Rapport + memory + cleanup (1j)
```

Total ~4-6 semaines. Commit per item TDD strict. Push warren-core + warren-app au fil de l'eau.

---

## 4. Critères GO ULTIMATE

- ✅ Tous P1 résidus listés section 2 livrés OU justifiés skip
- ✅ `cargo test --workspace` warren-core + warren-app PASS
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` PASS
- ✅ `cargo fmt --check` PASS cross-repo
- ✅ `cargo deny check` PASS (bincode advisory accepté via plan migration postcard docs/25)
- ✅ Baseline fmt+clippy+deny+test CLEAN
- ✅ Rapport `.planning/session-p1-hardening-report.md` rédigé avec items DONE/SKIP/DEFER

Verdict GO PARTIEL acceptable si :
- Quelques items P1 deferred avec justification (ex: M5+ scope)
- ≥ 80% items P1 livrés

---

## 5. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé obligatoire
- English-only code comments
- Pas em-dash
- Pas secrets in commits
- TDD strict per item

---

## 6. Memory updates

- `warren_session_p1_hardening_delivered.md` warren-core
- Update warren-core MEMORY.md + `warren_audit_2026_05_21` mention Phase final closed
- Update warren-app MEMORY.md si modif

---

## 7. Commencer maintenant

Worktrees §0.6 setup, lis docs/AUDIT-2026-05-21.md, attaque P1.1 (/metrics + root CancellationToken). Push au fil de l'eau. 4-6 sem budget.

Bonne route.
