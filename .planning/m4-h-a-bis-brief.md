# Phase M4.H.A.bis - Redeploy warren-exit-1 prod + re-bench cross-DC

> Brief d'agent autonome. Suit M4.H.A (verdict GO CONDITIONAL).
> Cible : fermer la boucle perf cross-DC empirique en alignant warren-exit-1
> prod sur warren-core HEAD post-M4.0 (= post-M4.E.C.quint).
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 1-2 heures (chantier ops chirurgical).
**Coût Hetzner** : ~0.05 EUR (1 CCX23 nbg1 client recyclé ou neuf ~30-45 min).
**Pré-condition** :
- warren-app main HEAD `22d4a6e1bb` (post-M4.H.A bench commit) ou descendant
- warren-core HEAD `b522e3c` ou descendant (M4.E.C.quint+M4.E.D)
- warren-exit-1 prod hel1 ACCESSIBLE via SSH (ssh key `pokash`)
- poka autorise fenêtre maintenance 5 min sur warren-exit-1 prod

**Objectif** : redeployer le binaire `warren-exit` sur warren-exit-1 PROD hel1
depuis warren-core@`b522e3c`, vérifier que le service repasse up, allowlist
re-syncée, puis re-bench daemon-fork warren-app ↔ warren-exit-1 cross-DC
pour valider perf empirique. Verdict attendu : GO ULTIMATE.

---

## 0. MANDAT STRICT

### Anti-patterns interdits (renforcés vs M4.H.A)

1. **Pas de modification de la signing key warren-exit-1 prod**. Le redeploy
   réutilise la signing key existante (poka prod). Si binaire pre-deploy
   propose de regénérer, REFUSER. La signing key vit dans
   `/srv/warren-exit/signing.key` (ou path équivalent, à confirmer par SSH
   inspection avant tout).
2. **Pas de toucher à warren-backend-api**. Service séparé, hors scope.
3. **Pas de toucher au config de warren-exit-1** (allowlist, port, ALPN,
   etc.). Seul le BINAIRE change.
4. **Rollback prêt avant deploy**. Backup du binaire actuel sur le serveur
   AVANT de swap. Plan rollback testé mentalement (cp old back + systemctl
   restart).
5. **Fenêtre courte** : objectif downtime warren-exit-1 ≤ 5 min. Si le
   service ne repasse pas up dans les 5 min post-restart, ROLLBACK immédiat
   sans investiguer.
6. **Pas de "voyons si ça marche en hot"**. Procédure linéaire : SSH
   inspect → stop → backup → deploy → start → verify → bench. Pas de
   parallèlisation sur la séquence deploy elle-même.

### Comportement attendu

- Verdict GO ULTIMATE / GO CONDITIONAL / NO-GO honnête.
- Si rollback nécessaire : verdict NO-GO sans honte, retour à warren-exit-1
  prod binaire 2026-05-13 acceptable. M4.H.A.bis.bis sera drafté si besoin.

---

## 1. Optimisations agent

- Parallélisme : pre-flight checks SSH + cross-compile en parallèle.
- Cross-compile une seule fois en local (warren-app + warren-core SSDs).
- scp parallèle si plusieurs artifacts (warren-exit binaire + éventuels
  helpers PKI).
- Pas de cargo check intermédiaire entre micro-edits.

---

## 2. Setup initial obligatoire

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # MUST clean main
git log --oneline -3                        # verify HEAD includes M4.H.A
cd /Users/poka/dev/warrenBros/warren-core
git log --oneline -1                        # MUST b522e3c or descendant
export WARREN_SSH_KEY=pokash                # gotcha
export HCLOUD_CONTEXT=warren                # never poka-perso
ssh -i ~/.ssh/pokash root@<warren-exit-1 IP>  # MUST succeed
```

Si SSH fail ou HEAD warren-core insuffisant : STOP, escalade poka.

---

## 3. Sources à lire (PARALLÈLE)

### Repo warren-core

- `crates/warren-exit/src/main.rs` (binaire entry point, flags `--multihop`,
  `--allow-anonymous-clients`, `--warren-api-url`, `--use-tun`, `--info-out`,
  cf. memory `warren_m4e_delivered.md` M4.E section)
- `crates/warren-exit/cloud-init.yaml` ou `provision-warren-exit.sh` si
  existant (référence pour invariants deploy)
- Memory `warren_backend_server.md` warren-core (warren-backend-api PROD info)
- Memory `warren_prod_admin_key_location.md` warren-core (admin signing
  key path SI besoin de re-signer un descriptor exit, normalement NON pour
  ce M4.H.A.bis)

### Repo warren-app

- `.planning/m4-h-a-brief.md` + `docs/m4-h-a-bench-cross-dc-verdict.md` du
  bench M4.H.A (pour comprendre exactement le wire mismatch observé)
- Memory `warren_m4h_a_delivered.md` warren-app (résultats détaillés
  M4.H.A)

### Memory cross-session warren-core

- `warren_obfuscation_doctrine_v1.md` (M4.0 ALPN h3 + SNI + Initial split
  + port 443 + spin bit random, MUST être actifs côté warren-exit-1 post-
  redeploy pour matcher daemon-fork)
- `warren_m4e_delivered.md` section M4.E.D auto-reconnect (le redeploy
  amène aussi cette feature côté warren-exit-1)
- `feedback_warren_hetzner_bench_ops_gotchas.md`

---

## 4. Plan d'exécution

### M4.H.A.bis.0 - Pre-flight + SSH inspection warren-exit-1 prod

1. Vérifier `git status` + HEADs (cf. §2).
2. SSH warren-exit-1 prod :
   - Identifier path binaire actuel : `systemctl cat warren-exit` ou
     `which warren-exit` ou `ps aux | grep warren-exit` → noter ExecStart
     + flags + version (probable `/usr/local/bin/warren-exit` ou
     `/srv/warren-exit/warren-exit`).
   - Noter ENV/flags : `--warren-api-url`, `--allow-anonymous-clients`
     vs no, `--multihop` vs no, `--use-tun`, port binding (443 ou 7000).
   - Noter path signing key.
   - Noter path config / allowlist cache (`/var/lib/warren-exit/allowlist.json`
     ou similaire).
   - `journalctl -u warren-exit -n 20` : capter état actuel (running OK).
3. Logger ces facts dans `/tmp/warren-exit-1-current-state.txt` local
   pour rollback reference.

### M4.H.A.bis.1 - Cross-compile warren-exit Linux

1. Depuis `/Users/poka/dev/warrenBros/warren-core/` :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-core
   ./scripts/dev/cargo-test-nofw.sh build --release --target x86_64-unknown-linux-gnu -p warren-exit
   ```
2. Vérifier binaire `target/x86_64-unknown-linux-gnu/release/warren-exit`
   présent + `file` montre `ELF 64-bit LSB executable`.
3. Note : warren-exit doit être built depuis warren-core HEAD `b522e3c+`,
   pas depuis le SHA pinné par warren-app `.warren-core-version`. Le pin
   warren-app est pour les path-deps daemon-fork, le warren-exit binaire
   est indépendant.

### M4.H.A.bis.2 - Backup + deploy warren-exit-1 prod

1. SSH warren-exit-1 prod :
   - `cp /path/to/warren-exit /path/to/warren-exit.backup-pre-M4HAbis`
   - Vérifier permissions + ownership backup matchent original.
2. scp `target/x86_64-unknown-linux-gnu/release/warren-exit` →
   `root@<warren-exit-1-IP>:/tmp/warren-exit.new`.
3. SSH warren-exit-1 prod :
   - `chmod +x /tmp/warren-exit.new`
   - `/tmp/warren-exit.new --version` : vérifier output (doit refléter
     b522e3c+, M4.E.C.quint).
   - `systemctl stop warren-exit`
   - `mv /tmp/warren-exit.new /path/to/warren-exit`
   - `systemctl start warren-exit`
   - `sleep 3 && systemctl status warren-exit` : MUST active running
   - `journalctl -u warren-exit -n 30` : aucun ERROR, no panic
   - `ss -ulnp | grep <port>` : binding correct
4. **Si service ne up pas en 60s** :
   - `systemctl stop warren-exit`
   - `mv /path/to/warren-exit.backup-pre-M4HAbis /path/to/warren-exit`
   - `systemctl start warren-exit`
   - Verdict NO-GO, rapport rollback + cause root suspectée

### M4.H.A.bis.3 - Allowlist re-sync + smoke

1. SSH warren-exit-1 :
   - `journalctl -u warren-exit -f` : observer 30s pour confirmer
     allowlist refresh tick (poll `/v1/subscribers/active`)
   - `cat /var/lib/warren-exit/allowlist.json` (ou path équivalent) :
     MUST contenir au moins 1 pubkey (poka prod)
2. Smoke depuis warren-app (local macOS ou nbg1 recyclé) :
   - Run un quick handshake test daemon-fork warren-app → warren-exit-1
     (si pubkey poka prod déjà connue par warren-app daemon, sinon
     re-enroll pubkey de test via wapi voucher).
   - Si handshake PASS → wire compat OK.

### M4.H.A.bis.4 - Re-bench cross-DC

1. Provisionner ou recycler 1 CCX23 nbg1 (client warren-app daemon).
   Si nbg1 CCX23 M4.H.A encore vivant : skip (gain temps + coût).
2. Re-run `bench/scripts/fork-e2e-linux.sh` warren-core OU adapter
   `run-bench-v2.sh` utilisé en M4.H.A (déjà testé + pré-seedé).
3. Critères perf (cf. §7).
4. Tear-down nbg1 CCX23 (laisser warren-exit-1 prod hel1 INTACT, c'est
   le but).

### M4.H.A.bis.5 - Verdict + commits + push

1. Drafter rapport `/tmp/m4-h-a-bis-report.md` (cf. §8).
2. Commit côté warren-app main :
   - `bench(M4.H.A.bis): cross-DC verdict post-warren-exit-1 redeploy <SHA>`
   - Inclut report bench + memory update warren-app
   - Push `origin/main`
3. Pas de commit côté warren-core (rien n'a changé côté code warren-core,
   juste binaire deploy).
4. Memory update warren-app : `warren_m4h_a_bis_delivered.md` créé +
   indexé dans MEMORY.md.

---

## 5. Règles non-négociables

### Sécurité ops prod

- JAMAIS toucher la signing key warren-exit-1 prod.
- JAMAIS toucher warren-backend-api.
- JAMAIS rotate ou regenerate quoi que ce soit côté allowlist / PKI.
- Si découverte d'un secret sur warren-exit-1 (signing.key dans un
  endroit insolite) : escalade `AskUserQuestion` poka, sans déplacer.

### Code

- Pas d'em-dash `, `, anglais comments, conventional commits subject-only,
  pas de Co-Authored-By Claude (cf. règles globales warren).

### Git

- Push direct main warren-app. Pas de branche.
- Aucun commit warren-core sauf si bug découvert au compile (escalade).

### Bench Hetzner

- Tear-down nbg1 obligatoire. WARREN-EXIT-1 PROD HEL1 RESTE UP en fin
  de phase (c'est l'objectif).
- `hcloud server list` final : warren-exit-1 + warren-backend-api seuls
  visibles (mêmes que pre-phase).

---

## 6. Pas de validation intermédiaire poka

Escalade `AskUserQuestion` SEULEMENT si :

1. Rollback warren-exit-1 effectué (verdict NO-GO).
2. Signing key warren-exit-1 doit être touchée pour quelque raison.
3. Allowlist cache corrompue post-redeploy.
4. cargo build warren-exit fail.
5. Coût Hetzner > 0.10 EUR.
6. Variance bench > 20% (zone NO-GO).

Sinon autonomie complète.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- warren-exit-1 prod redeployé depuis warren-core@`b522e3c+`
- Service `active running` post-restart, allowlist re-syncée
- Bench cross-DC daemon-fork ↔ warren-exit-1 :
  - Handshake PASS (vs timeout M4.H.A)
  - TCP 4-flow sustained ≥ 200 Mbps sur 5 min
  - 0 stall ≥ 5s, 0 errors, 0 decode_failures
  - PMTU ≥ 1280 négocié
  - RSS warren-exit-1 stable < +10 MB sur 5 min
- Tear-down nbg1 attesté

### GO CONDITIONAL

- Redeploy + bench PASS mais throughput 150-200 Mbps OU variance 10-20%

### NO-GO HONNÊTE

- Rollback warren-exit-1 nécessaire
- OU throughput < 150 Mbps post-redeploy
- OU régression vs binaire pré-redeploy

---

## 8. Rapport final attendu

`/tmp/m4-h-a-bis-report.md` ≤ 120 lignes. Sections :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO + 1 phrase
2. **Pre-deploy state** warren-exit-1 (version, flags, allowlist gen)
3. **Cross-compile** : SHA warren-core, binaire size, version output
4. **Deploy procedure** : timing stop → start, journalctl excerpt
5. **Smoke handshake** : PASS ou détails fail
6. **Bench results** : tableau throughput / PMTU / errors / RSS
7. **Caveats** résiduels
8. **Coût Hetzner** + tear-down attesté nbg1
9. **Commits poussés** main warren-app
10. **Memory update**

---

## 9. Next steps post-phase (orchestrateur)

- Si GO ULTIMATE : débloque M4.H.B (câblage stack M4.E.D complet) avec
  perf cross-DC validée empiriquement.
- Si CONDITIONAL : pondérer caveats vs M4.H.B scope.
- Si NO-GO : M4.H.A.ter pour investiguer cause root sans rollback hot.

Caveat `account create` factory bug Remote LOCAL=0 (découvert M4.H.A) :
reste open. À traiter M4.H.B ou M4.H.E selon orchestrateur tranche.

---

## 10. Trace de mémorisation

Memory file `warren_m4h_a_bis_delivered.md` à créer :
`/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/`

Inclure : verdict, SHA warren-core deployé, throughput cross-DC réel,
état warren-exit-1 prod post-redeploy, caveats résiduels.

Index MEMORY.md :
`- [M4.H.A.bis delivered](warren_m4h_a_bis_delivered.md), verdict cross-DC post-warren-exit redeploy <SHA> <verdict>`
