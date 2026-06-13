# Phase M4.H.E - Caveats fixes courts avant build pipeline

> Brief d'agent autonome warren-app (+ warren-core si nécessaire).
> Doctrine §0.5 full autonomy NO timid rollback.
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 1-1.5 jour (8-10h).
**Coût Hetzner** : 0 EUR (dev pure, pas de bench infra).
**Pré-condition** : warren-app main HEAD `72bdcc4bd5` (post-M4.H.C
+ orchestrator prompt sync) ou descendant. Stack M4.E.D câblée +
UI Electron multi-hop livré.

**Objectif** : éliminer 4 caveats accumulés M4.H.A → C avant
M4.H.D build pipeline :
1. **M4.H.E.1** : reconnect cache wiring (caveat M4.H.C.X), UX
   bug shipping blocker
2. **M4.H.E.2** : daemon-fork `account create` Remote LOCAL=0 factory
   bug (caveat M4.H.A)
3. **M4.H.E.3** : wapi VAL1/2 client-side regression (caveat
   M4.H.A.ter)
4. **M4.H.E.4** : warren-killswitch warren-core orphelin investigation
   (analyse archi orchestrateur 2026-05-20)

Pas dans scope (ops poka, ou feature dédiée) :
- SSH Hetzner bench bug → ops poka
- GHCR PAT write:packages → ops poka
- NAT-PMP client câblage → M4.H.F (différenciateur produit, design
  UI dédié)

---

## 0. MANDAT STRICT

Anti-patterns historiques M4.E §7 + TDD strict warren-core CLAUDE.md
§1 (RED→GREEN→REFACTOR) + /v1 constantes IMMUABLES + tests pertinents
(`feedback_tests_pertinents` warren-core).

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein
mandat. Diagnostic 30 min → fix tactique TDD cross-repo OK → commit
+ push → reprise. PAS de rollback opportuniste, PAS d'escalade pour
"valider l'approche".

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût > 0.30 EUR (n/a, pas de Hetzner)
3. Breaking change /v1 wire format
4. Signing key prod touchée

Verdict NO-GO seulement si après 3h investigation+fix tentés un
caveat dépasse l'agent (probablement archi nécessite refactor large).

Les 4 sous-tâches sont **indépendantes** : tu peux les attaquer dans
n'importe quel ordre. Recommandé séquentiel pour clarté commits.

---

## 1. Optimisations agent

- Lectures sources cross-repo en PARALLÈLE en début de phase
- Cargo validation groupée en fin de chaque sous-tâche logique
- Push origin/main warren-app au fur et à mesure (4 commits min
  attendus)

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main 72bdcc4bd5+
git log --oneline -3
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean
git log --oneline -1                        # HEAD bb9b7895+ (re-export canonical_message)
```

Si HEAD inattendu : escalade.

---

## 3. Sources à lire (PARALLÈLE)

### Warren-app

- `talpid-warren-tunnel/src/lib.rs` (start_multi_hop + supervisor
  spawn point pour M4.H.E.1)
- `mullvad-daemon/src/management_interface.rs` (WarrenStatus +
  WarrenStatusCache, point d'arrivée des reconnect events)
- `mullvad-daemon/src/warren_multi_hop.rs` (PKI loader)
- `mullvad-daemon/src/warren_tunnel_params.rs` (params extended)
- `mullvad-daemon/src/device/` + `account_history.rs` (point d'entrée
  account create factory pour M4.H.E.2)
- `mullvad-api/src/warren_auth.rs` (signing flow)
- `mullvad-daemon/src/warren_account_mode.rs` (toggle Remote/Local)

### Warren-core

- `crates/warren-client/src/multi_hop.rs` (MultiHopSupervisor M4.E.D :
  voir comment il expose reconnect events, callback ou observer pattern)
- `crates/warren-wapi/` ou bin (helper wapi pour M4.H.E.3 VAL1/2 fix)
- `crates/warren-killswitch/src/` (M4.H.E.4 investigation)
- `crates/warren-exit/Cargo.toml` (grep warren-killswitch deps M4.H.E.4)
- `crates/warren-admin/Cargo.toml` (idem)

### Memory cross-session

- `warren_m4h_a_quart_delivered.md` warren-app (caveat factory bug
  Remote LOCAL=0 source)
- `warren_m4h_a_ter_delivered.md` warren-app (caveat wapi VAL1/2 source)
- `project_warren_app_state_post_m4hc.md` warren-app (état post-M4.H.C
  source de vérité orchestrateur, ou post-m4hb si pas créé)
- `feedback_agent_full_autonomy_no_timid_rollback.md` warren-app
- `feedback_warren_phase_prompts_no_branch.md` warren-core

---

## 4. Plan d'exécution

### M4.H.E.1 - Reconnect cache wiring (caveat M4.H.C.X)

**Contexte** : M4.H.C a livré le UI display de `reconnect_count` +
`last_reconnect_age` via gRPC `WarrenStatusUpdates` stream consommant
`WarrenStatusCache` côté daemon. **Mais le cache n'est jamais
incrémenté** : aucun appel `record_reconnect` depuis le supervisor.
Donc UI affiche 0 steady-state.

1. Lire `talpid-warren-tunnel/src/lib.rs` section `start_multi_hop`
   pour identifier où le `MultiHopSupervisor` est spawné.
2. Lire `mullvad-daemon/src/management_interface.rs` pour comprendre
   `WarrenStatusCache::record_reconnect` API.
3. Trouver comment `MultiHopSupervisor` warren-core expose les
   reconnect events :
   - Callback `Fn(ReconnectInfo)` ?
   - Channel `mpsc::Receiver<ReconnectEvent>` ?
   - Observer trait ?
   - Inspect `crates/warren-client/src/multi_hop.rs` warren-core.
4. Câbler : passer une closure ou un sender au supervisor à la
   construction, qui appelle `WarrenStatusCache::record_reconnect`
   à chaque reconnect.
5. **TDD** :
   - RED : test qui spawn un supervisor avec un cache mock, simule
     un reconnect (force drop + reconnect), assert le cache a 1
     record_reconnect call avec timestamp cohérent.
   - GREEN : appliquer le câblage.
   - Test régression : multi-hop tunnel + simulated exit drop →
     `reconnect_count` incrémenté + `last_reconnect_age` updated.
6. Commit `fix(talpid-warren-tunnel): wire WarrenStatusCache.record_reconnect from MultiHopSupervisor`.
7. Push.

### M4.H.E.2 - Factory bug daemon-fork `account create` Remote LOCAL=0

**Contexte** : caveat M4.H.A : en mode Remote (`WARREN_LOCAL_ACCOUNT=0`),
`account create` plante avec `Set account number on factory with no
access token store`. Bug probable : la migration Iroh→Quinn a oublié
de wire l'access token store factory pour mode Remote (mode LOCAL=1
bypass cette factory, donc passe).

1. Reproduire le bug : `WARREN_LOCAL_ACCOUNT=0` + tentative `mullvad-cli
   account create` (ou équivalent) → expected error message.
2. Identifier le code path :
   - `mullvad-daemon/src/device/` (DeviceService, account factory)
   - `mullvad-api/src/access.rs` (post-rebrand warren-auth)
   - `mullvad-daemon/src/account_history.rs`
3. Grep "Set account number on factory" → trouver l'emplacement strict
   du message.
4. Identifier pourquoi la factory est instanciée sans access token
   store. Cause probable : la migration Iroh→Quinn (commit `7afa9b9d31
   refactor(mullvad-daemon): port Warren paths to WarrenPubkey +
   WarrenExitAddr`) a oublié de wire le store côté Remote.
5. **TDD** :
   - RED : test que `account create` mode Remote produit l'erreur
     verbatim actuel (= reproduit le bug).
   - GREEN : fix wire l'access token store, test devient PASS.
   - REFACTOR : si applicable, simplifier les dépendances.
6. Vérifier qu'on ne casse PAS le mode LOCAL=1 (test régression).
7. Commit `fix(mullvad-daemon): wire access token store factory for Remote account mode`.
8. Push.

### M4.H.E.3 - wapi VAL1/2 client-side regression

**Contexte** : caveat M4.H.A.ter : 2 smoke tests bench/scripts/test-
backend-smoke.sh FAIL car wapi rejette `country=FRANCE/A1` côté client
au lieu de let server respond 400. Doctrine attendue : validation
client-side stricte sur le format ISO 3166 (2 chars `[A-Z]{2}`). Donc :
- `FRANCE` (6 chars) DOIT être rejeté côté client (= le test smoke
  attend que le server répond 400, mais wapi reject earlier)
- `A1` (chiffre dans char) DOIT être rejeté côté client

**Décision tactique** : 2 options
- **Option α** : patch wapi pour laisser passer ces 2 inputs et let
  server respond 400 (= aligner sur le smoke test).
- **Option β** : patch test-backend-smoke.sh pour skip ces 2 cas ou
  changer les assertions (server response = 400 est ce qu'on attend,
  donc wapi reject early avec status 400-like est acceptable côté
  smoke).

**Recommandation** : Option β plus sûre (validation client-side stricte
est une bonne pratique no-log Warren : pubkey + country sont validés
strictement avant signature pour éviter de loguer du junk). Patch
script smoke pour assert que `wapi` reject avec code error 1 (ou
spécifique) au lieu de assert response server 400.

1. Lire le script `bench/scripts/test-backend-smoke.sh` warren-core
   pour identifier exactement les 2 assertions VAL1+VAL2.
2. Lire `crates/warren-wapi/` warren-core pour comprendre la logique
   client-side validation.
3. Patcher le script smoke : VAL1 + VAL2 doivent assert wapi reject
   client-side avec exit code spécifique (pas response server).
4. Vérifier que `bench/scripts/test-backend-smoke.sh` repasse 62/62.
5. **TDD côté wapi** si modif logique : pas applicable si option β.
   Si Option α : tests qui assertent wapi forwards malformed input.
6. Commit warren-core : `test(bench-scripts): VAL1/VAL2 assert wapi
   client-side validation` (ou wapi fix selon décision).
7. Push warren-core.

### M4.H.E.4 - warren-killswitch warren-core investigation

**Contexte** : analyse archi 2026-05-20 a identifié que `warren-
killswitch` (1608 LOC warren-core) n'est pas consommé par warren-app
(qui utilise `talpid-core` firewall upstream Mullvad). Est-il consommé
par warren-exit ou warren-admin warren-core ? Ou orphelin total ?

1. Grep cross-repo :
   ```bash
   grep -r "warren-killswitch\|warren_killswitch" \
     /Users/poka/dev/warrenBros/warren-core/crates/ \
     /Users/poka/dev/warrenBros/warren-core/Cargo.toml \
     /Users/poka/dev/warrenBros/warren-app/
   ```
2. Identifier consommateurs réels.
3. **3 verdicts possibles** :
   - **A** Consommé par warren-exit OU warren-admin warren-core :
     orphelin côté warren-app uniquement, conserver dans warren-core.
   - **B** Orphelin total (jamais utilisé nulle part) : candidat à
     archive/suppression warren-core. **Escalader poka** (décision
     business, pas tactique).
   - **C** Consommé partiellement (ex: test only) : conserver mais
     documenter dans memory que c'est minimal usage.
4. Créer une memory warren-core `warren_killswitch_audit.md`
   documentant le verdict (sans action de suppression unilateral).
5. Pas de commit code dans cette sous-tâche (juste investigation +
   memory).
6. Si verdict B → escalade poka avec recommandation (archive ou
   conserver pour avenir).

### M4.H.E.5 - Validation finale + finalize

1. Cargo validation full warren-app + warren-core :
   ```bash
   # warren-app
   cargo fmt --check &
   cargo clippy --workspace --all-targets -- -D warnings
   wait
   cargo test --workspace --no-fail-fast
   # warren-core (si touché par E.3)
   ./scripts/dev/cargo-test-nofw.sh fmt --check &
   ./scripts/dev/cargo-test-nofw.sh clippy --workspace --all-targets -- -D warnings
   wait
   ./scripts/dev/cargo-test-nofw.sh test --workspace
   ```
2. Tests UI Jest warren-app : `cd desktop && npm test` PASS attendu
   (M4.H.E ne touche pas l'UI sauf indirectement).
3. Smoke `bench/scripts/test-backend-smoke.sh` 62/62 PASS post-fix
   VAL1/2.
4. Rapport `/tmp/m4-h-e-report.md` ≤ 150 lignes.
5. Memory `warren_m4h_e_delivered.md` warren-app + index MEMORY.md.
6. Update source-of-truth orchestrateur (`project_warren_app_state_
   post_m4hc.md` ou créer `post_m4he.md`).

---

## 5. Règles non-négociables

### Sécurité

- Pas de secrets verbatim (`feedback_warren_no_secrets_in_commits`)
- Pas de signing key prod touchée
- M4.H.E.4 : pas de suppression unilaterale warren-killswitch sans
  validation poka

### Code

- TDD strict (RED→GREEN→REFACTOR) sur les fixes M4.H.E.1 + .2
- /v1 constantes IMMUABLES
- Conventional commits subject-only, anglais comments, pas em-dash,
  pas TODO laissés

### Git

- Push main direct warren-app + warren-core (si touché)
- 4-5 commits min attendus :
  1. `fix(talpid-warren-tunnel)` reconnect cache wiring M4.H.E.1
  2. `fix(mullvad-daemon)` factory access token Remote M4.H.E.2
  3. `test(bench-scripts)` OR `fix(warren-wapi)` VAL1/2 M4.H.E.3
  4. memory warren_killswitch_audit M4.H.E.4 (warren-core memory dir,
     pas un commit repo)
  5. `docs(M4.H.E)` delivery report

---

## 6. Escalade poka

Cf. §0.5. 4 cas ONLY. Cas additionnel M4.H.E.4 : si verdict B
(orphelin total warren-killswitch), escalade pour décision archive
ou conserver.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- **M4.H.E.1** : reconnect cache câblé, test régression PASS,
  `reconnect_count` UI live update sur reconnect simulé
- **M4.H.E.2** : factory bug Remote LOCAL=0 FIXÉ, test régression
  account create mode Remote PASS, mode LOCAL=1 non régressé
- **M4.H.E.3** : smoke `test-backend-smoke.sh` 62/62 PASS, VAL1+VAL2
  assertions cohérentes avec validation client-side
- **M4.H.E.4** : verdict warren-killswitch documenté en memory
  warren-core (A/B/C), action escalade si B
- Cargo validation warren-app + warren-core (si touché) full PASS
- 4-5 commits poussés origin/main des repos touchés
- Memory `warren_m4h_e_delivered.md` warren-app + index

### GO CONDITIONAL

- 3/4 sous-tâches livrées, 1 caveat documenté avec cause root précise
  (probable scope post-M4.H.E)

### NO-GO HONNÊTE (improbable §0.5)

- Caveat critique nécessite breaking change /v1
- 3h+ investigation+fix sans progrès sur l'un des 4

---

## 8. Rapport final attendu

`/tmp/m4-h-e-report.md` ≤ 150 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO
2. **M4.H.E.1 reconnect wiring** : commit + tests régression + diff
   architecture
3. **M4.H.E.2 factory fix** : commit + RED→GREEN trace + tests
4. **M4.H.E.3 wapi VAL1/2** : option choisie + commit + smoke 62/62
5. **M4.H.E.4 warren-killswitch** : verdict + memory créé
6. **Cargo validation** : output summary
7. **Caveats résiduels** post-E (SSH Hetzner + GHCR PAT ops poka +
   NAT-PMP future M4.H.F)
8. **Commits** + memory updates

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → débloque M4.H.D (build pipeline DMG/AppImage/MSI
  + signing + CI release). UI now ship-ready avec live reconnect
  counter.
- **CONDITIONAL** → pondérer si le caveat restant bloque M4.H.D ship.
- **NO-GO** → escalade poka.

Caveats persistants ops poka après M4.H.E :
- SSH Hetzner bench auth bug → résoudre avant M4.H.D bench E2E final
- GHCR PAT poka-IT write:packages → résoudre avant M4.H.D CI cosign
  push (peut être différé en M4.H.D.bis)

Phase future :
- M4.H.F : câblage NAT-PMP client + UI port-forwarding (différenciateur
  produit, vs Mullvad/IVPN qui ont abandonné 2023)

---

## 10. Trace de mémorisation

Warren-app :
- Create `warren_m4h_e_delivered.md`
- Update source-of-truth orchestrateur (`project_warren_app_state_
  post_m4hc.md` post-add section M4.H.E ou create post_m4he.md)
- Index MEMORY.md : `- [M4.H.E delivered](warren_m4h_e_delivered.md), <verdict> 4 caveats fixés avant M4.H.D build pipeline`

Warren-core :
- Si M4.H.E.3 patch warren-core : update memory si pertinent
- M4.H.E.4 verdict : create memory `warren_killswitch_audit.md`
  warren-core
