# Phase M4.H.G - `--bypass-cidr` + Backoff tune (caveats post-M4.E.D)

> Brief d'agent autonome cross-repo warren-core + warren-app.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 1 jour (8h).
**Coût Hetzner** : 0 EUR (tests unit + intégration suffisent, pas
de bench cross-DC nécessaire).
**Pré-condition** :
- warren-app main HEAD `cf22d648b4+` (post-M4.H.F + doctrine §0.0)
- warren-core HEAD `6272ba50+` (post-M4.H.F refresh_loop NAT-PMP)

**Objectif** : fixer 2 caveats connus post-M4.E.D pour clore le scope
dev warren-app M4.H :

1. **M4.H.G.1** : `--bypass-cidr` flag dans `warren-client` (warren-core)
   pour exclure des CIDR du routing tunnel. Le mode `--auto-routing`
   actuel casse les SSH inbound car il route 0.0.0.0/0 via le tunnel,
   coupant les sessions SSH entrantes. Flag pour exclure des CIDR
   (ex: `--bypass-cidr 192.168.0.0/16,10.0.0.0/8` pour LAN +
   private ranges).
2. **M4.H.G.2** : `Backoff::HANDSHAKE` ceiling 30s → 15s dans
   `warren-backoff` (warren-core) pour ramener worst-case reconnect
   M4.E.D ≤ 15s (vs 31s observé M4.E.D bench).

---

## 0.0 INVIOLABLE - pas de commande git destructive

Quelle que soit la situation (test, recovery, "voir si ça compile",
diagnostic, expérimentation), tu ne dois JAMAIS exécuter :

- `git stash` (et toutes variantes)
- `git checkout <path>` ou `git checkout -- .`
- `git restore <path>` ou `git restore .`
- `git reset --hard <ref>`
- `git reset --hard` (sans ref)
- `git clean -fd` (et toutes variantes destructives)
- `git rebase` interactif qui force-modify le WT
- `git revert --no-commit` qui modifie le WT sans valider
- Toute commande qui modifie ou discard les fichiers untracked OU
  modified du working tree

Cette interdiction PRIME sur le mandat d'autonomie §0.5. Si tu
penses avoir besoin d'une commande destructive : ESCALADE poka
via AskUserQuestion AVANT exécution, sans exception.

Pour tester un état antérieur, utilise `git show <ref>:<path>`
(read-only) ou `git diff <ref> --stat` ou `git log -p <path>`. Pour
récupérer une version, copie via `git show <ref>:<path> >
/tmp/file-at-ref` puis Read.

Si tu trouves le working tree dans un état inattendu (untracked
files, fichiers modified inattendus), ESCALADE poka SANS toucher
au tree.

Violation de cette règle = scope error CRITIQUE, indépendamment du
verdict final de la phase. Incident M4.H.F 2026-05-20 : agent autonome
a exécuté `git checkout a7159d94 -- .` dans warren-core, 5 fichiers
WIP poka perdus.

---

## 0. MANDAT STRICT

Anti-patterns historiques M4.E §7 + TDD strict warren-core CLAUDE.md
§1 (RED→GREEN→REFACTOR). /v1 constantes IMMUABLES. Pas em-dash,
anglais comments.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein
mandat pour atteindre GO. Diagnostic 30 min → fix tactique TDD →
commit + push → reprise. PAS de rollback, PAS d'escalade timide.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût > 0.30 EUR (n/a, pas de Hetzner)
3. Breaking change /v1 wire format
4. Signing key prod touchée
5. **Spécifique M4.H.G** : si `Backoff::HANDSHAKE` ceiling est figé
   /v1 (utilisé par d'autres consumers warren-core qui pourraient
   être impactés par la baisse 30→15s), escalade pour validation
   archi avant push

Décisions tactiques agent autorisées :
- Format du flag `--bypass-cidr` (multiple CIDR comma-sep vs --bypass-cidr
  répétable)
- Ordre route table (bypass avant ou après default route tunnel)
- Default CIDR inclus (rien par défaut ? ou auto-detect LAN ?)

---

## 1. Optimisations agent

- Read sources cross-repo en PARALLÈLE
- Tests TDD groupés en fin de sous-tâche
- Push warren-core + warren-app au fil de l'eau

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main HEAD cf22d648b4+
git remote -v                                # origin = GitHub WarrenBrowse
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean
git log --oneline -3                        # HEAD 6272ba50+ ou descendant
```

Si HEAD inattendu : escalade (cf. §0.0, surtout pas de checkout).

---

## 3. Sources à lire (PARALLÈLE)

### Warren-core

- `crates/warren-client/src/lib.rs` (entry point ClientTunnel,
  routing config)
- `crates/warren-client/src/auto_routing.rs` (si existant, sinon
  identifier où le routing 0.0.0.0/0 est posé)
- `crates/warren-client/src/default_route_split_*.rs` (Linux + macOS)
  pour comprendre le pattern routing existant
- `crates/warren-backoff/src/lib.rs` (Backoff::HANDSHAKE constante
  actuelle + consumers via grep)
- `crates/warren-tunnel/src/lib.rs` (pour vérifier si warren-tunnel
  consume Backoff::HANDSHAKE directement)
- `crates/warren-client/src/multi_hop.rs` (MultiHopSupervisor M4.E.D
  consumer probable)

### Warren-app

- `talpid-warren-tunnel/src/default_route_split.rs` (routing OS-spécifique
  client-side daemon)
- `talpid-warren-tunnel/src/lib.rs` (start_tunnel routing setup)
- `mullvad-daemon/src/warren_tunnel_params.rs` (params à étendre
  pour --bypass-cidr exposure UI future)
- `mullvad-daemon/src/warren_relay_selector.rs`
- `mullvad-management-interface/proto/management_interface.proto` (si
  besoin d'exposer --bypass-cidr setting via gRPC pour UI M4.H.x future)

### Memory cross-session

- `warren_m4e_delivered.md` warren-core (M4.E.D reconnect 31s worst-case)
- `warren_obfuscation_doctrine_v1.md` (PMTU + routing invariants)
- `feedback_agent_full_autonomy_no_timid_rollback.md`
- `feedback_no_destructive_git_in_agent_briefs.md`

---

## 4. Plan d'exécution

### M4.H.G.1 - `--bypass-cidr` flag

#### M4.H.G.1.a - warren-client lib API

1. Identifier où le routing 0.0.0.0/0 est posé dans warren-client
   (probable `auto_routing.rs` ou `default_route_split_*.rs`).
2. Étendre le builder/config :
   ```rust
   pub struct AutoRoutingConfig {
       pub enabled: bool,
       pub bypass_cidrs: Vec<IpNetwork>,  // NEW
   }
   ```
3. TDD RED : test que `--auto-routing` avec `--bypass-cidr 192.168.0.0/16`
   POSE une route plus spécifique (192.168.0.0/16 → original gateway,
   pas tunnel) AVANT la default route 0.0.0.0/0 → tunnel.
4. TDD GREEN : implémenter dans le module routing concerné.
5. Tests régression : pas de bypass = comportement actuel inchangé,
   bypass vide = pas de route ajoutée, CIDR overlap avec 0/0 = bypass
   wins (route plus spécifique).
6. Côté binaire `warren-client` si applicable : CLI flag
   `--bypass-cidr <CIDR>` répétable (clap multiple value).
7. Commit warren-core `feat(warren-client): add --bypass-cidr to AutoRoutingConfig (SSH inbound preservation)`.

#### M4.H.G.1.b - warren-app daemon plumbing

1. Étendre `WarrenTunnelParameters` daemon-side avec
   `bypass_cidrs: Vec<IpNetwork>` (default vec![]).
2. Plumber depuis settings → talpid-warren-tunnel → warren-client.
3. TDD : test parse settings + propagation jusqu'au warren-client
   builder.
4. Commit warren-app `feat(warren-tunnel): plumb bypass_cidrs from settings to warren-client`.

#### M4.H.G.1.c - (optionnel) exposition UI

Si scope cohérent avec M4.H.G effort 1j cible :
- gRPC proto extension settings ↔ UI : pas dans scope strict M4.H.G
- Si touch UI : nouvelle entry dans vpn-settings/ "Bypass private
  ranges" toggle (auto-fill LAN ranges) + custom CIDR input
- Sinon : settings persisté daemon-side uniquement (poka peut
  configurer via CLI mullvad-cli en attendant UI)

**Recommandation** : skip UI exposition cette phase, garder M4.H.G
≤ 1 jour. Memo dans next steps que l'exposition UI peut se faire
dans une mini-phase ultérieure ou intégrée à une future itération.

### M4.H.G.2 - `Backoff::HANDSHAKE` 30s → 15s

1. Lire `crates/warren-backoff/src/lib.rs` pour identifier la
   constante actuelle `HANDSHAKE` (probable
   `pub const HANDSHAKE: Backoff = Backoff::new(initial=Xs, ceiling=30s)`).
2. Grep consumers cross-repo warren-core :
   ```bash
   grep -rE "Backoff::HANDSHAKE|backoff::HANDSHAKE" warren-core/crates/
   ```
3. Pour chaque consumer identifié, vérifier l'impact d'une baisse
   ceiling 30→15s :
   - MultiHopSupervisor (M4.E.D) : impact direct sur worst-case
     reconnect. POSITIF (31s → ~15s).
   - Autres consumers (NAT-PMP refresh ? warren-tunnel handshake
     retries ?) : vérifier que 15s ceiling est suffisant pour les
     conditions network worst-case.
4. **TDD RED** : test que `Backoff::HANDSHAKE.ceiling() == 15s` après
   modification (avant : 30s).
5. **GREEN** : changer la constante.
6. **REFACTOR** : si l'invariant 30s était documenté `/v1` (figé),
   passer à `/v2` via une nouvelle constante `HANDSHAKE_V2` et
   migrer les consumers. Sinon mutation in-place autorisée.
7. Tests régression : reconnect path simulé converge < 15s p95
   (test mocked-time si possible).
8. Commit warren-core `feat(warren-backoff): tune HANDSHAKE ceiling 30s → 15s for sub-15s worst-case reconnect (M4.E.D follow-up)`.

### M4.H.G.3 - Pin warren-core bump warren-app

1. Bumper `.warren-core-version` côté warren-app au SHA post-M4.H.G.1
   + M4.H.G.2.
2. `cargo check --workspace` warren-app PASS post-pin.
3. Commit warren-app `chore(warren-core-pin): bump for --bypass-cidr + HANDSHAKE tune`.

### M4.H.G.4 - Validation finale

1. Cargo warren-core full :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-core
   ./scripts/dev/cargo-test-nofw.sh fmt --check &
   ./scripts/dev/cargo-test-nofw.sh clippy --workspace --all-targets -- -D warnings
   wait
   ./scripts/dev/cargo-test-nofw.sh test -p warren-client -p warren-backoff -p warren-tunnel
   ```
2. Cargo warren-app full :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-app
   cargo fmt --check &
   cargo clippy --workspace --all-targets -- -D warnings
   wait
   cargo test --workspace --no-fail-fast
   ```
3. Smoke build warren-app : `bash scripts/dev/smoke-build.sh` PASS.

### M4.H.G.5 - Finalize + commits + memory

1. Rapport `/tmp/m4-h-g-report.md` ≤ 100 lignes (scope court).
2. Commits 3-4 atomiques poussés origin/main (warren-core + warren-app).
3. Memory `warren_m4h_g_delivered.md` warren-app + index MEMORY.md.
4. Update source-of-truth orchestrateur si applicable.

---

## 5. Règles non-négociables

### Sécurité

- Pas de secrets verbatim
- Pas de signing key prod touchée
- §0.0 INVIOLABLE git rappelé

### Code

- TDD strict (RED→GREEN→REFACTOR)
- /v1 constantes IMMUABLES (si Backoff::HANDSHAKE est figé /v1 →
  escalade pour /v2 plutôt que mutation in-place)
- Conventional commits subject-only, anglais comments, no em-dash
- IpNetwork parsing : utiliser `ipnetwork` crate (déjà workspace dep)
  pour cohérence avec talpid-warren-tunnel

### Git

- Push main direct warren-app (GitHub WarrenBrowse) + warren-core
- §0.0 INVIOLABLE rappelé en tête
- Pas de feature branch

---

## 6. Pas de validation intermédiaire poka

§0.5. 5 cas d'escalade ONLY.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- **M4.H.G.1** : `--bypass-cidr` câblé warren-client warren-core +
  daemon plumbing warren-app, TDD RED→GREEN PASS, route plus spécifique
  préservée hors tunnel
- **M4.H.G.2** : `Backoff::HANDSHAKE` ceiling 30→15s avec test régression,
  consumers vérifiés non régressifs (notamment MultiHopSupervisor M4.E.D)
- **M4.H.G.3** : pin warren-core bumped warren-app, cargo check PASS
- Cargo validation warren-core + warren-app PASS (fmt + clippy -D
  warnings + test)
- Smoke build warren-app PASS
- 3-4 commits atomiques poussés origin/main des 2 repos
- Memory updates

### GO CONDITIONAL

- 1/2 caveats fixés (ex: --bypass-cidr OK mais Backoff tune
  impacte un consumer non identifié initialement)

### NO-GO HONNÊTE (improbable §0.5)

- Routing 0.0.0.0/0 est posé par OS-level mécanisme hors warren-client
  (probable refactor archi)
- `Backoff::HANDSHAKE` est figé /v1 wire format (escalade /v2)

---

## 8. Rapport final attendu

`/tmp/m4-h-g-report.md` ≤ 100 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO
2. **--bypass-cidr** : API exposed, route plus spécifique posée,
   tests RED→GREEN snippet
3. **Backoff::HANDSHAKE** : 30→15s avec impact consumers vérifié
4. **Pin warren-core** : SHA bumped
5. **Cargo + smoke** : output summary
6. **Caveats résiduels** : exposition UI bypass-cidr différée (memo
   future phase)
7. **Commits** + memory updates

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → scope dev warren-app M4.H pure CLOSED
  (M4.H.A → M4.H.G livrés)
- Reste pour ship beta Warren :
  - **Caveats ops poka** : GH Actions billing + WARREN_CORE_RO_TOKEN
    + signing assets + SSH Hetzner
  - **M4.H.H** doc warrenbrowse.com (scope web séparé, à clarifier
    avec poka)
  - **Exposition UI bypass-cidr** (si retenu future phase)
  - **Stabilisation warren-core PF architecture + H.E.5-7** (en cours
    par d'autres agents poka)

---

## 10. Trace de mémorisation

Warren-app :
- Create `warren_m4h_g_delivered.md`
- Index MEMORY.md : `- [M4.H.G delivered](warren_m4h_g_delivered.md), <verdict> --bypass-cidr SSH inbound + Backoff tune 15s sub-cap`

Warren-core (si commits push) :
- Update memory si pertinent (`warren-client_bypass_cidr.md` ou
  intégration à `warren_m4e_delivered` section M4.H follow-up)
