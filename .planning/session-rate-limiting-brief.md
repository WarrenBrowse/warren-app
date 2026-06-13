# Session Rate-Limiting, Per-pubkey rate-limiting client-side (#12)

> Brief d'agent autonome warren-core + warren-app.
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session courte sécurité : hardening anti-abuse subscriber.

**Effort estimé** : wall-clock 2-3 jours.
**Coût Hetzner** : 0 EUR (tests unit + integration suffisent).
**Pré-conditions** :
- warren-app `main` HEAD `eced6c8613+`
- warren-core `main` HEAD `fed1c88+`

**Objectif** : protection abuse client-side. Subscriber qui spam connect/disconnect (potentiel bot ou client buggué) saturera l'exit. Per-pubkey token bucket côté exit limite reconnect rate. Différé tier 5 audit (cf. memory H.E.5/6/7).

Sous-phases (séquentielles autonomes) :

1. **RL.1, Setup worktree** (~30 min)
2. **RL.2, Design + spec algorithm** (~0.5j)
3. **RL.3, Token bucket per-pubkey warren-exit** (~1j)
4. **RL.4, Wire dans accept loop warren-tunnel exit** (~0.5j)
5. **RL.5, Tests + bench micro impact** (~0.5j)
6. **RL.6, Rapport + cleanup** (~0.5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si secret leak, coût > 0.30 EUR, breaking /v1 wire format, signing key prod, OU **spécifique RL** : si rate-limit threshold demande tuning empirique bench prod réel (vs valeur tactique agent), escalader pour validation valeurs.

Décisions tactiques agent autorisées :
- Token bucket params (BURST=5 connects, REFILL=1/min OU BURST=30 RATE=10/s/IP pattern Phase 1 B.3)
- Storage : in-memory HashMap (recommandé bursty) vs sqlite persisté (overkill, restart clear OK pour anti-abuse short-window)
- Per-pubkey vs per-IP : pubkey (subscriber-stable) > IP (mobile users change)
- Comportement quand limite hit : refuse handshake avec error code spécifique OR silencieux drop (deny stealthy)
- Recommandation : refuse handshake avec `RateLimited` error variant + log warn

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-rate-limit main
cd ../warren-core-rate-limit
```

Cleanup :
```bash
git worktree remove ../warren-core-rate-limit
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-rate-limit main
cd ../warren-core-rate-limit

# Read existing rate-limit patterns (Phase 1 B.3 warren-api)
grep -rln "token_bucket\|rate_limit\|RateLimiter" crates/ | head
ls crates/warren-ratelimit/  # crate déjà présent (audit M4.H.E.4)
```

---

## 2. RL.2, Design + spec algorithm (~0.5j)

### Scope

Token bucket per-pubkey :
- Bucket capacity (burst) : 5 connects
- Refill rate : 1 token/minute (= 5 connects/min steady-state, 5 burst max)
- Per (pubkey, exit_id) key (pubkey identique pour multi-exit OK reset par exit)
- Storage in-memory HashMap<(PubkeyHex, ExitId), TokenBucketState>
- Cleanup : entries older than 1h auto-evicted (background task ou lazy on insert)

Trade-offs documentés :
- Mobile users handover Wi-Fi ↔ cellular peuvent dépasser burst → bursté 5 acceptable
- Bot spammer 100 connects/s → refusé après 5e, attente 1 min
- Multi-tenant subscriber sharing wallet (CGU permettent ?) → limite globale per pubkey

### Critères GO

- Design documenté `.planning/rate-limit-design.md`
- Algorithme + storage choisi

---

## 3. RL.3, Token bucket per-pubkey warren-exit (~1j)

### Scope

1. Crate `warren-ratelimit` existant (cf. M4.H.E.4 audit), vérifier si déjà adaptable
2. Nouveau module ou extension : `warren-ratelimit::PerPubkeyBucket`
3. API :
   ```rust
   pub struct PerPubkeyBucket {
       buckets: parking_lot::RwLock<HashMap<(PubkeyHex, ExitId), BucketState>>,
       burst: u32,
       refill_rate_per_sec: f64,
   }
   
   impl PerPubkeyBucket {
       pub fn new(burst: u32, refill_rate_per_sec: f64) -> Self
       pub fn try_acquire(&self, pubkey: PubkeyHex, exit_id: ExitId) -> Result<(), RateLimitError>
       pub fn cleanup_stale(&self)  // background task
   }
   ```
4. Tests TDD strict :
   - Burst 5 acquires OK consecutive
   - 6e acquire fails
   - After 1 minute, 1 token refilled
   - Cleanup stale entries
   - Concurrent acquires thread-safe

### Critères GO

- PerPubkeyBucket impl complète
- 5+ unit tests PASS
- clippy strict PASS

---

## 4. RL.4, Wire dans accept loop warren-tunnel exit (~0.5j)

### Scope

1. Identifier accept loop warren-exit : `serve_multihop_with_tun_and_daita` ou équivalent (warren-tunnel)
2. Au moment handshake reçu pubkey client :
   - Call `bucket.try_acquire(pubkey, self_exit_id)`
   - Si Ok → continue accept
   - Si Err(RateLimited) → reject handshake + log warn + emit metric `rate_limited_connections_total`
3. Spawn background task `bucket.cleanup_stale` toutes les 5 min
4. Config exposé via CLI flag : `--rate-limit-burst 5 --rate-limit-refill-per-sec 0.0167` (~1/min) ou defaults
5. Disable flag `--no-rate-limit` (utile bench/dev)

### Critères GO

- Wire complet warren-exit
- CLI flags + defaults
- Metric counter exposé
- Reject handshake correct comportement

---

## 5. RL.5, Tests + bench micro impact (~0.5j)

### Scope

1. Integration test warren-exit :
   - Simuler 10 connects same pubkey burst → 5 ok + 5 rejected
   - After sleep 60s → 1 ok + 4 rejected
2. Bench micro : impact perf accept loop avec rate-limit ON vs OFF (target overhead < 100ns/handshake)
3. Verify cleanup task ne crash pas + libère mémoire correctement (stress test 10k pubkeys then idle 5 min then cleanup)

### Critères GO

- 3+ integration tests PASS
- Bench micro overhead documenté
- Stress test 10k pubkeys cleanup OK

---

## 6. RL.6, Rapport + cleanup (~0.5j)

### Scope

- Rapport `.planning/session-rate-limiting-report.md`
- Memory `warren_session_rate_limiting_delivered.md` warren-core
- Update MEMORY.md
- Cleanup worktree

---

## 7. Sources cross-repo à lire (PARALLÈLE)

- `crates/warren-ratelimit/` (existant)
- `crates/warren-tunnel/src/exit.rs` (accept loop)
- `crates/warren-exit/src/main.rs` (CLI flags)
- Phase 1 B.3 rate-limit per-IP warren-api (pattern reference)
- Memory `warren_killswitch_audit` (pattern consume warren-client warren-core)

---

## 8. Critères GO ULTIMATE

- ✅ RL.2-RL.6 critères GO PASS
- ✅ Per-pubkey bucket impl + tested
- ✅ Wire warren-exit accept loop
- ✅ CLI flags
- ✅ Tests integration + bench micro
- ✅ `cargo test --workspace` warren-core PASS + clippy strict
- ✅ Rapport rédigé
- ✅ Worktree cleaned

---

## 9. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree
- English-only code comments
- Pas em-dash
- Pas secrets in commits

---

## 10. Memory updates

- `warren_session_rate_limiting_delivered.md`
- Update MEMORY.md

---

## 11. Commencer maintenant

Worktree §0.6, sources §7 en parallèle, attaque RL.2 design. Push au fil de l'eau.

Bonne route.
