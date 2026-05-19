# Phase M4.H.A.ter - Burn down + redeploy 2-node prod from HEAD + re-bench cross-DC

> Brief d'agent autonome. Suit M4.H.A.bis (verdict NO-GO rollback,
> découverte couplage warren-exit ↔ warren-backend-api).
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 2-3 heures.
**Coût Hetzner** : ~0.10-0.15 EUR (2 CCX23 prod préservés sur mêmes IPs +
1 CCX23 nbg1 client bench transient).
**Pré-condition** :
- warren-app main HEAD `83b7dab396` (post-M4.H.A.bis commit) ou descendant
- warren-core HEAD `b522e3c` (M4.E.C.quint+M4.E.D) ou descendant
- poka autorise burn-down complet de la "prod" actuelle (= stack de test,
  pas de vrais clients à préserver)
- DNS `api.warrenbrowse.com` + exit endpoints pointent vers les IPs
  Hetzner actuelles des 2 nodes prod (à préserver via SSH-replay OU
  via `hcloud server rebuild`)

**Objectif** : burn down + redeploy from scratch warren-backend-api +
warren-exit-1 depuis warren-core HEAD `b522e3c` sur les mêmes IPs (pour
préserver les DNS sans bascule), puis re-bench daemon-fork warren-app
↔ warren-exit-1 cross-DC pour fermer la boucle perf empirique. Verdict
attendu : GO ULTIMATE.

---

## 0. MANDAT STRICT

### Anti-patterns interdits

1. **Pas de changement d'IP des 2 nodes prod**. DNS pointe déjà bien.
   Méthode : SSH-replay clean wipe (recommandé) OU `hcloud server rebuild`
   si CLI supporte. Si `hcloud server delete + create` envisagé : STOP
   escalade poka (impacte DNS).
2. **Pas de migration de state**. Le state SQLite warren-backend-api
   v0.2.7 actuel est à JETER. Fresh DB, fresh signing keys admin, fresh
   allowlist. Poka regénérera depuis HEAD avec les helper bins.
3. **Pas de toucher au DNS warrenbrowse.com**. Déjà câblé.
4. **Pas de "garder un peu" du state actuel**. Burn down COMPLET signing
   keys, DBs, allowlist caches, configs avant deploy fresh.
5. **Lecture des scripts provision avant action**. Identifier le script
   exact warren-core (`provision-warren-api.sh`, `provision-warren-exit.sh`
   ou équivalent) AVANT toute commande. Si scripts absents/obsolètes :
   STOP escalade poka.

### Comportement attendu

- Procédure ordonnée : burn-down → fresh provision → handshake smoke →
  bench → tear-down node client.
- Si fresh deploy fail (warren-backend-api ne up pas, ou warren-exit
  fail at boot) : escalade poka IMMÉDIATE avec output verbatim.
  Pas de "essayons de débugger en hot sur prod fresh".
- Verdict honnête GO ULTIMATE / GO CONDITIONAL / NO-GO.

---

## 1. Optimisations agent

- Parallélisme : cross-compile warren-exit + warren-backend-api en
  parallèle (cargo workspace ou 2 cargo build &).
- scp parallèle des 2 binaires vers leurs nodes respectifs.
- SSH commands en parallèle (`ssh node1 ... & ssh node2 ... ; wait`)
  où sans dépendance.
- Une seule cargo passe en fin de phase pour validation.

---

## 2. Setup initial obligatoire

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main
git log --oneline -3                        # verify M4.H.A.bis included

cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean (or document delta)
git log --oneline -1                        # b522e3c or descendant

export WARREN_SSH_KEY=pokash
export HCLOUD_CONTEXT=warren                # never poka-perso
hcloud server list                          # capture current IPs of warren-exit-1 + warren-backend-api
```

Si HEADs non conformes : escalade.

---

## 3. Sources à lire (PARALLÈLE)

### Repo warren-core

- `bench/scripts/setup-warren-api.sh` ou `provision-warren-api.sh` ou
  équivalent (identifier la procédure officielle de boot warren-backend-api)
- `bench/scripts/provision-warren-exit.sh` (procédure warren-exit deploy)
- `bench/scripts/test-backend-smoke.sh` (Tier 1 smoke 62 assertions ~10s
  - cf. memory `warren_test_scripts.md`)
- `bench/scripts/test-exit-lifecycle.sh` (Tier 2 Hetzner)
- `crates/warren-api/Cargo.toml` (warren-backend-api binaire HEAD)
- `crates/warren-exit/src/main.rs` (warren-exit HEAD, flags, ENV)
- Memory `warren_backend_server.md` (warren-backend-api v0.2.7 PROD info
  pré-burn : IP `204.168.244.76`, /srv/warren, mTLS CA)
- Memory `warren_prod_admin_key_location.md` (admin signing key path
  pour mint voucher post-fresh-deploy)
- Memory `warren_test_scripts.md` (E2E test scripts catalogue)

### Repo warren-app

- `.planning/m4-h-a-bis-brief.md` + memory `warren_m4h_a_bis_delivered.md`
  (procédure deploy + rollback validée, couplage version exposé)
- `docs/m4-h-a-bis-bench-cross-dc-verdict.md` (rapport bench M4.H.A.bis)

### Memory cross-session warren-core

- `feedback_warren_no_secrets_in_commits.md` (test mnemonics + signing
  keys → out-of-repo, fs ephemeral)
- `feedback_warren_hetzner_bench_ops_gotchas.md`
- `warren_obfuscation_doctrine_v1.md` (M4.0 invariants warren-exit-1 doit
  exposer post-fresh-deploy)

---

## 4. Plan d'exécution

### M4.H.A.ter.0 - Pre-flight + inventaire

1. SSH warren-backend-api PROD : capturer toute la config actuelle (path
   binaire, ExecStart systemd, ENV vars, paths SQLite + mTLS CA + admin
   signing). Logger `/tmp/warren-api-pre-burn-state.txt` LOCAL.
2. SSH warren-exit-1 PROD : idem, capturer `/tmp/warren-exit-pre-burn-state.txt`.
3. `hcloud server describe warren-backend-api` + `warren-exit-1` :
   capturer IPs + datacenter + type + image. Logger LOCAL.
4. Identifier méthode redeploy :
   - **A (recommandée)** : SSH-replay clean wipe (stop services, rm /srv,
     rm signing keys, rm allowlist caches, re-cloud-init via SSH script)
   - **B** : `hcloud server rebuild` si CLI supporte (vérifier
     `hcloud server --help`). Préserve IP.
   - **C** (DERNIER RECOURS escaladable) : `hcloud server delete + create`.
     STOP avant si poka pas validé (impact DNS).
5. Choix méthode + documentation in report.

### M4.H.A.ter.1 - Cross-compile binaires warren-backend-api + warren-exit

```bash
cd /Users/poka/dev/warrenBros/warren-core
./scripts/dev/cargo-test-nofw.sh build --release --target x86_64-unknown-linux-gnu -p warren-api -p warren-exit
```

Vérifier `target/x86_64-unknown-linux-gnu/release/{warren-api,warren-exit}` présents.

NOTE : warren-api est probablement le binaire warren-backend-api. Si le
nom diffère (warren-backend-api séparé), adapter le `-p`. Lire
`bench/scripts/setup-warren-api.sh` pour confirmer.

### M4.H.A.ter.2 - Burn down PROD (méthode A SSH-replay)

Séquentiel (pas parallèle, on veut savoir lequel des 2 fail si fail) :

1. **warren-backend-api** :
   - SSH : `systemctl stop warren-api` (ou nom exact service)
   - `rm -rf /srv/warren/` (ou path data dir capturé)
   - `rm -rf /etc/warren-api/` (ou path config dir)
   - `userdel warren-api` si user dédié
   - `rm /usr/local/bin/warren-api` ou path binaire
   - Vérifier `systemctl status warren-api` = inactive (dead)
2. **warren-exit-1** :
   - SSH : `systemctl stop warren-exit`
   - `rm -rf /var/lib/warren-exit/`
   - `rm -rf /etc/warren-exit/`
   - `rm /usr/local/bin/warren-exit`
   - Vérifier inactive
3. Confirmer no rogue process : `ps aux | grep warren` sur les 2 nodes.

### M4.H.A.ter.3 - Fresh deploy depuis HEAD

1. **warren-backend-api fresh** :
   - scp binaire `target/x86_64-unknown-linux-gnu/release/warren-api`
     vers `/usr/local/bin/warren-api`
   - Replay cloud-init OU run script `bench/scripts/setup-warren-api.sh`
     adapté pour init fresh (créer user, dirs, mTLS CA fresh, signing
     key admin fresh, SQLite vide, systemd unit)
   - `systemctl daemon-reload && systemctl enable --now warren-api`
   - Vérifier 60s : `journalctl -u warren-api -n 30` = no ERROR, no panic,
     port binding OK (probable 443/tcp via Caddy ou direct)
   - Smoke `bench/scripts/test-backend-smoke.sh` Tier 1 (62 assertions
     ~10s) → MUST PASS
2. **warren-exit-1 fresh** :
   - Identifier signing key admin nouvellement générée côté warren-api,
     mint un descriptor exit OU laisser warren-exit s'enregistrer en
     auto-registration si supporté HEAD
   - scp binaire `warren-exit` vers `/usr/local/bin/warren-exit`
   - Replay script `bench/scripts/provision-warren-exit.sh` ou équivalent
   - `systemctl daemon-reload && systemctl enable --now warren-exit`
   - Vérifier 60s : journalctl no ERROR, allowlist poll OK (pas de
     fail-closed après 300s grace cette fois), TUN+NAT-PMP+DNS forwarder
     bound
3. Smoke handshake daemon-fork warren-app local → warren-exit-1 fresh :
   PASS attendu (wire format aligné HEAD vs HEAD).

### M4.H.A.ter.4 - Bench cross-DC

1. Provision 1 CCX23 nbg1 (client bench). Si nbg1 reusable depuis
   M4.H.A.bis encore vivant : skip (gain).
2. Bench scenario reproductible (utiliser `run-bench-v2.sh` ou équivalent
   M4.H.A.bis adapté). Inclut :
   - REF baseline cross-DC iperf3 TCP 1-flow + 4-flow + UDP 1G (sanity
     link)
   - WARREN scenarios via daemon-fork :
     - TCP 1-flow sustained 5 min
     - TCP 4-flow sustained 5 min
     - PMTU négocié observable
     - Stall counter ≥ 5s
     - errors / decode_failures / replay_rejects compteurs
     - RSS warren-exit-1 + daemon-fork client sur 5 min
3. Tear-down nbg1 obligatoire.

### M4.H.A.ter.5 - Verdict + commits

1. Rapport `/tmp/m4-h-a-ter-report.md` ≤ 150 lignes (§8).
2. Commit côté warren-app main :
   - `bench(M4.H.A.ter): cross-DC verdict post 2-node fresh deploy HEAD warren-core`
   - Push origin/main
3. Memory update warren-app `warren_m4h_a_ter_delivered.md` + index.
4. **Update memory warren-core `warren_backend_server.md`** : noter que
   la stack PROD a été refresh from HEAD `b522e3c`, plus v0.2.7. Mettre
   à jour les coordonnées (path signing key admin nouvelle, etc.) car
   les anciennes en memory sont stale post-burn.

---

## 5. Règles non-négociables

### Sécurité

- Pas de seeds/mnemonics/private keys verbatim en commits ou rapports
  (cf. `feedback_warren_no_secrets_in_commits`). Test artifacts in
  `/tmp/m4-h-a-ter/` LOCAL ou ephemeral, wipés en fin de phase.
- Nouvelles signing keys admin warren-api : path doit être documenté en
  memory `warren_prod_admin_key_location.md` warren-core POST-deploy
  (sans reproduire le contenu).
- mTLS CA fresh : conserver dans data dir warren-api uniquement.

### Code

- Anglais comments, pas em-dash, pas TODO trail, conventional commits
  subject-only, pas Co-Authored-By Claude.

### Git

- Push main direct warren-app.
- Memory update warren-core via Write (memory dir hors repo).

### Bench Hetzner

- Tear-down nbg1 bench obligatoire.
- warren-exit-1 + warren-backend-api RESTENT UP en fin de phase (mêmes
  IPs, stack HEAD).
- `hcloud server list` final = warren-exit-1 + warren-backend-api seuls.

---

## 6. Pas de validation intermédiaire poka

Escalade `AskUserQuestion` SEULEMENT si :

1. Script provision warren-api / warren-exit obsolète ou absent côté
   warren-core HEAD.
2. Méthode redeploy nécessite `hcloud server delete + create` (impact DNS).
3. Fresh deploy fail à boot (warren-api ou warren-exit ne up pas en 60s).
4. Smoke Tier 1 warren-api fail (62 assertions ≥ 1 fail).
5. Coût Hetzner > 0.20 EUR (seuil monté vs M4.H.A.bis car burn down + 2
   provision = plus cher).
6. Découverte sécurité (secret leak, signing key path inhabituel).

Sinon autonomie complète.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- warren-backend-api fresh from HEAD `b522e3c+`, active running, smoke
  Tier 1 PASS (62 assertions)
- warren-exit-1 fresh from HEAD, active running, allowlist polling OK
  (pas de fail-closed grace evicted)
- Smoke handshake daemon-fork → warren-exit-1 PASS
- Bench cross-DC :
  - TCP 4-flow sustained ≥ 200 Mbps sur 5 min
  - 0 stall ≥ 5s, 0 errors, 0 decode_failures
  - PMTU ≥ 1280 négocié (idéalement 1379 GSO)
  - RSS warren-exit-1 stable < +10 MB sur 5 min
- Mêmes IPs préservées (DNS intact)
- Tear-down nbg1 attesté

### GO CONDITIONAL

- Stack HEAD up, mais throughput 150-200 Mbps OU variance 10-20%

### NO-GO HONNÊTE

- Provision fresh fail (warren-api ou warren-exit ne up pas)
- OU bench throughput < 150 Mbps après stack HEAD complete
- OU régression vs baseline M4.E.C.quint warren-core empirique

---

## 8. Rapport final attendu

`/tmp/m4-h-a-ter-report.md` ≤ 150 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO + 1 phrase
2. **Burn down** : ce qui a été wiped (paths, services), confirmation
   ps clean
3. **Cross-compile** : SHA warren-core, binaires sizes, build time
4. **Méthode redeploy** : A (SSH-replay) OU B (rebuild) OU C (escaladé)
5. **Fresh deploy** : warren-api smoke Tier 1 résultats, warren-exit
   journalctl excerpt
6. **Smoke handshake** : PASS/fail
7. **Bench results** tableau throughput / PMTU / errors / RSS
8. **Caveats** résiduels
9. **Coût Hetzner** + tear-down nbg1 attesté
10. **Commits** + memory updates (warren-app + warren-core)

---

## 9. Next steps post-phase (orchestrateur)

- Si GO ULTIMATE : débloque M4.H.B (câblage stack M4.E.D complet sur
  warren-app). Le scope du brief M4.H.B sera : path-deps additionnels
  warren-multihop + warren-client + warren-relay + warren-backoff,
  MultiHopSupervisor au tunnel state machine, tests régression.
- Si CONDITIONAL : pondérer caveats vs M4.H.B scope.
- Si NO-GO : déboucher cause root avant M4.H.B (analyse logs détaillée).
- Caveat M4.H.A `account create` factory bug Remote LOCAL=0 : à intégrer
  scope M4.H.B ou M4.H.E.
- Le couplage de version warren-exit ↔ warren-backend-api découvert
  M4.H.A.bis : acter en memory warren-core comme leçon ops, point
  d'attention pour tout futur upgrade isolé.

---

## 10. Trace de mémorisation

Memory à créer :
- `warren_m4h_a_ter_delivered.md` warren-app
- Update `warren_backend_server.md` warren-core (coordonnées POST-burn :
  IP/path/SHA déployée, nouvelles paths signing keys, état SQLite fresh)
- Optionnel : nouvelle memory warren-core
  `warren_exit_api_version_coupling.md` documentant la leçon M4.H.A.bis
  (warren-exit ne peut être redeployé sans co-deploy warren-backend-api).

Index `MEMORY.md` warren-app :
`- [M4.H.A.ter delivered](warren_m4h_a_ter_delivered.md) — <verdict> burn-down 2-node + fresh deploy HEAD + cross-DC bench`
