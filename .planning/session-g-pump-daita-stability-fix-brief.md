# Session G — warren-core `pump_with_daita` stability fix

> Brief d'agent autonome warren-core (surface principale) + warren-app (pin bump).
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session courte critique : débloquer DAITA prod desktop + mobile.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : ~0.10 EUR (1 re-bench cross-DC fix validation, optionnel).
**Pré-conditions** :
- warren-core `main` HEAD `8b0e345+` (post-Session E)
- warren-app `main` HEAD `3c371fb8+` (post-Session E pin bump)
- Session F finding documenté : `.planning/session-f-report.md` + memory `warren_session_f_delivered`

**Objectif** : fixer `warren_tunnel::pump_*_bidirectional_with_daita` qui génère "QUIC datagram read error: downlink: timed out" en sustained cross-DC (26 WARN cumulées 5 min Session F). DAITA prod desktop + mobile bloqué jusqu'au fix. UDP 200 Mbps cap DAITA ON fail socket EAGAIN, DAITA OFF stable. Bug DAITA-amplifié (pump non-DAITA stalls existent mais UDP rate-cap reste fonctionnel).

Sous-phases (séquentielles autonomes) :

1. **G.1 — Setup worktree warren-core dédié** (~30 min)
2. **G.2 — Reproduction in-process avec netem RTT sim** (~1j)
3. **G.3 — Instrumentation tracing downlink task** (~0.5j)
4. **G.4 — Diagnostic root cause + fix candidat** (~1-2j)
5. **G.5 — Regression tests sustained 5 min** (~0.5j)
6. **G.6 — Re-bench Hetzner cross-DC validation (optionnel)** (~0.5j)
7. **G.7 — Rapport + memory + commit + push** (~30 min)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard (interdit : `git stash`, `git checkout <path>`, `git restore`, `git reset --hard`, `git clean`). Préserver tout fichier modified ou untracked warren-core + warren-app. Submodules warren-core preserved.

Violation = scope error CRITIQUE. Incident M4.H.F 2026-05-20 = 5 fichiers WIP poka warren-core perdus.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût Hetzner > 0.30 EUR (G.6 re-bench optionnel, cap ~0.10)
3. Breaking change /v1 wire format (le fix ne devrait PAS être wire-breaking — escalade si tu détectes nécessité)
4. Signing key prod
5. **Spécifique session G** : si root cause identifié nécessite refactor architectural majeur du pump (vs fix tactique), escalade pour validation archi avant push (effort > 5j)

Décisions tactiques agent autorisées :
- Reproduction strategy : in-process pure (deux tasks tokio loopback) vs `tc qdisc netem` Linux network sim vs Docker compose 2-container
- Fix strategy : sync lock → tokio Mutex async, ou batch timer events, ou separate Send/Recv loops, ou backpressure window
- Test pattern : Tokio's `tokio::test(flavor = "multi_thread")` vs `tokio::test` single-thread
- Bench tool re-bench : `warren_bench_multihop` ou warren-client `--enable-daita` production stack (cf. Session F pivot, ce dernier marche)

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

Pour éviter les race conditions agents parallèles (cf. memory `feedback_parallel_agents_same_worktree`, incident Sessions C+D 2026-05-21), cette session utilise un worktree dédié :

```bash
cd /Users/poka/dev/warrenBros/warren-core
git fetch origin
git worktree add ../warren-core-pump-fix main
cd ../warren-core-pump-fix
git status                                  # clean main 8b0e345+
```

Tous les commits + push depuis ce worktree. Cleanup en fin de session :
```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree remove ../warren-core-pump-fix
```

NE PAS travailler dans `/Users/poka/dev/warrenBros/warren-core` directement (risque collision si sessions H/C-cont/D-cont en parallèle bumpent le pin).

---

## 1. Setup initial

```bash
# 0.6 worktree setup
cd /Users/poka/dev/warrenBros/warren-core
git fetch origin
git worktree add ../warren-core-pump-fix main
cd ../warren-core-pump-fix

# Verify HEAD + tools
git log --oneline -3
cargo --version
rustc --version
```

Lire sources Session F :
```bash
cat /Users/poka/dev/warrenBros/warren-app/.planning/session-f-report.md
cat /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-core/memory/warren_session_f_delivered.md  # si présent
```

---

## 2. Optimisations agent

- Read sources warren-core pump + warren-app dispatch en PARALLÈLE
- Tests TDD groupés en fin de sous-tâche
- Push warren-core au fil de l'eau (pas batch)
- Pin warren-app `.warren-core-version` bump en fin de session unique commit warren-app

---

## G.1 — Setup worktree warren-core dédié (~30 min)

Cf. §0.6. Worktree créé. Confirmer HEAD warren-core = `8b0e345+`.

### Critères GO G.1

- Worktree opérationnel `../warren-core-pump-fix`
- `cargo check --workspace` PASS sur le worktree
- Memory + briefs Session F lus

---

## G.2 — Reproduction in-process avec netem RTT sim (~1j)

### Scope G.2

Reproduire le bug Session F en environnement controllé. Le bug se manifeste cross-DC sustained 5 min UDP 200 Mbps cap DAITA ON → socket EAGAIN. In-process devrait reproduire si vraie cause = deadlock task/timer/read.

1. **G.2.1** Créer test integration `crates/warren-tunnel/tests/daita_sustained_cross_dc.rs` :
   - 2 endpoints loopback Quinn (client side + exit side)
   - Multi-hop activé (HPKE + relay simulé in-process via warren-multihop)
   - pump_multi_bidirectional_with_daita lancé sur client
   - Send loop UDP 200 Mbps cap (RFC 9221 datagrams 1232 bytes, ~20k pps)
   - Duration 5 min sustained
   - Assert : 0 "QUIC datagram read error: downlink: timed out" WARN
2. **G.2.2** Si non-reproductible in-process pur :
   - Ajouter RTT simulation : `tc qdisc add dev lo root netem delay 10ms` (Linux uniquement)
   - Variante avec netem packet loss 0.01% (cross-DC realistic)
   - Re-run G.2.1
3. **G.2.3** Capturer stack traces si crash via `RUST_BACKTRACE=1`
4. **G.2.4** Si toujours pas reproductible : test alternatif :
   - Docker compose 2 containers Linux avec netem bridge entre eux
   - Ou re-bench Hetzner micro 1 min (cap coût)

### Critères GO G.2

- Bug reproductible (au moins 1 occurrence "downlink: timed out" sur 5 min sustained)
- Trace de l'event capturée

### Décisions tactiques G.2

- Préférer reproduction in-process avec netem (rapide, deterministe, gratuit)
- Si in-process refuse de reproduire : passer Docker. Si Docker refuse : 1 micro-bench Hetzner ~5 min (cap 0.05 EUR)
- Si AUCUNE reproduction : escalade pour confirmer bug environnemental Hetzner spécifique (pas warren-core)

---

## G.3 — Instrumentation tracing downlink task (~0.5j)

### Scope G.3

Ajouter visibilité dans le pump pour identifier où le deadlock/timeout se produit.

1. **G.3.1** Identifier les 3 tasks du pump multi-conn DAITA (memory `warren_session_b_delivered` §B.1) :
   - Uplink (1 task) : TUN.recv → NormalSent → MultiSession.send_datagram
   - Downlink (N tasks, 1/conn) : conn.read_datagram → is_daita_dummy → NormalRecv ou PaddingRecv
   - Timer (1 task) : sleep_until(next_timer) → drain_expired → dummies
2. **G.3.2** Ajouter `tracing::trace!` ou `tracing::debug!` spans (feature `tracing` warren-core, vérifier dep) :
   - Entry/exit chaque task tick
   - Lock acquisition + release `Arc<parking_lot::Mutex<DaitaState>>`
   - Quinn datagram read return value (Ok/Err type)
   - Timer next_wakeup duration
3. **G.3.3** Run reproduction G.2 avec `RUST_LOG=warren_tunnel=trace`
4. **G.3.4** Analyser logs autour des "downlink: timed out" : 
   - Timer task starvation ?
   - DaitaState lock contention ?
   - Quinn datagram_recv timeout interaction avec read_datagram await ?
   - Cancellation propagation ?

### Critères GO G.3

- Tracing instrumenté
- Reproduction G.2 produces trace logs autour de l'event
- Hypothèse root cause documentée (2-3 candidats max)

### Décisions tactiques G.3

- Garder les traces verbose seulement dans les tests (cfg(test)) ou derrière feature flag pour pas polluer prod logs
- `parking_lot::Mutex` n'est pas async-aware, contention bloque la task → suspect prime

---

## G.4 — Diagnostic root cause + fix candidat (~1-2j)

### Scope G.4

Sur la base de G.3, implémenter fix candidat. Hypothèses prioritaires (à valider G.3) :

**Hypothèse A — Lock contention sync vs async** :
- `Arc<parking_lot::Mutex<DaitaState>>` est sync (cf. memory B.1)
- Sustained 200 Mbps = ~20k packets/sec = 20k lock acquisitions/sec
- Si critical section non-triviale (HashMap lookup + state update), saturation possible sur certains workloads
- **Fix candidat** : `Arc<tokio::sync::Mutex<DaitaState>>` async-aware + reduce critical section, ou `Arc<RwLock>` si read-heavy

**Hypothèse B — Timer task starvation** :
- Timer task fait `sleep_until(next_timer) → drain_expired → multi_send_drop_too_large`
- Si drain_expired générer 100+ dummies en un seul drain, multi_send_drop_too_large peut bloquer (channel full, backpressure)
- **Fix candidat** : batch + yield, ou tokio::spawn detached send

**Hypothèse C — Downlink read_datagram timeout vs DAITA wakeup** :
- N downlink tasks, chacune `loop { conn.read_datagram().await }`
- DAITA dummies sortent via uplink/timer, pas downlink
- Si downlink Quinn datagram_recv interne timeout (Quinn 0.11 default ? config max_idle_timeout 180s post M4.E.C.quint), erreur surface ici
- **Fix candidat** : verifier Quinn datagram_recv error handling, distinguer "transient timeout" (continue) vs "real disconnect" (propagate)

**Hypothèse D — DAITA dummies overflow** :
- Si machine spec preset Mullvad v2 génère padding > capacity datagram channel Quinn
- Backpressure cascade
- **Fix candidat** : rate-limit Action::InjectPadding ou bounded queue

1. **G.4.1** Confirmer hypothèse via G.3 traces
2. **G.4.2** Implémenter fix candidat (smallest possible patch)
3. **G.4.3** Re-run G.2 test integration sustained 5 min
4. **G.4.4** Si fix valide : 0 WARN. Si toujours WARN : itérer sur hypothèse suivante.

### Critères GO G.4

- Hypothèse root cause validée par traces
- Fix candidat livré + smaller-than-50-LOC ideally
- Sustained 5 min in-process PASS (0 WARN "downlink: timed out")

### Décisions tactiques G.4

- Commit intermédiaire pour chaque hypothèse testée (revertable individuellement)
- Si toutes 4 hypothèses échouent : escalade case 5 (refactor archi)

---

## G.5 — Regression tests sustained 5 min (~0.5j)

### Scope G.5

Garantir non-régression future :

1. **G.5.1** Ajouter test integration `daita_sustained_cross_dc_no_timeout` dans le test suite warren-tunnel
2. **G.5.2** Marquer `#[ignore]` si trop lent (5 min) pour CI standard, runnable via `cargo test -- --ignored`
3. **G.5.3** Documenter dans CONTRIBUTING ou docs : run regression tests pump DAITA avant chaque release warren-core
4. **G.5.4** Test additionnel : 200 Mbps UDP cap sustained 5 min DAITA ON vs OFF, expect overhead documented

### Critères GO G.5

- Test integration ajouté
- `cargo test --test daita_sustained_cross_dc_no_timeout -- --ignored` PASS
- Pas de régression `cargo test --workspace`

---

## G.6 — Re-bench Hetzner cross-DC validation (optionnel) (~0.5j)

### Scope G.6

Valider que le fix tient en conditions réelles cross-DC Hetzner :

1. **G.6.1** Pre-flight Hetzner (memory `feedback_warren_hetzner_bench_ops_gotchas`) :
   ```bash
   export WARREN_SSH_KEY=pokash
   hcloud context use warren
   ```
2. **G.6.2** Provision 2 nodes ccx13 FSN1+NBG1 (parité Session F)
3. **G.6.3** Deploy warren-core binaires HEAD + fix
4. **G.6.4** Run bench DAITA ON UDP 200 Mbps cap 5 min sustained
5. **G.6.5** Compare WARN count : Session F = 26, target post-fix = 0
6. **G.6.6** Cleanup nodes + cost report

### Critères GO G.6

- 0 "QUIC datagram read error: downlink: timed out" WARN sur 5 min sustained Hetzner
- Cost ≤ 0.10 EUR
- Nodes cleaned, prod intacte

### Décisions tactiques G.6

- Si in-process G.5 PASS : G.6 optionnel mais recommandé pour confiance
- Skip G.6 si coût cumul session approche cap 0.30 EUR

---

## G.7 — Rapport + memory + commit + push (~30 min)

### Scope G.7

1. **G.7.1** Rapport `.planning/session-g-report.md` warren-app :
   - Verdict global
   - Hypothèse confirmée
   - Patch summary
   - Test integration ajouté
   - Re-bench Hetzner (si G.6 fait)
   - Recommandations DAITA prod desktop + mobile
2. **G.7.2** Memory warren-core : `warren_pump_daita_stability_fix.md` (root cause + patch + regression test pattern)
3. **G.7.3** Update warren-app `.warren-core-version` pin → HEAD post-fix
4. **G.7.4** Commit + push warren-core (worktree dédié) + warren-app (worktree main, single commit pin bump + report)
5. **G.7.5** Cleanup worktree warren-core :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-core
   git worktree remove ../warren-core-pump-fix
   ```

### Critères GO G.7

- Rapport + memory rédigés
- Pin warren-core bump dans warren-app committed
- Worktree warren-core nettoyé
- Push origin/main réussi cross-repo

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-core
- `crates/warren-tunnel/src/pump.rs` (pump_multi_bidirectional_with_daita)
- `crates/warren-tunnel/src/multi_session.rs` (MultiSession dispatch + daita_spec)
- `crates/warren-client/src/lib.rs` (run_uplink_with_daita + run_downlink_with_daita)
- `crates/warren-multihop/src/lib.rs` (HPKE layer + dummy marker 0xFF)
- Memory warren-core : `warren_session_b_delivered`, `warren_daita_doctrine_v1`
- Cargo.toml workspace : feature `tracing`

### warren-app
- `.planning/session-f-report.md` (Session F findings)
- `.warren-core-version` (pin actuel)

### Documentation maybenot
- crates.io `maybenot` docs (machine spec format + Action enum)
- Memory `warren_session_b_delivered` §B.1 (3-task model description)

---

## 4. Plan d'exécution (séquentiel)

```
G.1 Worktree setup (30 min)
G.2 Reproduction in-process + netem (1j)
G.3 Tracing instrumentation (0.5j)
G.4 Root cause + fix candidat (1-2j)
G.5 Regression tests (0.5j)
G.6 Re-bench Hetzner validation (0.5j, optionnel)
G.7 Rapport + memory + cleanup (30 min)
```

Total ~3-5j wall-clock.

---

## 5. Critères GO ULTIMATE session G

- ✅ G.1-G.7 critères GO PASS (G.6 optionnel)
- ✅ Bug "QUIC datagram read error: downlink: timed out" reproductible puis fixé
- ✅ Sustained 5 min UDP 200 Mbps DAITA ON in-process PASS sans WARN
- ✅ `cargo test --workspace` warren-core PASS + clippy strict PASS
- ✅ `cargo test --workspace` warren-app PASS (no régression desktop)
- ✅ Pin warren-app `.warren-core-version` bumped
- ✅ Rapport + memory rédigés
- ✅ Worktree warren-core cleaned

Verdict GO PARTIAL acceptable si :
- G.6 Hetzner skipped (cost cap atteint OU in-process G.5 solidly PASS)
- Multiple hypothèses échouent mais une combinaison de mitigations améliore stabilité (documenter caveat)

Verdict NO-GO si refactor archi major > 5j requis (escalation case 5).

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé obligatoire
- English-only code comments
- Pas em-dash
- TDD strict (RED → GREEN → REFACTOR)
- `hcloud --context warren` exclusif si G.6 fait
- Push warren-core au fil de l'eau

---

## 7. Memory updates attendus

À ajouter dans warren-core memory :
- `warren_pump_daita_stability_fix.md` — root cause + patch + regression test
- Update warren-core MEMORY.md

À ajouter dans warren-app memory :
- `warren_session_g_delivered.md` — verdict + caveats
- Update warren-app MEMORY.md

---

## 8. Commencer maintenant

Worktree setup §0.6, sources §3 en parallèle, attaque G.2.1. Plein mandat §0.5.

DAITA prod desktop + mobile dépend de cette session. Sans fix, le différenciateur DAITA publié warrenbrowse.com/features reste théorique. Priorité critique.

Bonne route.
