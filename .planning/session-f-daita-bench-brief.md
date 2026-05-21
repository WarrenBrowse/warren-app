# Session F — B.1.8 bench Hetzner DAITA overhead

> Brief d'agent autonome warren-core (bench scripts) + warren-app (consume résultat).
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> Session courte ops : provision Hetzner cross-DC, bench, cleanup, rapport.

**Effort estimé** : wall-clock 0.5-1 jour.
**Coût Hetzner** : ~0.05 EUR (2-3 nodes CX31 × ~3-4h × 0.013 €/h).
**Pré-conditions** :
- warren-core `main` HEAD `ba819cf+` (DAITA multi-conn pump intégré session B)
- warren-app `main` HEAD `2c49588b22+` (talpid dispatch session B)
- `hcloud --context warren` configuré poka-side
- SSH key Hetzner `pokash` enregistrée
- `bench/scripts/` patterns warren-core opérationnels (M4.E + sessions B référence)

**Objectif** : mesurer empiriquement overhead bandwidth DAITA ON vs OFF en bench cross-DC réel, valider target ≤ 15% (cible nominale ≤ 10% memory `warren_daita_doctrine_v1`). Caveat unique restant de session B livrée.

Sous-phases (séquentielles autonomes) :

1. **F.1 — Pre-flight checklist + provisioning Hetzner cross-DC FR** (~1-2h)
2. **F.2 — Deploy warren-tunnel client + warren-exit + warren-relay** (~1h)
3. **F.3 — Bench baseline DAITA OFF (5 min sustained)** (~30 min)
4. **F.4 — Bench DAITA ON (5 min sustained)** (~30 min)
5. **F.5 — Analyse overhead + verdict** (~30 min)
6. **F.6 — Cleanup nodes + rapport** (~30 min)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard. Préserver tout fichier modified ou untracked warren-core + warren-app. Si état inattendu : escalader.

Violation = scope error CRITIQUE.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak (admin key warren-backend-api, mnemonic, SSH key contenu)
2. Coût Hetzner cumul > 0.30 EUR (escalader si bench dépasse, soit ~6h × 3 nodes)
3. **Spécifique session F** : si overhead mesuré > 25% (vs target 10-15%), escalader pour décision tuning machine spec maybenot vs garder preset Mullvad v2 vs investigation perf
4. Si refresher loop / contract mismatch silencieux warren-exit ré-apparaît (cf. M4.H.A.ter), diagnostic + escalation
5. Si node Hetzner provisioning échoue 3x consécutivement (compte plein, quota), escalade

Décisions tactiques agent autorisées :
- Type machine Hetzner : CX31 recommandé (~CPU 2-core, RAM 8 GB, suffisant pour Quinn 800 Mbps)
- DC cross-DC pair : FSN1 (Falkenstein DE) + NBG1 (Nuremberg DE) — proche, ~10-20 ms RTT. Ou FSN1 + HEL1 (Helsinki) si plus représentatif marché FR. Recommandation FSN1+NBG1 d'abord (faster bench).
- Réutiliser nodes existants si présents (`hcloud server list --context warren`) vs fresh provision, à l'agent de juger
- Duration bench : 5 min sustained par direction (DAITA OFF / DAITA ON), pas plus (coût + cohérence M4.E references)
- Bench tool : `warren_bench_multihop` (warren-core/bench, existing) ou `warren_client_bench` (single-hop). Recommandation multi-hop pour exercer pre-HPKE padding M5.B.1.5
- Mesure : Mbps moyen + median + P95 + RSS client/relay/exit + CPU
- Cleanup obligatoire en fin de session

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main 2c49588b22+
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean ba819cf+

# Pre-flight Hetzner (memory feedback_warren_hetzner_bench_ops_gotchas)
export WARREN_SSH_KEY=pokash
export HCLOUD_CONTEXT=warren

hcloud context use warren
hcloud server list                            # inventory existing
```

Si état inattendu : escalade.

Lire memory `feedback_warren_hetzner_bench_ops_gotchas` pour gotchas :
- SSH key name `pokash` exact
- Cleanup `/tmp/warren-*` obligatoire avant relance bench sur node existant (permission denied warren-owned)

---

## 2. Optimisations agent

- Read warren-core `bench/scripts/` patterns + provision scripts en PARALLÈLE
- Provision 3 nodes (client + relay + exit) en parallèle (`hcloud server create` simultanés)
- Build warren-core binaires une seule fois localement (cross-compile target x86_64-unknown-linux-gnu), scp aux 3 nodes en parallèle
- Pas re-build après chaque iteration
- Cleanup async en arrière-plan pendant rédaction rapport

---

## F.1 — Pre-flight checklist + provisioning Hetzner cross-DC FR (~1-2h)

### Scope F.1

1. **F.1.1** Pre-flight :
   - `export WARREN_SSH_KEY=pokash`
   - `hcloud context use warren`
   - `hcloud server list` : noter ce qui existe (warren-exit-1 + warren-backend-api doivent rester intacts cf. memory `warren_backend_server`)
   - Décider : réutiliser nodes existants si dispo + idle, ou provision fresh
2. **F.1.2** Si provision fresh : 3 nodes
   - `warren-bench-client` : DC FSN1, CX31, image debian-12, SSH key pokash
   - `warren-bench-relay` : DC NBG1, CX31, image debian-12, SSH key pokash
   - `warren-bench-exit` : DC NBG1, CX31, image debian-12, SSH key pokash
3. **F.1.3** Wait ready + collect IPs (`hcloud server describe` chaque)
4. **F.1.4** Cleanup pre-flight via SSH : `ssh root@<ip> "rm -f /tmp/warren-*.log /tmp/warren-*.json"` sur chaque node
5. **F.1.5** SSH connectivity check : `ssh root@<ip> "uname -a"` sur les 3

### Critères GO F.1

- 3 nodes opérationnels (client + relay + exit)
- IPs collectées
- SSH accessible
- Cleanup /tmp pré-bench effectué
- warren-exit-1 + warren-backend-api production PRÉSERVÉS (NE PAS TOUCHER)

### Décisions tactiques F.1

- DC pair FSN1↔NBG1 (proches, ~10ms RTT) par défaut
- Si quota Hetzner atteint : escalade (cas 5)
- Si nodes existants idle (mais pas warren-exit-1 / warren-backend-api) : réutiliser, économise temps

---

## F.2 — Deploy warren-tunnel client + warren-exit + warren-relay (~1h)

### Scope F.2

1. **F.2.1** Build local cross-compile :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-core
   cargo build --release --target x86_64-unknown-linux-gnu \
     -p warren-client --bin warren_bench_multihop \
     -p warren-exit \
     -p warren-relay
   ```
2. **F.2.2** SCP binaires aux 3 nodes en parallèle :
   ```bash
   for (node, binary) in [(client, warren_bench_multihop), (relay, warren-relay), (exit, warren-exit)]:
     scp target/x86_64-unknown-linux-gnu/release/<binary> root@<node_ip>:/usr/local/bin/
   ```
3. **F.2.3** Setup chaque node :
   - exit : génère Ed25519 keypair, démarre `warren-exit --multihop --listen 0.0.0.0:7000 --keypair <key>`
   - relay : démarre `warren-relay --listen 0.0.0.0:7100 --exits-yaml <exit_pubkey + ip:port>`
   - client : prépare config `warren-bench-config.yaml` avec relay IP + exit pubkey
4. **F.2.4** Smoke check : ping HPKE handshake client→relay→exit, 1 message echo (10s)

### Critères GO F.2

- Binaires deployed
- Exit + relay running
- Smoke handshake PASS

### Décisions tactiques F.2

- TLS : RPK Ed25519 entre client↔relay↔exit (config Warren standard)
- Mode : multi-hop (exerce DAITA pre-HPKE padding M5.B.1.5)
- Si binaire warren-bench-multihop pas dispo : compile l'équivalent client warren_client::bench module (existait M4.E.cont)

---

## F.3 — Bench baseline DAITA OFF (5 min sustained) (~30 min)

### Scope F.3

1. **F.3.1** Côté client, lancer bench 5 min DAITA OFF (default), config bandwidth-driven (sender saturé, recv counter Mbps) :
   ```bash
   ssh root@<client_ip> "WARREN_DAITA=0 nohup /usr/local/bin/warren_bench_multihop \
     --relay-ip <relay_ip> --relay-port 7100 \
     --exit-pubkey <exit_pubkey> \
     --duration 300 \
     --datagram-size 1232 \
     > /tmp/warren-bench-daita-off.log 2>&1 &"
   ```
2. **F.3.2** Collecte metrics pendant les 5 min :
   - Mbps moyen + median + P95 + min + max
   - Datagrammes sent / recv / loss ratio
   - RSS client/relay/exit (via `ps -o rss=`)
   - CPU% client (via `ps -o %cpu=` 30s sample)
3. **F.3.3** Sauvegarde résultat brut + parse JSON

### Critères GO F.3

- 5 min sustained sans crash
- Mbps moyen extrait + ratio delivery > 95%
- Reference : M4.E.D bench cross-DC multi-hop = ~409 Mbps. Valider ordre de grandeur similaire (200-500 Mbps acceptable, < 100 Mbps = problem)

### Décisions tactiques F.3

- Datagram size : 1232 bytes (MTU-aware Quinn datagram budget RFC 9221)
- Duration : 5 min (cf. M4.E.D reference)
- Si Mbps < 100 effondré : suspicion bug bench / config, escalade cas 4

---

## F.4 — Bench DAITA ON (5 min sustained) (~30 min)

### Scope F.4

1. **F.4.1** Cleanup `/tmp/warren-*` sur client (préserve warren-owned permission)
2. **F.4.2** Lancer bench DAITA ON, même config sauf `WARREN_DAITA=1` :
   ```bash
   ssh root@<client_ip> "rm -f /tmp/warren-*.log; \
     WARREN_DAITA=1 nohup /usr/local/bin/warren_bench_multihop \
     --relay-ip <relay_ip> --relay-port 7100 \
     --exit-pubkey <exit_pubkey> \
     --duration 300 \
     --datagram-size 1232 \
     > /tmp/warren-bench-daita-on.log 2>&1 &"
   ```
3. **F.4.3** Collecte mêmes metrics que F.3.2

### Critères GO F.4

- 5 min sustained sans crash
- Mbps + RSS + CPU extraits
- Ratio delivery > 95%

### Décisions tactiques F.4

- Si DAITA spec n'arrive pas dans SetupAck (négociation backend cf. session B), agent escalade pour confirmer wire format (cas 4 contract mismatch)
- Si crash spécifique DAITA ON (bug pump_multi_bidirectional_with_daita), escalade pour fix avant retry

---

## F.5 — Analyse overhead + verdict (~30 min)

### Scope F.5

1. **F.5.1** Calcul overhead bandwidth :
   ```
   overhead_pct = (baseline_mbps - daita_on_mbps) / baseline_mbps × 100
   ```
2. **F.5.2** Verdict selon target :
   - ≤ 10% : GO ULTIMATE (nominal target memory doctrine)
   - 10-15% : GO acceptable (target brief session B)
   - 15-20% : GO PARTIAL avec caveat "tuning machine spec recommandé future M5"
   - 20-25% : escalation case 3 (tuning maybenot machine spec)
   - > 25% : NO-GO, escalation immédiate
3. **F.5.3** Compare CPU + RSS overhead DAITA ON vs OFF, documenter
4. **F.5.4** Documenter datagrammes dummies / total (estimation visibility)

### Critères GO F.5

- Overhead calculé + verdict émis
- Rapport intermédiaire `.planning/session-f-bench-data.md` avec metrics raw + verdict

### Décisions tactiques F.5

- Si verdict ≤ 15% : continue F.6 cleanup direct
- Si verdict 15-20% : continue F.6 + note caveat tuning M5
- Si verdict > 20% : escalation poka avant cleanup, garder nodes online pour investigation suite

---

## F.6 — Cleanup nodes + rapport (~30 min)

### Scope F.6

1. **F.6.1** Cleanup binaires + logs des 3 nodes :
   ```bash
   for ip in <client> <relay> <exit>:
     ssh root@$ip "rm -f /usr/local/bin/warren_* /tmp/warren-* /tmp/exit-keypair"
   ```
2. **F.6.2** Delete nodes Hetzner (si provision fresh F.1.2) :
   ```bash
   hcloud server delete warren-bench-client warren-bench-relay warren-bench-exit
   ```
   Si nodes réutilisés (pas fresh) : skip delete, retourner état idle
3. **F.6.3** `hcloud server list` post-cleanup : verifier que warren-exit-1 + warren-backend-api PRÉSERVÉS
4. **F.6.4** Rapport final `.planning/session-f-report.md` :
   - Verdict global (GO ULTIMATE / acceptable / partial)
   - Metrics raw DAITA OFF vs ON (Mbps + RSS + CPU)
   - Overhead %, decision
   - Caveats pour future M5 (si tuning recommandé)
   - Cost Hetzner réel
   - DC pair utilisé
5. **F.6.5** Update memory warren-core : `warren_daita_bench_v1.md` avec metrics + verdict + machine spec confirmée
6. **F.6.6** Update memory warren-app : entry `MEMORY.md` mention B.1.8 closed
7. **F.6.7** Commit + push warren-core report + memory (si applicable)

### Critères GO F.6

- Nodes cleaned (delete si fresh, idle si reused)
- warren-exit-1 + warren-backend-api production PRÉSERVÉS
- Rapport rédigé
- Memory updated

### Décisions tactiques F.6

- Cleanup obligatoire même si verdict NO-GO (sauf escalation explicite pour investigation)
- `hcloud server delete` confirme via prompt → utiliser `--force` ou wrapper non-interactif

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-core
- `bench/scripts/m3e-multi-conn-sweep.sh` (pattern bench multi-conn cross-DC)
- `bench/scripts/m3d-warren-tunnel.sh` (pattern multi-hop bench)
- `bench/scripts/local-multi-hop-pump.sh` (pattern multi-hop local)
- `crates/warren-client/src/bench.rs` (lib bench module ref M4.E.cont)
- `crates/warren-client/src/bin/warren_bench_multihop.rs` (binaire bench M4.E)
- `crates/warren-tunnel/src/pump.rs` (pump_multi_bidirectional_with_daita, DAITA wire)
- Memory warren-core : `feedback_warren_hetzner_bench_ops_gotchas`, `warren_m4e_delivered`, `warren_daita_doctrine_v1`

### warren-app
- `.planning/session-b-daita-multiexit-onboarding-brief.md` §B.1.8
- Memory warren-app : `warren_session_b_delivered.md` (caveat B.1.8 documenté)

---

## 4. Plan d'exécution (séquentiel, autonome)

```
F.1 Pre-flight + provisioning (1-2h)
F.2 Deploy binaires + smoke (1h)
F.3 Bench DAITA OFF 5 min (30 min)
F.4 Bench DAITA ON 5 min (30 min)
F.5 Analyse + verdict (30 min)
F.6 Cleanup + rapport + memory (30 min)
```

Total ~4-5h wall-clock.

---

## 5. Critères GO ULTIMATE session F

- ✅ F.1-F.6 critères GO PASS
- ✅ Overhead DAITA ≤ 15% (target)
- ✅ warren-exit-1 + warren-backend-api production PRÉSERVÉS (cleanup confiné aux nodes bench)
- ✅ Pas de régression pump_multi_bidirectional_with_daita (5 min sustained sans crash)
- ✅ Rapport `.planning/session-f-report.md` rédigé
- ✅ Memory `warren_daita_bench_v1.md` warren-core créée
- ✅ Update MEMORY.md warren-app entry B.1.8 closed
- ✅ Cost Hetzner ≤ 0.30 EUR (target ~0.05)

Verdict GO ACCEPTABLE si overhead 15-20% (caveat tuning M5 future).
Verdict GO PARTIAL si overhead 20-25% (caveat investigation).
Escalation NO-GO si overhead > 25%.

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- `hcloud --context warren` exclusif (jamais poka-perso)
- `WARREN_SSH_KEY=pokash` obligatoire
- Cleanup /tmp/warren-* obligatoire avant relance sur node existant
- Cleanup nodes Hetzner OBLIGATOIRE en fin session (sauf escalation explicite)
- warren-exit-1 + warren-backend-api production PRÉSERVÉS (NE PAS TOUCHER)
- Pas em-dash
- Pas secrets in commits

---

## 7. Memory updates attendus

À ajouter dans warren-core memory :
- `warren_daita_bench_v1.md` — metrics DAITA OFF baseline + DAITA ON + overhead % + DC pair + machine spec preset Mullvad v2 + verdict
- Update index MEMORY.md

À ajouter dans warren-app memory :
- Update MEMORY.md entry `warren_session_b_delivered.md` mention B.1.8 closed (status update inline) ou nouveau memory `warren_session_f_delivered.md` avec ref croisée

---

## 8. Commencer maintenant

Lis le brief, sources §3 en parallèle (memory bench gotchas + pump_multi_bidirectional_with_daita + bench scripts patterns), attaque F.1.1. Plein mandat §0.5.

Session courte mais valeur élevée : ferme le dernier caveat session B + valide empiriquement le différenciateur DAITA produit warrenbrowse.com publié. Sans cette bench, le pitch DAITA reste théorique.

Cost ≤ 0.30 EUR cap, target ~0.05 EUR.

Bonne route.
