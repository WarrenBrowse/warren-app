# Phase M4.H.A - Linux bench fork E2E validation

> Brief d'agent autonome. Fichier source de vérité.
> La commande `/goal` compagne pointe vers ce fichier.
> Optimisé pour parallélisme max et tests groupés.

**Effort estimé** : wall-clock 1 jour.
**Coût Hetzner** : ~0.05 EUR (1 paire CCX23 nbg1+hel1, 1-2h provisionnement + bench).
**Pré-condition** : working tree warren-app clean sur `main` HEAD `bb7a5ff586` (post-migration Quinn complète). `.warren-core-version` pin obsolète à corriger dans cette phase.
**Objectif** : valider que le fork warren-app post-migration Quinn (commits `75319088ec` → `f5c0770319`) ne régresse pas vs warren-core M4.E.C.quint baseline en network réel Linux. Caveat #1 du `docs/warren-app-quinn-migration-report.md`, blocker pre-prod.

---

## 0. MANDAT STRICT - lire avant tout

### Anti-patterns interdits (M4.E lessons learned)

1. **Validation partielle déclarée GO** : ne pas déclarer "GO inconditionnel" si critères perf non atteints. Verdict HONNÊTE obligatoire : GO, CONDITIONAL, ou NO-GO selon les critères §7. Si ambiguïté, escalader poka.
2. **Contournement au lieu d'investigation** : si un bench échoue ou montre une régression, identifier la cause root (PMTU, knobs, GSO patch propagation, transport config). PAS de "baissons les params pour voir si ça marche".
3. **Hypothèses non vérifiées avant action** : ne pas supposer une cause sans grep/read. Si tu suspectes le GSO patch non propagé, vérifie `cargo tree -p talpid-warren-tunnel -i quinn` AVANT de lancer un fix.
4. **Bench tool buggé propagé** : avant de bencher, vérifier que le script `warren-core/bench/scripts/fork-e2e-linux.sh` est dans sa version courante (`git log -1 fork-e2e-linux.sh`) et qu'il n'a pas de hardcoded values invalidant les résultats.
5. **Loopback comme proxy de cross-DC** : la validation macOS du migration report ne suffit pas. Bench OBLIGATOIRE sur réseau réel cross-DC Hetzner nbg1 ↔ hel1.
6. **Combinaisons jamais testées ensemble** : tester le bench avec la stack complète activée (single-hop d'abord, multi-hop pas dans ce scope vu que pas câblé).

### Comportement attendu

- Verdict honnête en fin de bench.
- Si caveat critique découvert : escalade `AskUserQuestion` à poka avant de "fixer en allant" (ex: GSO patch non propagé, cargo tree montre upstream quinn pas le fork local).
- Si réseau Hetzner instable (variance > 15%) : escalader plutôt que normaliser.

---

## 1. Optimisations agent (impératif performance)

- **Tool calls EN PARALLÈLE** quand pas de dépendance : multiples Read, multiples Bash hcloud (provisioning nbg1 + hel1 simultanés via background tasks).
- **Tests/builds groupés** : un seul `cargo check --workspace` en début de phase pour vérifier compile, un seul `cargo build --release` cross-compile en milieu de phase. Pas de cargo check intermédiaire entre étapes.
- **Cross-compile une seule fois** : `cargo build --release --target x86_64-unknown-linux-gnu -p mullvad-daemon -p mullvad-cli` une fois après bump pin, scp parallèle vers nbg1 et hel1.
- **Validation finale** :
  ```bash
  ./scripts/dev/cargo-test-nofw.sh fmt --check &
  ./scripts/dev/cargo-test-nofw.sh clippy -p talpid-warren-tunnel --all-targets -- -D warnings
  wait
  ./scripts/dev/cargo-test-nofw.sh test -p talpid-warren-tunnel -p talpid-core
  ```
  (NOTE : `warren-app` n'a pas de `cargo-test-nofw.sh` historiquement. Vérifier `ls scripts/dev/` au démarrage et utiliser `cargo` direct si le wrapper absent. Sur Linux le wrapper passe transparent de toute façon.)

---

## 2. Setup initial obligatoire

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                           # MUST be clean main (HEAD bb7a5ff586)
git log --oneline -5                 # verify post-migration HEAD
cat .warren-core-version             # currently 278c374969 (obsolète)
git -C ../warren-core log --oneline -1  # warren-core actual HEAD
ls scripts/dev/ 2>/dev/null          # check wrapper presence
hcloud context list                  # MUST show warren active (jamais poka-perso)
```

Si `git status` non-clean ou HEAD inattendu : STOP, escalade poka.

---

## 3. Sources à lire (PARALLÈLE - un seul message multi-Read)

### Repo warren-app (présent)

- `docs/warren-app-quinn-migration-plan.md` (audit + stratégie sharing)
- `docs/warren-app-quinn-migration-report.md` (verdict migration macOS + caveats §"Residual risks" §"Follow-ups identified")
- `talpid-warren-tunnel/Cargo.toml` (path-deps actuels = `warren-tunnel`, `warren-protocol`)
- `talpid-warren-tunnel/src/lib.rs` (entry point adapter, structure post-migration)
- `Cargo.toml` workspace root (vérifier `[patch.crates-io] quinn = ../warren-core/vendor/quinn-fork/quinn`)
- `mullvad-daemon/src/warren_iroh_params.rs` (params daemon-side, `WarrenPubkey` + `WarrenExitAddr` post-migration)
- `.warren-core-version` (SHA pin obsolète à bumper)

### Repo warren-core (référence)

- `bench/scripts/fork-e2e-linux.sh` (le bench script à utiliser)
- `bench/results/2026-05-10_fork_e2e_validated.md` (baseline pré-migration de référence)
- `bench/results/2026-05-19_M4E_REPORT.html` (rapport M4.E.C.quint, baseline 409 Mbps cross-DC)
- `crates/warren-tunnel/src/lib.rs` (entry point single-hop ClientTunnel)
- `crates/warren-tunnel/src/transport_config.rs` (knobs warren GSO + M4.0 obfuscation)

### Memory cross-session warren-core

- `warren_quinn_stack_final.md` (stack Quinn 0.11 + fork GSO)
- `warren_m4e_delivered.md` (récap M4.E.C.quint + M4.E.D)
- `warren_obfuscation_doctrine_v1.md` (M4.0 knobs `min_first_datagram_size` = TUNNEL_INITIAL_MTU = 1280)
- `feedback_warren_hetzner_bench_ops_gotchas.md` (pre-flight `WARREN_SSH_KEY=pokash` + cleanup `/tmp/warren-*` SSH)
- `feedback_warren_competitor_comparatives.md` (si rapport mentionne perf vs concurrents)

---

## 4. Plan d'exécution (sous-tâches enchaînées, validation groupée)

### M4.H.A.0 - Pré-flight repo + warren-core pin bump

1. Vérifier état clean repo (cf. §2).
2. Lire `git -C ../warren-core log --oneline -1` → SHA HEAD warren-core actuel (devrait être `b522e3c` ou descendant M4.E.C.quint).
3. Bumper `.warren-core-version` au SHA HEAD warren-core ou un SHA validé M4.E.D (poka peut indiquer une SHA précise post-M4.E.D si plus récente que `b522e3c`).
4. `cargo check --workspace` côté warren-app, doit passer 0 erreurs / 0 warnings (sauf submodules windows/* qui sont hors scope Linux).
5. Si compile fail : STOP, escalade poka avec output cargo verbatim.

### M4.H.A.1 - Cross-compile Linux release

1. `rustup target add x86_64-unknown-linux-gnu` si pas déjà présent.
2. `cargo build --release --target x86_64-unknown-linux-gnu -p mullvad-daemon -p mullvad-cli` côté warren-app.
3. Vérifier binaires `target/x86_64-unknown-linux-gnu/release/{mullvad-daemon,mullvad-cli}` présents (binaires Mullvad renommés warren-daemon/warren-cli au runtime via `CARGO_BIN_NAME`).
4. **Critique** : vérifier propagation patch GSO via :
   ```bash
   cargo tree --target x86_64-unknown-linux-gnu -p talpid-warren-tunnel -i quinn | head -5
   ```
   Doit montrer `quinn v0.11.9 (/Users/poka/dev/warrenBros/warren-core/vendor/quinn-fork/quinn)`, PAS upstream `quinn v0.11.x` registry. Si registry : GSO patch perdu silencieusement, STOP escalade poka.

### M4.H.A.2 - Provisionnement Hetzner cross-DC

1. Pre-flight : `export WARREN_SSH_KEY=pokash`, `hcloud context list` → warren actif.
2. Provisionner en parallèle (background tasks ou `&` + `wait`) :
   - 1× CCX23 nbg1 (client warren-app daemon Linux)
   - 1× CCX23 hel1 (exit warren-core warren-exit single-hop)
   Cloud-init minimal : install build-essential pour libc deps, ouvrir UDP port (443 si M4.0 obfuscation, 7000 fallback).
3. Vérifier `hcloud server list` = 2 nodes warren-* + warren-exit-1 + warren-backend-api intacts.
4. Cleanup pre-bench `/tmp/warren-*` côté nodes via SSH (gotcha 2026-05-19).

### M4.H.A.3 - Bench fork E2E paired same-session

1. scp parallèle des binaires Linux warren-app vers nbg1 (client) et hel1 (exit warren-core baseline).
2. **Run 1 - warren-app post-migration** : exécuter `bench/scripts/fork-e2e-linux.sh` warren-core (adapter pour pointer vers le warren-daemon de warren-app cross-compilé). Bench scenario reproduit M3.J / M4.E.D pattern : 30s × 5 runs minimum, payload 900-1100B selon path-MTU vanilla observable, log min/max/stddev.
3. **Run 2 - baseline pré-migration** (optionnel selon décision poka §6) : checkout warren-base à HEAD pre-migration (`440c97f36a` upstream baseline pré-fork ne convient pas car pré-Warren ; utiliser le commit `8bfe4ea46b cargo deps` qui précède immédiatement les 7 commits migration). Re-cross-compile + bench même scénario. Coût Hetzner +0.03 EUR.
4. Comparer paired :
   - Throughput sustained (cible : ≥ 90% baseline pré-migration ; tolérance variance Hetzner ±10%)
   - Latence p50/p99 (cible : ≤ baseline + 5%)
   - PMTU négocié (`max_datagram_size` doit être ≥ 1280, idéalement 1379 avec GSO+TUNNEL_INITIAL_MTU)
   - Stall events ≥ 5s (cible : 0)
   - errors / decode_failures / replay_rejects (cible : tous 0)
   - CPU client/exit utilisé (sampling 30s)
   - RSS growth sur 5 min (cible : < 10 MB)

### M4.H.A.4 - Tear-down + verdict

1. `./scripts/teardown-hetzner.sh --yes` ou `hcloud server delete warren-fork-bench-*`.
2. Vérifier `hcloud server list` = seuls warren-exit-1 + warren-backend-api (PROD intacts).
3. Drafter rapport `/tmp/m4-h-a-report.md` (cf. §8).
4. Validations cargo finales (groupées) :
   ```bash
   cargo fmt --check &
   cargo clippy -p talpid-warren-tunnel --all-targets -- -D warnings
   wait
   cargo test -p talpid-warren-tunnel -p talpid-core -p mullvad-daemon
   ```

### M4.H.A.5 - Finalize + commits + memory update

1. Si bump `.warren-core-version` validé : commit `chore(warren-core-pin): bump to <sha> for M4.E.D auto-reconnect support`.
2. Commit rapport bench : `bench(M4.H.A): fork E2E Linux cross-DC post-Quinn migration verdict`.
3. Push `origin/main`.
4. Update mémoire orchestrateur (path : `/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/`) avec un memory `warren_m4h_a_delivered.md` synthétisant les résultats.

---

## 5. Règles non-négociables

### Sécurité

- JAMAIS écrire/committer textuellement seeds, mnemonics, private keys, signing keys (cf. incident 2026-05-18 dans `warren-core memory feedback_warren_no_secrets_in_commits.md`). Si découverte d'un secret : escalade `AskUserQuestion` sans reproduire le contenu.

### Code

- Tous commentaires nouveaux en anglais (cf. `feedback_english_only_comments.md` warren-app).
- Pas de step-tracking dans le code (`// M4.H.A.x` interdit).
- Pas d'em-dash `, ` n'importe où (chat, commit, code, docs).
- Pas de TODO laissé sans traitement (delete, fix, ou convert en doc).
- Pas d'`unwrap()` / `expect()` en prod sans `# Panics` documenté.
- Pas de `unsafe` (forbid global Warren).
- Conventional commits subject-only, pas de body, pas de `Co-Authored-By: Claude`.

### Git

- Branche de travail : `main` direct, push autorisé.
- Pas de feature branch, pas de worktree, pas de `--no-verify`.
- Pas de commande git destructive (`stash`, `reset --hard`, `checkout .`, `restore .`, `clean`) sans demande poka explicite.

### Bench Hetzner

- Tear-down obligatoire en fin de session (`hcloud server delete` + vérif liste vide).
- Toujours `WARREN_SSH_KEY=pokash` + cleanup `/tmp/warren-*` pre-bench (gotcha).
- Résultats commit dans `bench/results/` côté warren-core (ou `docs/` côté warren-app si pertinent).

---

## 6. Pas de validation intermédiaire poka

Tu travailles en autonomie complète. Cas d'escalade `AskUserQuestion` SEULEMENT si :

1. **Compile fail** post-bump `.warren-core-version` non résolvable par sync warren-core path.
2. **GSO patch non propagé** détecté par `cargo tree` (régression silencieuse du `[patch.crates-io]`).
3. **Run 2 baseline pré-migration** : poka tranche s'il veut comparer paired (coût +0.03 EUR mais comparaison rigoureuse) ou s'il accepte de comparer contre les baselines warren-core M4.E.* publiées.
4. **Verdict ambigu** : si throughput entre 85-90% de baseline (zone grise variance Hetzner).
5. **Caveat sécurité découvert** (cf. §5 sécurité).
6. **Coût Hetzner dépassé** > 0.10 EUR sans accord.

Sinon : tu décides, tu commits, tu push, tu rapportes.

---

## 7. Critères phase livrée

### GO ULTIMATE (idéal)

- `cargo check --workspace` + `cargo clippy -- -D warnings` PASS post-pin bump
- Cross-compile Linux x86_64 PASS
- `cargo tree` confirme GSO fork propagé
- Bench cross-DC nbg1 ↔ hel1 :
  - Throughput ≥ 200 Mbps sustained 5 min (vs M4.E baseline ~280 Mbps single-hop client direct, on attend une fraction acceptable via daemon Mullvad state machine overhead)
  - 0 stall ≥ 5s
  - 0 errors / decode_failures / replay_rejects
  - PMTU négocié ≥ 1280
  - RSS stable (< +10 MB sur 5 min)
- Tear-down attesté

### GO CONDITIONAL (acceptable)

- Tous compile + cargo tree OK
- Bench passe critères principaux mais avec 1-2 caveats documentés (variance > 10% mais < 20%, ou throughput entre 150-200 Mbps)

### NO-GO HONNÊTE

- Compile fail non résolu
- GSO patch perdu silencieusement
- Throughput < 150 Mbps OU stall ≥ 5s OU errors > 0 sustained
- Régression > 25% vs baseline pré-migration

---

## 8. Rapport final attendu

Markdown structuré, ≤ 200 lignes, sections :

1. **Verdict** : GO ULTIMATE / GO CONDITIONAL / NO-GO + 1 phrase de justification
2. **Pin bump** : SHA avant → après, commit hash
3. **Compile + cargo tree** : output `cargo tree -p talpid-warren-tunnel -i quinn` (clé : doit montrer le fork local)
4. **Bench setup** : nodes Hetzner, scénarios, durée
5. **Résultats** : tableau throughput / latence / PMTU / errors / RSS / CPU
6. **Caveats** : tout ce qui n'est pas GO ULTIMATE
7. **Coût Hetzner** : EUR cumulés, tear-down attesté
8. **Commits poussés** : hashes + un-liner
9. **Memory update** : référence au memory file créé

---

## 9. Next steps post-phase (mémoire orchestrateur, pas scope agent)

- Si GO : débloque M4.H.B (câblage stack M4.E.D dans talpid-warren-tunnel)
- Si CONDITIONAL : déterminer si caveats bloquent M4.H.B ou peuvent être différés
- Si NO-GO : phase de correction `M4.H.A.bis` à drafter sur cause root identifiée
- Quel que soit le verdict : mettre à jour memory `warren_m4h_a_delivered.md` warren-app

---

## 10. Trace de mémorisation

Le fichier memory `warren_m4h_a_delivered.md` à créer au path :
`/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/`

Format frontmatter YAML + body. Inclure :
- Verdict + résultats clés (3-5 lignes max)
- Commits hashes
- Caveats résiduels (pour briefs M4.H.B/C qui les consument)
- SHA `.warren-core-version` finalisé
- Pointeur `[[warren-m4e-delivered]]` (cross-repo memory warren-core)

Indexer dans `MEMORY.md` warren-app :
`- [M4.H.A delivered](warren_m4h_a_delivered.md), verdict Linux fork E2E + SHA pin + caveats`
