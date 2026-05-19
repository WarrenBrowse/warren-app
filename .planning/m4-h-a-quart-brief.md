# Phase M4.H.A.quart - Fix bug warren-core /v1/subscribers/active poll silent fail + re-bench cross-DC

> Brief d'agent autonome **cross-repo** warren-core + warren-app.
> Doctrine NOUVELLE 2026-05-20 : **full autonomy, NO timid rollback**
> (cf. memory `feedback_agent_full_autonomy_no_timid_rollback`).
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 4-8h.
**Coût Hetzner** : ≤ 0.20 EUR (2-node prod préservé + 1 CCX23 nbg1 bench).
**Pré-condition** : briefs M4.H.A → bis → ter lus. Memory warren-core
`warren_exit_api_silent_poll_fail.md` lue (diagnostic 90% à valider+fixer).
**Objectif** : fixer le bug warren-core `apply_snapshot` early-return
silent identifié M4.H.A.ter, redeploy 2-node HEAD+fix, re-bench cross-DC,
verdict GO ULTIMATE final pour la séquence M4.H.A.

---

## 0. MANDAT STRICT

Anti-patterns historiques M4.E §7 (GO partiel, contournement, hypothèse
non vérifiée, loopback vs cross-DC, etc.) restent interdits. TDD strict
warren-core CLAUDE.md §1 obligatoire.

---

## 0.5 MANDAT D'AUTONOMIE (CRITIQUE - nouveauté doctrine 2026-05-20)

**Tu as plein mandat pour atteindre le verdict GO ULTIMATE.**

Si une étape fail ou si un imprévu apparaît :
- **Diagnostic** : 30 min max sur traces / logs / journalctl / cargo
  expand / cargo tree.
- **Fix tactique** en TDD strict (RED→GREEN→REFACTOR) **cross-repo
  autorisé**. Commit conventional, push origin/main du repo concerné.
- **Reprise** : continue la procédure brief avec le fix appliqué.
- **NO-GO** uniquement si après 2h d'investigation+fix tentés, le
  problème dépasse l'agent (probablement breaking change /v1 ou archi).

**PAS de rollback opportuniste. PAS d'escalade poka pour "voir si ok",
"valider l'approche", "que faire ensuite", "que veux-tu prioriser".
TU DÉCIDES, tu COMMIT, tu PUSH, tu CONTINUES.**

### Escalade `AskUserQuestion` poka SEULEMENT si :

1. **Secret leak** découvert dans working tree ou commit (seed, mnemonic,
   private key). Pas reproduire le contenu, escalader.
2. **Coût Hetzner > 0.30 EUR** (seuil monté, on accepte plus de profondeur
   investigation).
3. **Breaking change /v1** nécessaire (ALPN, HKDF salt/info, AAD layout,
   descriptor schema /v1 figés). Escalader pour passage /v2.
4. **Signing key prod doit être touchée** (rotate exit identity, admin
   signing key warren-api regen).

Tout autre cas tu décides. Si tu hésites sur l'archi : décide via TDD
puis commit. Si poka veut une autre approche il refactorera après.

---

## 1. Optimisations agent

- Parallélisme : lecture sources warren-core + warren-app en parallèle.
- TDD warren-core : 1 test RED ciblé d'abord, fix minimal, REFACTOR
  + tests adjacents en fin.
- Cross-compile warren-exit + warren-backend-api en parallèle.
- scp parallèle vers nodes.
- Un seul cargo final côté warren-core (`fmt --check && clippy -D warnings
  && test`) en validation phase.

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean
git log --oneline -3                        # HEAD b522e3c+ verify
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main HEAD post-M4.H.A.ter
git log --oneline -3
export WARREN_SSH_KEY=pokash
export HCLOUD_CONTEXT=warren
hcloud server list                          # warren-exit-1 + warren-backend-api state
```

---

## 3. Sources à lire (PARALLÈLE)

### Warren-core memory + code

- Memory `warren_exit_api_silent_poll_fail.md` warren-core (diagnostic
  90% précis, hypothèse 1 + 2 + 3, code paths suspects)
- `crates/warren-iroh-tunnel/src/allowlist.rs` : `AllowlistHandle`,
  `apply_snapshot`, `clear_if_stale`, `last_success_unix_secs`
- `crates/warren-exit/src/allowlist_refresh.rs` : `run_refresh_loop`,
  `one_tick`, fetcher trait
- `crates/warren-exit/src/allowlist_refresh.rs::tests` : les 15 tests
  existants pour identifier ce qui n'a PAS été couvert (= le bug)
- `crates/warren-api/src/handlers.rs:252` : handler `GET /v1/subscribers/
  active` (réponse schema, generation, active_pubkeys)
- `crates/warren-api/src/storage.rs` : `SqliteSubscriptionStore::list_
  active(now)` + `generation()` AtomicU64
- `crates/warren-iroh-tunnel/src/allowlist.rs::tests` : 17 tests
  existants pour identifier le trou
- Cross-ref memory `project_warren_auth_chain` warren-app (architecture)

### Warren-app

- `docs/m4-h-a-ter-bench-cross-dc-verdict.md` (rapport agent précédent)
- Memory `warren_m4h_a_ter_delivered.md`
- Brief précédent `.planning/m4-h-a-ter-brief.md`

### Cross-session memories à respecter

- `feedback_warren_no_secrets_in_commits` warren-core
- `feedback_warren_hetzner_bench_ops_gotchas` warren-core
- `feedback_no_em_dash` warren-core
- `feedback_handle_todos_properly` warren-core
- `feedback_tests_pertinents` warren-core
- `feedback_agent_full_autonomy_no_timid_rollback` warren-app (CRITIQUE)

---

## 4. Plan d'exécution

### M4.H.A.quart.0 - Reproduction RED warren-core

1. Lire le code `apply_snapshot` + `clear_if_stale` + le test
   `tests/d3_allowlist_dynamic.rs` pour comprendre les invariants.
2. Écrire un test RED dans `crates/warren-iroh-tunnel/src/allowlist.rs::tests`
   OU `crates/warren-exit/src/allowlist_refresh.rs::tests` qui reproduit
   le scenario exact :
   - Seed AllowlistHandle depuis cache disk avec `fetched_at_unix_secs =
     T0` (vieux timestamp simulé)
   - Spawn refresh loop avec fetcher mock qui retourne 10× successivement
     `Ok(Snapshot { generation: G_constant, pubkeys: [P0] })` (stable)
   - Tick le clock à T0 + 10 × 30s + 300s = T0 + 600s
   - Assert : allowlist NON cleared (`is_allowed(P0)` true)
3. Run le test : DOIT FAIL (= bug reproduit RED).
4. Si le test PASS dès le RED : le diagnostic 90% est faux, investiguer
   plus profondément. Hypothèses 2 et 3 de la memory warren-core
   `warren_exit_api_silent_poll_fail.md` à tester. Maximum 30 min en
   diagnostic supplémentaire.

### M4.H.A.quart.1 - Fix GREEN warren-core

1. Selon la cause identifiée :
   - **Hypothèse 1 (dominante)** : `apply_snapshot` doit set
     `last_success_unix_secs` à `now` SUR CHAQUE POLL SUCCESSFUL même
     si early-return generation gate. Distinguer "ack server" (= last_
     success update) de "apply mutations" (= early return si gen <=
     prev). Solution probable : un appel `last_success_unix_secs.store(
     now, Relaxed)` AU DÉBUT de `apply_snapshot` avant le gate.
   - **Hypothèse 2** : schema mismatch `/v1/subscribers/active`.
     Si confirmé : fixer le côté qui devie (probable warren-api handler
     retournant generation=0 systematic au lieu d'incrémenter au boot).
   - **Hypothèse 3** : auth chain régression. Si confirmé : fixer mTLS
     ou signature en gardant compat.
2. Patch minimal en GREEN, run test RED → PASS.
3. **Ajouter tests régression adjacents** (TDD discipline) :
   - Poll OK répété + generation décroissante → behaviour OK ?
   - Poll OK répété + generation stable + new pubkey snapshot →
     last_success doit update ET diff doit broadcast
   - Poll Err → last_success ne doit PAS update (existing test à vérifier)
   - Poll OK Snapshot empty → last_success update OK (= ack server)
4. Cargo validation warren-core groupée (fmt + clippy + test workspace
   subset) :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-core
   ./scripts/dev/cargo-test-nofw.sh fmt --check &
   ./scripts/dev/cargo-test-nofw.sh clippy -p warren-iroh-tunnel -p warren-exit -p warren-api --all-targets -- -D warnings
   wait
   ./scripts/dev/cargo-test-nofw.sh test -p warren-iroh-tunnel -p warren-exit -p warren-api
   ```
5. Commits warren-core sur main :
   - `test(warren-iroh-tunnel): RED for apply_snapshot last_success not updating on stable generation` (commit du test seul SI poka veut traçabilité RED-then-GREEN ; sinon commit unique fix+test)
   - `fix(warren-iroh-tunnel): apply_snapshot acks server on each successful poll regardless of generation gate` (ou équivalent selon cause root réelle)
6. Push origin/main warren-core.

### M4.H.A.quart.2 - Redeploy 2-node depuis HEAD+fix

1. Cross-compile warren-exit + warren-backend-api depuis warren-core
   HEAD post-fix.
2. Méthode SSH-replay (M4.H.A.bis §4.2) ou docker compose (M4.H.A.ter
   §4.3) pour les 2 nodes. Same IPs préservées.
3. Vérifier service up + journalctl healthy + allowlist polling
   visible (avec le fix appliqué, on doit voir le `last_success` update
   tracé en INFO+).
4. Smoke `bench/scripts/test-backend-smoke.sh` Tier 1 (cible 62/62 PASS,
   ou 60/62 si VAL1+VAL2 wapi regression toujours présente → fixer aussi
   tactique côté wapi puisqu'on est en autonomy mode).

### M4.H.A.quart.3 - Bench cross-DC empirique

1. Provision 1 CCX23 nbg1 client transient.
2. Bench scenario reproductible (run-bench-v2.sh M4.H.A.bis/ter ou
   équivalent fork-e2e-linux.sh adapté).
3. **Critère clé** : à T+5 min ET T+10 min, allowlist warren-exit-1
   ENCORE active (vs M4.H.A.bis/ter qui fail-closed à T+5).
4. Mesures perf cross-DC :
   - TCP 4-flow sustained 5 min ≥ 200 Mbps
   - PMTU ≥ 1280, idéalement 1379 GSO+TUNNEL_INITIAL_MTU
   - 0 stall ≥ 5s, 0 errors, 0 decode_failures, 0 replay_rejects
   - RSS stable
5. Si fix M4.H.A.quart.1 incomplet (par exemple bug encore reproductible
   sur T+10 min même après fix) : **boucle diagnostic+fix tactique
   AUTORISÉE** sans escalade. 30 min max par boucle.

### M4.H.A.quart.4 - Tear-down + finalize

1. Tear-down nbg1 (warren-exit-1 + warren-backend-api restent UP HEAD+fix).
2. Cleanup test artifacts `/tmp/m4-h-a-quart/`, voucher cancelled,
   pubkey décrochée, etc.
3. Rapport `/tmp/m4-h-a-quart-report.md` ≤ 150 lignes (§8).

### M4.H.A.quart.5 - Commits + memory updates

Warren-app :
- `bench(M4.H.A.quart): cross-DC verdict post warren-core fix /v1/subscribers/active poll` + push origin/main
- Memory `warren_m4h_a_quart_delivered.md` créé + index MEMORY.md

Warren-core :
- Commits fix déjà pushés §4.1
- Update memory `warren_exit_api_silent_poll_fail.md` warren-core →
  RÉSOLU avec SHA du fix + tests régression ajoutés
- Update memory `warren_backend_server.md` warren-core si coordonnées
  PROD ont changé post-redeploy

---

## 5. Règles non-négociables

### Sécurité

- Pas de secrets verbatim en commits/rapports/memories (cf.
  `feedback_warren_no_secrets_in_commits` warren-core). Test mnemonics
  + signing keys → ephemeral `/tmp/m4-h-a-quart/`.
- Pas de signing key prod touchée. Si nécessaire pour avancer : escalade.

### Code (warren-core CLAUDE.md §3)

- Edition Rust 2024, MSRV 1.89, toolchain pin 1.89.0
- Conventional commits subject-only, pas de body, pas de Co-Authored-By
- Identité git `poka <poka@p2p.legal>`
- TDD strict RED → GREEN → REFACTOR (cf. CLAUDE.md §1)
- /v1 constantes IMMUABLES (escalade pour /v2)
- Pas d'unsafe (forbid)
- Pas d'em-dash `—`
- Pas de step-tracking comments (`// M4.H.A.quart.x` interdit)
- Tests pertinents (cf. `feedback_tests_pertinents`)
- Pas de TODO laissés (delete, fix, ou convert doc/issue)

### Git

- Push direct main warren-core ET warren-app. Pas de feature branch,
  pas de worktree (cf. `feedback_warren_phase_prompts_no_branch`
  warren-core).
- Conventional commits.

### Bench Hetzner

- Tear-down nbg1 obligatoire en fin.
- warren-exit-1 + warren-backend-api LAISSÉS UP HEAD+fix (vraie nouvelle
  baseline prod).
- `hcloud server list` final = 2 nodes prod seuls.

---

## 6. Pas de validation intermédiaire poka

Cf. §0.5 ci-dessus. 4 cas d'escalade ONLY.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- Test RED warren-core reproduit le bug pré-fix
- Fix GREEN warren-core appliqué + tests régression ajoutés (≥ 3 nouveaux
  tests TDD)
- Cargo validation warren-core PASS (fmt + clippy -D warnings + test)
- Redeploy 2-node HEAD+fix sur IPs préservées : services active running,
  allowlist polling visible (last_success update tracé)
- Smoke Tier 1 PASS (62/62 ou 60/62 si VAL1+VAL2 wapi laissé open)
- Bench cross-DC :
  - Allowlist NON cleared à T+5min ET T+10min (= fix validé empiriquement)
  - TCP 4-flow sustained ≥ 200 Mbps sur 5 min
  - PMTU ≥ 1280, errors 0, stall 0
  - RSS stable
- Commits poussés warren-core + warren-app
- Memory updates dans les 2 repos

### GO CONDITIONAL

- Fix appliqué + bug NON reproductible empiriquement, mais throughput
  150-200 Mbps OU variance 15-20%
- OU caveat secondaire non bloquant identifié (wapi VAL1/VAL2, GHCR PAT
  write:packages)

### NO-GO HONNÊTE (très improbable vu doctrine §0.5)

- Fix prouvé impossible sans breaking change /v1 (escalade §6.3)
- OU 2h+ investigation profonde sans progrès tangible (escalade)

---

## 8. Rapport final attendu

`/tmp/m4-h-a-quart-report.md` ≤ 150 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO + 1 phrase
2. **Root cause finale** : Hypothèse 1 / 2 / 3 / autre, avec SHA fix
3. **Tests warren-core** : RED snippet + GREEN + tests régression listés
4. **Commits warren-core** + warren-app
5. **Redeploy 2-node** : methodology, downtime, smoke Tier 1 score
6. **Bench cross-DC** : tableau perf + témoignage allowlist non-cleared
   à T+10min
7. **Caveats résiduels** (wapi VAL1/2, GHCR PAT, etc.)
8. **Coût Hetzner** + tear-down attesté
9. **Memory updates**

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → débloque M4.H.B (câblage stack M4.E.D dans
  warren-app) avec perf cross-DC validée empiriquement. Le gros chunk
  dev. Brief M4.H.B sera drafté à la livraison.
- **GO CONDITIONAL** → on pondère caveats, possible M4.H.B en parallèle.
- **NO-GO** → escalade business poka (très improbable).

Caveats M4.H.A persistants à traiter plus tard :
- daemon-fork `account create` Remote LOCAL=0 factory bug (M4.H.A) →
  scope M4.H.B ou M4.H.E
- wapi VAL1/VAL2 client-side regression (M4.H.A.ter) → quick fix wapi
- GHCR PAT poka-IT write:packages (M4.H.A.ter) → ops poka

---

## 10. Trace de mémorisation

Warren-app :
- `warren_m4h_a_quart_delivered.md` créé
- Index MEMORY.md : `- [M4.H.A.quart delivered](warren_m4h_a_quart_delivered.md) — <verdict> + SHA fix warren-core + bench cross-DC final`

Warren-core :
- Update `warren_exit_api_silent_poll_fail.md` → RÉSOLU + SHA + cross-ref test régression
- Update `warren_backend_server.md` si coordonnées prod changent
- Optionnel : memory `feedback_allowlist_refresher_tdd_coverage.md`
  warren-core si la leçon test "ack server distinct de apply mutation"
  mérite une règle générale.
