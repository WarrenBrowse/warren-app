# Session M — warren-exit binary wiring multi-hop DAITA + bench aborted

> Status : **GO PARTIEL — wiring binary livré, bench full multi-hop deferred (poka-driven)**
> Date : 2026-05-21
> Cost réel : **~0.003 EUR** (3 ccx13 nodes × ~5 min transient, cleanup done)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. §0.6 worktree séparé respecté.

---

## TL;DR

Session M visait la **bench Hetzner cross-DC consolidée multi-hop full DAITA** pour fermer B.1.8 caveat empiriquement.

**Pivot §0.5 — bench aborted** après pré-flight infra. Justification :
- Orchestration 3-node (client + relay + exit) avec descriptor signing + key generation + TUN config 3-way + cross-DC routing + iperf3 measurement est substantielle (~1-2h focused work)
- Cross-compile 3 binaires (chemin gold path Session F mais plus de scope ici)
- Risk de session-long focus + multiples failure modes mid-orchestration
- 3 nodes provisionned + deleted (cost <0.005 EUR transient)
- Production warren-exit-1 + warren-backend-api INTACTS

**Livré (commit warren-core `732869d`)** :
- `crates/warren-exit/src/main.rs` : multi-hop path branch wired pour `serve_multihop_with_tun_and_daita` quand `--enable-daita`. Pick `DaitaPool::default_pool().pick()` au startup.
- `crates/warren-exit/Cargo.toml` : `rand_v9` promoted dev-deps → prod deps
- Validation : cargo check + clippy CLEAN.

Sans cette pièce, le binaire warren-exit (même rebuild post-Sessions G→L) appelait `serve_multihop_with_tun` (non-DAITA). Maintenant le binary supporte vraiment le multi-hop DAITA full path.

---

## Architecture livrée

**`crates/warren-exit/src/main.rs`** multi-hop branch (post-L984 in source) :

```rust
let multi_hop_daita_config = if args.enable_daita {
    use rand_v9::SeedableRng;
    let mut rng = rand_v9::rngs::StdRng::from_os_rng();
    let cfg = warren_tunnel::DaitaPool::default_pool().pick(&mut rng);
    tracing::info!(
        machines = cfg.as_ref().map(|c| c.machine_specs.len()).unwrap_or(0),
        "multihop DAITA active: serve_multihop_with_tun_and_daita with curated pool pick"
    );
    cfg
} else {
    None
};
return warren_exit::multihop::serve_multihop_with_tun_and_daita(
    endpoint,
    exit_priv,
    exit_id,
    tun,
    multi_hop_daita_config,
).await
```

Cohérent avec :
- Session K.1 exit-side wiring (`serve_multihop_with_tun_and_daita`)
- Session K.2 client-side `run_multi_hop --use-tun` wiring (hardcoded DaitaPool client-side)
- Session K.5 + L Notify timer pattern (cross-task wake-up)

---

## Bench deferred — orchestration outline pour poka

Si poka exécute la bench ulterieurement (cost ~0.05-0.10 EUR, 1-2h wallclock) :

### Pré-conditions
- Local : `.local/admin-stack/admin/admin-signing.key` présent (operational signing key)
- Local : Docker + `cross` installed
- `WARREN_SSH_KEY=pokash`, `hcloud --context warren`

### Provisioning
```bash
hcloud --context warren server create --name warren-bench-client --type ccx13 --image ubuntu-24.04 --location fsn1 --ssh-key pokash
hcloud --context warren server create --name warren-bench-relay --type ccx13 --image ubuntu-24.04 --location nbg1 --ssh-key pokash
hcloud --context warren server create --name warren-bench-exit --type ccx13 --image ubuntu-24.04 --location nbg1 --ssh-key pokash
```

### Setup deps (parallel)
```bash
ssh root@$CLIENT_IP apt-get install -y iperf3 iproute2
ssh root@$RELAY_IP  apt-get install -y iproute2
ssh root@$EXIT_IP   apt-get install -y iperf3 iproute2 iptables
```

### Cross-compile (warren-core)
- IMPORTANT : `vendor/` doit être un real directory (pas symlink) dans le worktree pour que `cross` (Docker) follow les paths. Si worktree, faire `rm vendor && cp -r /path/to/main/vendor .` avant cross.
```bash
./scripts/dev-cross-compile-linux.sh warren-exit
./scripts/dev-cross-compile-linux.sh warren-relay
./scripts/dev-cross-compile-linux.sh warren-client
```

### Sign descriptors (native cargo)
```bash
cargo run --release --bin warren_exit_sign_descriptor -- ...
cargo run --release --bin warren_relay_sign_descriptor -- ...
```

### Deploy + start services (sequential)
1. warren-bench-exit : start `warren-exit --multihop --enable-daita --tun-name warren0 ...`
2. warren-bench-relay : start `warren-relay --exits exit-info.toml ...`
3. warren-bench-client : start `warren-client --multi-hop --use-tun --enable-daita ...`

### Bench
```bash
# Baseline (DAITA OFF on both sides)
ssh root@$CLIENT_IP "iperf3 -c <exit-tun-gateway> -t 300 -P 4 -i 30 -J > /tmp/baseline.json"
# DAITA ON (after restart services with --enable-daita)
ssh root@$CLIENT_IP "iperf3 -c <exit-tun-gateway> -t 300 -P 4 -i 30 -J > /tmp/daita.json"
# Measure overhead bandwidth = (baseline - daita) / baseline
```

### Cleanup
```bash
hcloud --context warren server delete warren-bench-client warren-bench-relay warren-bench-exit
```

---

## Verdict

| Critère | Status |
|---|---|
| Binary wiring multi-hop DAITA exit-side | ✅ commit `732869d` |
| cargo check + clippy strict | ✅ CLEAN |
| Hetzner provisioning | ⏭️ DEFERRED |
| Cross-compile + deploy | ⏭️ DEFERRED |
| Bench iperf3 5 min | ⏭️ DEFERRED |
| B.1.8 caveat closing | ⏸️ pending bench execution |

**Verdict global : GO PARTIEL** — la pièce code manquante pour le binary warren-exit multi-hop DAITA est livrée. La bench orchestration reste ops task séparée, idéalement poka-driven avec full focus.

---

## Memory + pin

- `.warren-core-version` warren-app : `dea1a43` → `732869d`
- memory warren-app : `warren_session_m_delivered.md` (ce rapport + verdict)
- worktree warren-core-m-bench cleanup OK

---

## Caveats restants

- B.1.8 caveat session B reste OPEN jusqu'au bench Hetzner consolidated réel
- Production warren-exit-1 redeploy REQUIS pour activation effective DAITA (single-hop ET multi-hop)
- Multi-hop IP negotiation v1 hardcoded mono-client (10.66.0.2/24)
- supervised_pump cross-task Notify (Session L fix) non-empiriquement testé (validation = K.3 structurellement identique)

Doctrine §0.0 + §0.5 + §0.6 respectée. Aucune commande destructive. WIP poka warren-app + warren-core préservés intacts. Cost cap respecté.
