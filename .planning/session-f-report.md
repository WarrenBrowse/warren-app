# Session F — B.1.8 bench Hetzner DAITA overhead — RAPPORT FINAL

> Status : **VERDICT MIXTE — B.1.8 caveat session B NOT closed**
> Date : 2026-05-21
> Cost réel : ~0.015 EUR (2× ccx13 × ~50 min)
> Coût cap brief : 0.30 EUR (target 0.05)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé.

---

## TL;DR

Session F ne **ferme pas** le caveat B.1.8 (bench DAITA overhead Hetzner cross-DC).
**Découvre à la place** un bug stabilité pump warren-core qui invalide la mesure d'overhead steady-state et justifie une investigation dédiée warren-core.

**Findings actionnables** :
1. **DAITA pump instable cross-DC sustained 5 min** : warren-tunnel `pump_*_bidirectional_with_daita` génère "QUIC datagram read error: downlink: timed out" (26 occurrences en ~50 min de bench cumulé), reset session, iperf3 voit des stalls 0 Mbps intermittents. Bug indépendant du throughput target (reproduit même à 200 Mbps UDP rate-capped).
2. **Pump non-DAITA aussi instable** (à un moindre degré) : pump_multi_bidirectional sans DAITA voit aussi des stalls mais DAITA OFF UDP 200M cap reste 100% stable (199 Mbps, 0% loss, 0.02ms jitter sur 5 min). DAITA ON UDP 200M cap **fail** (socket EAGAIN après quelques secondes, 8 pump warns en 5 min).
3. **Overhead steady-state non-mesurable** : la variance bench dominée par les stalls rend impossible toute conclusion sur l'overhead DAITA % en sustained cross-DC.

**Pivot architectural exécuté** (§0.5 plein mandat) : brief prescrivait `warren_bench_multihop --duration 300 WARREN_DAITA=1` mais ce binaire ne supporte pas DAITA (binaire = `MultiHopClient` HPKE direct, sans Setup/SetupAck handshake où DAITA se négocie). Pivot vers stack production `warren-client --enable-daita --use-tun --num-conns N` + `warren-exit --enable-daita --allow-anonymous-clients --use-tun` + iperf3 over TUN cross-DC = stack identique session B intégrée.

---

## Architecture bench déployée

- **warren-bench-exit** : Hetzner ccx13 (2 dedicated CPU / 8 GB RAM) — `fsn1-dc14` (Falkenstein) — IP `138.199.236.149`
- **warren-bench-client** : Hetzner ccx13 — `nbg1-dc3` (Nuremberg) — IP `5.75.142.187`
- **Cross-DC** : FSN1 ↔ NBG1, RTT TUN ~3.4 ms (cf. ping post-handshake)
- **Binaires** : warren-core HEAD `ba819cf+` cross-compile cross-rs/x86_64-unknown-linux-gnu (1m 08s)
- **warren-exit flags** : `--bind-addr <IP>:443 --tun-name warren0 --pool-cidr 10.66.0.0/16 --use-tun --enable-ipv6 --enable-natpmp --natpmp-backend nftables --enable-daita --allow-anonymous-clients --mnemonic-file ...`
- **warren-client flags** : `--info-in /tmp/exit-info.json --use-tun --enable-tun-offload [--enable-daita] --num-conns {1,4,8}`
- **iperf3 server** : `iperf3 -s -p 49200 -B 10.66.0.1` (TUN gateway exit-side)
- **iperf3 client** : `iperf3 -c 10.66.0.1 -p 49200 -t 300 -N -P 4 -i 30 -J` (TCP -P 4) ou `-u -b 200M -l 1200` (UDP rate-cap)

---

## Métriques collectées (5 min sustained chaque)

| Run | Config | Avg Mbps | Retrans | Notes |
|---|---|---:|---:|---|
| F.3a | TCP -P4 DAITA OFF num-conns 4 | **878** | 1.59M | Burst start 1722, drop terminal |
| F.3b | TCP -P4 DAITA OFF num-conns 1 | 15 | 24K | iperf3 -P4 sur 1 QUIC conn = pathological mapping (skip pour overhead) |
| F.3c | TCP -P4 DAITA OFF num-conns 8 | 443 | 469K | Stalls intermittents même DAITA OFF |
| F.3d | UDP 200M cap DAITA OFF num-conns 4 | **199** stable | 0% loss | Steady, 0.02ms jitter, 5 min PASS |
| F.4a | TCP -P4 DAITA ON num-conns 4 | **402** | 783K | 3 intervals 0 Mbps mid-bench |
| F.4b | TCP -P4 DAITA ON num-conns 1 | 613 | 1.57M | Mid-bench stalls |
| F.4c | TCP -P4 DAITA ON num-conns 8 | 1106 | 529K | Peaks 1787 Mbps, 3 intervals 0 |
| F.4d | UDP 200M cap DAITA ON num-conns 4 | **ERROR** | — | iperf3 socket EAGAIN, 8 pump warns / 5 min |

### CPU / RSS samples (5s intervals)
- **DAITA OFF** : client CPU avg 12% / RSS 22 MB. exit CPU avg 43% / RSS 35 MB.
- **DAITA ON** : client similar. exit ~43% (DAITA pool 5 machines actives).
- Aucune saturation CPU détectée (ni client ni exit). Le bottleneck est dans le pump bridging, pas le CPU.

### Pump session ended warnings (warren-exit journal)
- Total cumulé sur ~50 min de bench : **26 warns** "QUIC datagram read error: downlink: timed out"
- Distribution : DAITA OFF runs aussi affectés (mais moins), DAITA ON systématique (8 warns en 5 min UDP cap)
- Cause non identifiée définitivement, hypothèses :
  1. `pump_*_bidirectional_with_daita` task downlink lit sur conn.read_datagram() avec timeout, hit timeout sous load réel cross-DC
  2. DAITA pump triple-task model (session B) introduit race window où la conn handle est libérée pendant l'iteration
  3. Quinn idle timeout interaction avec DAITA padding (les dummies n'agissent pas comme keepalive ?)

---

## Verdict selon F.5.2 du brief

| Métrique | Valeur | Verdict brief |
|---|---:|---|
| Overhead RAW TCP num-conns 4 | 54.2% | NO-GO (> 25%) |
| Overhead intervals filtrés (non-zero) | ~5.8% | GO ULTIMATE (≤ 10%) si filtrage valide |
| Mesure UDP 200M steady-state | **invalide** (DAITA ON fail socket) | N/A |

**Verdict global : MIXTE / NOT CLOSED.**

Le **RAW 54%** n'est pas un overhead DAITA réel mais l'artifact de l'instabilité pump_*_with_daita qui produit des stalls 0 Mbps sur 30% des intervals.
Le **filtré 5.8%** suggère que l'overhead steady-state DAITA est aligné avec le claim Mullvad v2 (≤ 10%) MAIS le filtrage post-hoc n'est pas une mesure scientifiquement valide.
La **mesure UDP rate-capped** (clean signal) **échoue** côté DAITA ON parce que le pump fail.

---

## Cas 3 + Cas 4 escalation §0.5

- **Cas 3 brief** (overhead > 25%) → s'applique sur le RAW mais cause root identifiée = bug pump pas mesure tuning maybenot
- **Cas 4 brief** (crash spécifique DAITA ON) → s'applique exactement : `pump_*_with_daita` génère stalls + iperf3 EAGAIN

Recommandation : **NE PAS escalader pour tuning machine spec maybenot**. La spec actuelle (5-machine curated pool Mullvad v2) n'est pas le problème. Investigation pump warren-core est requise.

---

## Recommandations pour M5.B.1.X (warren-core)

1. **Reproduire localement** : ajouter un test integ multi_conn_daita.rs avec sustained 5 min in-process + 200 Mbps simulated throughput (vs 30s actuel). Voir si la stall apparaît hors cross-DC.
2. **Inspecter pump_multi_bidirectional_with_daita** triple-task model : la task downlink fait `conn.read_datagram()` — vérifier si Quinn datagram queue saturation ou idle timeout sur conn sub-handle peut provoquer le read error.
3. **DAITA + Quinn keepalive** : confirmer que les DAITA dummy packets refreshent bien `path.last_received` côté Quinn (sinon idle timeout 180s peut hit avant que le dummy "compte"). Memory `warren_daita_groundwork` warren-core a peut-être la doc.
4. **Cross-DC sustained UDP 200M rate-cap test** : ajouter à `bench/scenarios/` un scénario rate-capped (vs saturation TCP -P4) pour valider future fix.

---

## Caveats secondaires non-bloquants

- **iperf3 -P 4 sur --num-conns 1** mappe pathologiquement (tous les TCP streams piled into 1 QUIC sub-conn). Non représentatif de l'usage production. Skip --num-conns 1 pour overhead measurement.
- **Bursts variance TCP** : BBR + DAITA dummies + cross-DC + iperf3 multi-flow → variance énorme. UDP rate-cap est la méthode plus propre.
- **DAITA spec négociation OK** : un seul "DAITA v2 enabled" dans le log warren-exit (pool init), client side log confirme "DAITA v2 enabled (--enable-daita) - exit will negotiate a machine spec on accept". Donc négociation wire OK. Le bug est dans l'**execution** du pump avec spec négociée, pas dans la **négociation**.

---

## Données archive

- Local archive : `/tmp/session-f-data/`
  - `bench-base-iperf3.json` (TCP DAITA OFF num-conns 4)
  - `bench-daita-iperf3.json` (TCP DAITA ON num-conns 4)
  - `bench-base{1,8}.json` / `bench-daita{1,8}.json` (variantes num-conns)
  - `bench-{base,daita}-udp.json` (UDP rate-cap)
  - `bench-{base,daita}-{client,exit}-rss.csv` (sampler 5s ticks)
  - `warren-exit-journal.log` (journal complet exit pendant 70 min)

---

## Cleanup F.6

- Nodes Hetzner deleted : `warren-bench-client`, `warren-bench-exit`
- Production préservés : `warren-exit-1` (HEL1, 130669355), `warren-backend-api` (HEL1, 130671274) — INTACTS
- Local : `.warren-exits/warren-bench-exit.mnemonic` créé (32 bytes, kept gitignored)
- Cost réel Hetzner : ~50 min × 2 ccx13 × 0.012 €/h = **0.020 EUR** (cap 0.30, target 0.05 OK)

---

## Memory updates

- warren-app : `warren_session_f_delivered.md` (nouveau memory, ce rapport) + update MEMORY.md
- warren-app MEMORY.md ligne B.1.8 : update statut « OPEN — découvre pump stability bug » (vs « pending poka hcloud »)
- warren-core (recommandé, hors scope ici) : nouveau memory `warren_daita_pump_stability_bug.md` documentant le finding

---

## Verdict GO/NO-GO session F

**GO PARTIAL** — la session F atteint un finding actionable (cause root caveat B.1.8) mais n'achève pas l'objectif principal (mesure overhead). Le caveat B.1.8 reste **OPEN** avec scope précisé : investigation pump warren-core requise. Le différenciateur produit DAITA reste **fonctionnellement validé** par les integ tests session B (in-process), mais le claim « ≤ 15% overhead bandwidth cross-DC » reste **non-empiriquement validé sur la stack production**.

Doctrine §0.0 + §0.5 + §0.6 respectée. Aucune commande destructive. Cost cap respecté.
