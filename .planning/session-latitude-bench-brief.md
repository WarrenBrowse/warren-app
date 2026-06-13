# Session Latitude-Bench, Paired bare-metal bench validation (#19)

> Brief d'agent autonome warren-core bench scripts + Latitude.sh.
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session courte ops : caractérisation throughput Warren sur bare-metal vs cloud Hetzner.

**Effort estimé** : wall-clock 0.5-1 jour.
**Coût Latitude.sh** : ~5-10 EUR (2 nodes bare-metal ~6h × ~0.50-1 EUR/h).
**Pré-conditions** :
- warren-core `main` HEAD `fed1c88+` (Latitude.sh provisioning scripts livrés)
- Accès Latitude.sh poka-side (ou éligibilité free-tier)
- SSH key configurable Latitude.sh

**Objectif** : valider la perf Warren multi-hop + DAITA sur bare-metal paired (Latitude.sh) vs cloud Hetzner ccx13. Référence Session AE.5 cross-DC ccx13. Cible : confirmer no noisy neighbor + caractérisation throughput nominal pour pitch produit.

Sous-phases (séquentielles autonomes) :

1. **Lat.1, Setup worktree warren-core** (~30 min)
2. **Lat.2, Provisioning 2 nodes Latitude.sh paired** (~1h)
3. **Lat.3, Deploy warren-tunnel client + warren-exit + warren-relay** (~1h)
4. **Lat.4, Bench baseline + DAITA OFF (5 min sustained iperf3)** (~30 min)
5. **Lat.5, Bench multi-hop + DAITA ON (5 min sustained)** (~30 min)
6. **Lat.6, Comparaison Hetzner ccx13 vs Latitude.sh bare-metal** (~30 min)
7. **Lat.7, Cleanup + rapport + memory** (~30 min)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si secret leak, coût Latitude.sh > 10 EUR (cap), breaking /v1, signing key prod, OU **spécifique Lat** : si Latitude.sh API access manquant (token, account éligibilité), escalade poka.

Décisions tactiques agent autorisées :
- Instance type Latitude.sh : `c2.small.x86` ou équivalent (bare-metal 4-core dédié), région EU (Amsterdam/Frankfurt)
- Pair location : intra-DC (low RTT bench limite max) ou cross-DC (réaliste user) → recommandation cross-DC Frankfurt↔Amsterdam (~10ms RTT)
- Bench tool : `warren_bench_multihop` ou iperf3 cross-tunnel (cf. Session F pattern)
- Duration : 5 min sustained (parité M4.E.D + Session AE.5)
- Si Latitude.sh free-tier inéligible : escalade poka pour fund account ou fallback Hetzner ccx33 (next tier up)

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-latitude-bench main
cd ../warren-core-latitude-bench
```

Cleanup :
```bash
git worktree remove ../warren-core-latitude-bench
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-latitude-bench main
cd ../warren-core-latitude-bench

# Read provisioning scripts (commit fed1c88)
ls bench/scripts/ | grep -i latitude
cat bench/scripts/provision-latitude-warren-*.sh 2>/dev/null || ls bench/scripts/

# Read AE.5 Hetzner bench report (baseline reference)
cat /Users/poka/dev/warrenBros/warren-app/.planning/session-ae5-hetzner-ip-nego-bench-report.md
```

---

## 2. Lat.2, Provisioning 2 nodes Latitude.sh paired (~1h)

### Scope

1. Auth Latitude.sh CLI ou API token (env var `LATITUDE_API_TOKEN`)
2. Provision 2 nodes :
   - `warren-bench-lat-exit` : Frankfurt DE, c2.small.x86 (ou équivalent bare-metal 4-core)
   - `warren-bench-lat-client` : Amsterdam NL, c2.small.x86
3. Wait ready + collect IPs
4. SSH connectivity check
5. Cleanup pre-flight `/tmp/warren-*` (memory feedback_warren_hetzner_bench_ops_gotchas)

### Critères GO

- 2 nodes opérationnels bare-metal
- IPs + SSH ready
- Cleanup pré-bench effectué

### Décisions tactiques

- Si seulement 1 location dispo : single-DC bench (caractérisation throughput max sans RTT penalty)
- Si bare-metal pas dispo : escalade fallback Hetzner ccx33 ou équivalent

---

## 3. Lat.3, Deploy warren-tunnel client + warren-exit + warren-relay (~1h)

### Scope

1. Build local cross-compile x86_64-unknown-linux-gnu warren-client + warren-exit + warren-relay HEAD
2. SCP binaires aux 2 nodes en parallèle
3. Setup :
   - exit : démarre `warren-exit --multihop --listen 0.0.0.0:7000 --enable-daita`
   - client : prépare config + démarre `warren-client --use-tun --enable-daita`
4. Smoke handshake : ping multi-hop client → exit, 1 message echo

### Critères GO

- Binaires deployed
- Exit + client running
- Smoke handshake PASS

### Décisions tactiques

- Si 3 nodes nécessaires (relay séparé) : provision 3e node OR co-locate relay sur exit machine
- Pour simplifier : co-locate (gain 1 node)

---

## 4. Lat.4, Bench baseline + DAITA OFF (5 min sustained) (~30 min)

### Scope

iperf3 cross-tunnel TCP 4-flow 5 min DAITA OFF :
```bash
ssh root@<client_ip> "WARREN_DAITA=0 nohup /usr/local/bin/warren-client --use-tun ... > /tmp/warren-bench.log 2>&1 &"
sleep 10  # wait tunnel up
ssh root@<exit_ip> "iperf3 -s -B 10.66.0.1 -p 5201 &"
ssh root@<client_ip> "iperf3 -c 10.66.0.1 -B 10.66.0.2 -p 5201 -t 300 -P 4 -J > /tmp/iperf3-daita-off.json"
```

### Critères GO

- 5 min sustained
- Mbps moyen extracted from JSON
- Reference : Session AE.5 (Hetzner ccx13 cross-DC) : ~200 Mbps avg, peaks 1106 Mbps DAITA OFF intra-DC

---

## 5. Lat.5, Bench multi-hop + DAITA ON (5 min sustained) (~30 min)

### Scope

Same setup, `WARREN_DAITA=1`. Mesure overhead bandwidth + CPU% + RSS.

### Critères GO

- 5 min sustained sans crash
- Overhead calculé vs Lat.4

---

## 6. Lat.6, Comparaison Hetzner ccx13 vs Latitude.sh bare-metal (~30 min)

### Scope

Table comparative :
| Metric | Hetzner ccx13 cross-DC | Latitude.sh bare-metal cross-DC |
|--------|------------------------|--------------------------------|
| Throughput DAITA OFF Mbps avg | 200 (AE.5) | TBD |
| Throughput DAITA ON Mbps avg | TBD (AE.5 deferred) | TBD |
| RTT TUN ms | 5 | TBD |
| CPU% client peak | TBD | TBD |
| RSS client/exit/relay | TBD | TBD |
| DAITA overhead % | TBD (Session R 5.6% indicative) | TBD |
| Cost/h | ~0.013 €/h | ~0.50-1 €/h |

Verdict : si bare-metal ≥ 2x cloud → pitch produit "Warren full-speed sur bare-metal", recommander deployment exit sur Latitude.sh pour clients premium.

### Critères GO

- Table comparative + verdict
- Decision deploy strategy documentée

---

## 7. Lat.7, Cleanup + rapport + memory (~30 min)

### Scope

1. Cleanup binaires + logs des 2 nodes
2. Delete nodes Latitude.sh
3. Verify warren-exit-1 Hetzner + warren-backend-api Hetzner PRESERVED (non touchés cette session)
4. Rapport `.planning/session-latitude-bench-report.md`
5. Memory `warren_session_latitude_bench_delivered.md` warren-core
6. Update MEMORY.md
7. Cleanup worktree

### Critères GO

- Nodes cleaned (delete confirmed via API)
- Rapport rédigé
- Memory updated
- Cost réel documenté ≤ 10 EUR

---

## 8. Sources cross-repo à lire (PARALLÈLE)

- `bench/scripts/provision-latitude-warren-*.sh` (commit fed1c88)
- `bench/scripts/m3e-multi-conn-sweep.sh` (pattern bench multi-conn cross-DC)
- `.planning/session-ae5-hetzner-ip-nego-bench-report.md` (baseline reference)
- `crates/warren-client/src/bench.rs`
- Memory `feedback_warren_hetzner_bench_ops_gotchas` (gotchas SSH key + /tmp cleanup)

---

## 9. Critères GO ULTIMATE

- ✅ Lat.2-Lat.7 critères GO PASS
- ✅ 2 benchs 5 min sustained
- ✅ Overhead DAITA caractérisé
- ✅ Table comparative Hetzner vs bare-metal
- ✅ Cleanup nodes confirmé
- ✅ Cost ≤ 10 EUR
- ✅ Rapport + memory rédigés

Verdict GO PARTIEL si :
- 1 seule region Latitude.sh dispo (single-DC bench, pas cross-DC)
- Bare-metal indisponible (fallback Hetzner ccx33)

---

## 10. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree
- Cleanup nodes OBLIGATOIRE
- warren-exit-1 + warren-backend-api Hetzner PRESERVED
- English-only code comments
- Pas em-dash

---

## 11. Memory updates

- `warren_session_latitude_bench_delivered.md`
- Update MEMORY.md

---

## 12. Commencer maintenant

Worktree §0.6, sources §8 en parallèle, attaque Lat.2 provisioning. Cost cap 10 EUR.

Bonne route.
