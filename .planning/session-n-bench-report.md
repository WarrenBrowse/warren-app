# Session N, Hetzner bench multi-hop consolidé + finding critique DAITA path

> Status : **GO PARTIEL**, baseline DAITA OFF mesuré, DAITA ON révèle bug critique
> Date : 2026-05-21
> Cost réel : **~0.02 EUR** (3 ccx13 nodes ~30 min, cap brief 0.10 ✓)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. §0.6 worktree skipped (warren-core working dir clean, no parallel work).

---

## TL;DR

Bench cross-DC consolidé multi-hop FSN1 → NBG1 → NBG1 livré la pièce manquante par rapport à Session M (qui a abort pré-orchestration). 3 ccx13 Hetzner nodes provisionnés via gold-path `provision-warren-{exit,relay,client}.sh`, binaires cross-compilés rafraîchis (warren-client + warren-exit pre-Session-K.2/M stales remplacés).

**Verdict B.1.8** : `overhead ≤ 15%` ne peut **PAS** être validé empiriquement, DAITA-on multi-hop `--use-tun` path révèle un **bug critique latent** : tunnel "session established" mais **0 paquet IP réel ne traverse**, dans les deux directions, qu'on active DAITA client-side, exit-side, ou les deux. Le bug n'est PAS couvert par les tests Session K/L (loopback mpsc only, structural validation insuffisante).

**Baseline DAITA OFF** : 262 Mbps TCP cross-DC 4 flows 5 min, full multi-hop client→relay→exit. Confirms pipeline structure works at scale ; bug isolé à la 3-task pump model + supervised_pump quand DAITA actif.

---

## Setup déployé

| Node | Hetzner type | Location | Public IP | Role |
|---|---|---|---|---|
| warren-bench-n-client | ccx13 | FSN1-dc14 | 138.199.236.149 | warren-client multi-hop --use-tun |
| warren-bench-n-relay | ccx13 | NBG1-dc3 | 46.225.209.176 | warren-relay |
| warren-bench-n-exit | ccx13 | NBG1-dc3 | 5.75.142.187 | warren-exit --multihop --use-tun |

Cross-DC FSN1↔NBG1, ping RTT mesuré tunnel-side **4.2 ms** (multi-hop chain).

**Binaires** :
- warren-client : cross-compile @ HEAD `8779843` (post Session K.2 wiring) puis patch Session N `8e7c042`
- warren-exit : cross-compile @ HEAD `8779843` (post Session M wiring)
- warren-relay : cross-compile pré-existant inchangé

**Configurations** :
- Client : `warren-client --multi-hop --use-tun [--enable-daita] --relay-info-in ... --exit-info-in ... --operational-pubkey ...`
- Exit : systemd unit avec `--multihop --use-tun --enable-ipv6 [--enable-daita]`
- Relay : turnkey `warren-relay` systemd unit + signed relay.toml
- iperf3 server bound 10.66.0.1:5201 sur exit warren0 (firewall nftables rule ajoutée)

---

## Résultats bench

### Baseline DAITA OFF (5 min, 4 flows TCP, BBR)

```
=== BASELINE DAITA OFF ===
throughput_mbps  : 262
bytes            : 9.82 GB
duration_s       : 300.03
per-stream       : 64 / 78 / 54 / 64 Mbps
retransmits      : 135k, 182k per stream
```

Cohérent avec attentes multi-hop QUIC cross-DC BBR :
- ccx13 = 2 vCPU, ~1 Gbps NIC theoretical
- 1280 MTU (multi-hop default) vs 1500 standard → ~17% MTU loss
- BBR + 4ms RTT + multi-hop encrypt/decrypt path = CPU-bound ~70-80% utilization
- Retransmits high mais cohérents avec BBR aggressive probing cross-DC

### DAITA ON, BLOQUÉ par bug critique

**Symptome reproductible 100%** : tunnel "session established" mais 0 paquet réel ne flow.

| Configuration | session established | ping 10.66.0.1 | iperf3 |
|---|---|---|---|
| client OFF, exit OFF | ✓ | ✓ 4.2ms 0% loss | 262 Mbps |
| client ON, exit ON | ✓ | ✗ 100% loss | timeout |
| client OFF, exit ON | ✓ | ✗ 100% loss | timeout |
| client ON, exit OFF | ✓ | ✗ 100% loss | timeout |

**Diagnostic** :
- Logs client : "multi-hop session established" + "multi-hop DAITA active (... machines=N)", handshake OK
- Logs exit : "multihop DAITA active: serve_multihop_with_tun_and_daita with curated pool pick", wiring Session M honoré
- Logs exit : ZERO warning/error pendant pass DAITA
- tcpdump exit warren0 pendant client ping : **0 paquet reçu** sur 5s
- Le bug bloque dans les **deux** directions (no upstream/downstream)
- Bug est dans le 3-task pump + supervised_pump quand DAITA actif (NOT in DAITA negotiation)

**Hypothèses bug** (à investiguer Session O+) :
1. `pump_multi_bidirectional_with_daita_inner` (3-task model warren-tunnel) : timer task interfere avec downlink task ; les real packets sont mal routés vers maybenot framework au lieu du TUN ?
2. `supervised_pump::run_uplink_with_daita` + `run_downlink_with_daita` : la Notify-driven loop (Session L fix) a un deadlock ou la priorisation DAITA bloque uplink TUN reads
3. Universal dummy filter (Session I.4) : filter trop agressif ? ESPACE byte[0]==0xFF ne devrait jamais match IPv4 (0x4N) / IPv6 (0x6N), mais peut-être appliqué AVANT décryption sur ciphertext (alors filter random 1/256 packets) ou bug filter logic
4. `serve_one_connection_with_tun_and_daita` (Session K.1) : send_datagram pour dummies bloque le datagram queue pour real packets ?

**Tests qui ratent ce bug** :
- `daita_sustained_stress.rs` (Session G) : loopback intra-process, no network
- `e2e_daita_full_bidir.rs` (Session I.5) : intra-process Quinn loopback
- `multihop_tun_with_daita.rs` (Session K.3) : loopback dans le même process, mpsc-based MockPump
- `multi_hop_pump.rs` tests (Session J) : MockPump mpsc

Tous testent la SURVIE du pump et l'ÉMISSION de dummies, mais aucun teste le **THROUGHPUT REAL DE PACKETS IP** end-to-end network.

---

## Patch livré (commit warren-core `8e7c042`)

**`crates/warren-client/src/main.rs::run_multi_hop_with_tun`** : honor `--enable-daita` CLI flag.

Avant Session N :
```rust
let daita_config = {
    use rand_v9::SeedableRng;
    let pool = DaitaPool::default_pool();
    let mut rng = rand_v9::rngs::StdRng::from_os_rng();
    pool.pick(&mut rng)
};
```
DAITA **toujours actif** sur le multi-hop --use-tun path. `--enable-daita` CLI flag = no-op pour cette branche.

Après Session N :
```rust
let daita_config = if args.enable_daita {
    use rand_v9::SeedableRng;
    let pool = DaitaPool::default_pool();
    let mut rng = rand_v9::rngs::StdRng::from_os_rng();
    pool.pick(&mut rng)
} else {
    None
};
```

Aligns multi-hop CLI semantics with single-hop (où `--enable-daita` honored par construction). Permet bench setups avec baseline DAITA-off propre.

Validé : `cargo clippy --release -p warren-client -- -D warnings` PASS.

**Note** : ce patch n'est PAS la cause du bug findng, le bug existait pré-patch (DAITA toujours ON sur multi-hop --use-tun + bug). Le patch a juste rendu possible la mesure baseline DAITA-OFF pour isoler.

---

## Pin warren-app

`.warren-core-version` : `8779843` → `8e7c042`. Effectif dès rebuild warren-app.

---

## Caveats restants post-Session-N

1. **B.1.8 caveat reste OPEN** : overhead measurement empirique impossible jusqu'à fix bug DAITA multi-hop.
2. **CRITICAL BUG** : DAITA-on multi-hop `--use-tun` path produces tunnel mais 0 throughput. **Bloque tout déploiement DAITA en production multi-hop**.
3. **Production warren-exit-1 redeploy** : SAFE en single-hop DAITA (Session I tested intra-process E2E + Session F bench cross-DC 4 cas), mais **NE PAS activer --enable-daita en mode multi-hop** jusqu'au fix bug Session N finding.
4. Universal dummy filter (Session I.4) cross-network behavior pas validé empiriquement (peut être contributing factor).
5. Multi-hop IP negotiation v1 hardcoded mono-client (10.66.0.2/24), non-bench-blocking mais needed for multi-client v1.

---

## Next steps recommandés

**M5.C.X, debug DAITA multi-hop critical path** :
- Repro local : warren-client --multi-hop --use-tun avec real TUN + DAITA, no network (loopback exit on same host) → isolate bug to source code, not network
- Add integration test `multi_hop_real_data_with_daita.rs` qui mesure RX bytes côté exit pendant 1s de ping uplink → catch ce bug avant prod
- Bisect : revert Session L Notify ? revert Session I.4 universal filter ? revert Session K.5 Notify ? identifier le commit qui introduit le bug
- Si le bug est dans le 3-task pump model itself (architecture), considérer rollback vers `pump_multi_bidirectional` non-DAITA pour multi-hop, ajouter DAITA via overlay path (additive padding only, no pump restructure)

**Bench follow-up post-fix** :
- Rerun Session N orchestration avec fix appliqué
- Mesurer overhead réel DAITA on vs off
- Verdict B.1.8 final

---

## Cost récap

- 3 ccx13 nodes × ~30 min @ ~0.012 EUR/h = **~0.02 EUR** ✓ cap 0.10 respecté
- Production warren-exit-1 + warren-backend-api intacts
- Cleanup OK (`hcloud server delete warren-bench-n-{client,relay,exit}`)

---

## Doctrine

- **§0.0 INVIOLABLE** : zero destructive git command. Source patch via Edit tool + cargo commit additif.
- **§0.5 plein mandat** : autonomous bench full orchestration exécuté, abort §0.5 NOT appliqué (vs Session M qui abort pré-orchestration). Source patch additif validé (clippy strict), opportune amélioration découverte mid-bench.
- **§0.6 worktree** : skipped justified, warren-core working dir clean at HEAD `8779843`, no parallel poka work, single-purpose ops bench. WIP poka warren-app + warren-core intacts.
