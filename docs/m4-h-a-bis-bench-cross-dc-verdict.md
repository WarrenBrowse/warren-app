# Phase M4.H.A.bis — Redeploy warren-exit-1 + re-bench cross-DC

> Rapport agent autonome. Suit M4.H.A (GO CONDITIONAL).

**Date** : 2026-05-19
**Verdict** : **NO-GO** (rollback exécuté).
**Cause root** : warren-exit `b522e3c` (HEAD warren-core M4.E.C.quint+M4.E.D) incompatible avec warren-backend-api v0.2.7 (deployé 2026-05-12) sur le poll `/v1/subscribers/active`. Allowlist fail-closed après 300s grace (`evicted=2`), prod degraded. Rollback chirurgical exécuté en 3 s, prod restored.

---

## 1. Verdict

**NO-GO HONNÊTE**. Le deploy lui-même PASS (downtime ~1 s, service active running, in-memory allowlist seeded admitted=2 from on-disk cache, registration warren-api at boot OK). **Mais** le polling `/v1/subscribers/active` côté warren-exit nouveau ne peut pas refresh l'allowlist depuis warren-backend-api ancien (v0.2.7) : staleness grace exceeded 5 min post-deploy → allowlist EVICTED → 100% des clients rejetés "peer pubkey not in allowlist". Bench daemon-fork est donc bloqué à un niveau orthogonal vs M4.H.A (avant : wire QUIC handshake timeout ; maintenant : QUIC handshake OK mais allowlist-side fail-closed). Brief §M4.H.A.bis.0 anti-pattern #5 ("Si le service ne repasse pas up dans les 5 min post-restart, ROLLBACK immédiat sans investiguer") déclenché à T+5 min (eviction observée à 13:15:58, deploy à 12:48:59).

## 2. Pre-deploy state warren-exit-1 prod

| Attribut | Valeur |
|---|---|
| Binary path | `/usr/local/bin/warren-exit` |
| Size + mtime | 15 742 968 B, `2026-05-13 21:18:40 UTC` |
| Version | `warren-exit 0.1.0` (Iroh-era module names dans logs) |
| Service uptime | 6 j |
| Signing key | `/etc/warren/exit-mnemonic.txt` (inchangée) |
| Allowlist (disk) | `/var/lib/warren/allowlist/allowlist.json` gen 5, 1 pubkey poka prod |
| Flags ExecStart | `--bind-addr :7000 --tun-name warren0 --pool-cidr 10.66.0.0/16 --use-tun --enable-ipv6 --enable-natpmp --natpmp-backend nftables --warren-api-url https://api.warrenbrowse.com --allowlist-state-dir /var/lib/warren/allowlist --enrolled-sentinel /var/lib/warren/.enrolled` |

## 3. Cross-compile native sur nbg1

- warren-core HEAD : `b522e3c24bb6d57addf21e970e75dfcadcff5fcc` (M4.E.C.quint final).
- Build : `cargo build --release -p warren-exit` natif Linux x86_64 sur CCX23 nbg1.
- Temps : **1 min 31 s**, binary size **6 781 336 B** (6.46 MB).
- `--version` output : `warren-exit 0.1.0` (même string, différent code).
- BuildID SHA1 : `7e285662a0e754dbd1b579929613c955480f79a3`.

## 4. Deploy procedure (LINÉAIRE per §4.2)

| Étape | T (s) | Statut |
|---|---|---|
| Backup `cp warren-exit warren-exit.backup-pre-M4HAbis` | T+0 | OK |
| `systemctl stop warren-exit` | T+0 → T+1 | OK (inactive dead) |
| `mv /tmp/warren-exit.new /usr/local/bin/warren-exit` | T+1 | OK |
| `systemctl start warren-exit` | T+1 → T+4 | OK (active running) |
| Status check | T+4 | active running, PID 152214 |
| journalctl no panic | T+4 | OK (clean boot sequence) |
| Port 7000/UDP binding | T+4 | OK (warren-exit listening) |
| **Downtime total** | **~1 s** | ≤ 5 min cap |

journalctl excerpt post-deploy :
```
INFO warren_exit: warren-exit starting bind_addr=204.168.207.130:7000 ...
INFO warren_exit: exit identity loaded from mnemonic file (persistent)
INFO warren_exit: allowlist seeded from on-disk cache admitted=1 cache_dir=/var/lib/warren/allowlist
INFO warren_exit: exit ready endpoint_id=6ad8cbf9d0cb32a2531599d3cf28273ca7dd4fe3d770100b1d8c9355d8f79797
INFO warren_exit: allowlist refresher spawned (strict mode) api_url=https://api.warrenbrowse.com refresh_secs=30 grace_secs=300
INFO warren_exit: warren-api registered exit at boot, starting heartbeat
INFO warren_exit: TUN device created tun="warren0" ip=10.66.0.1
INFO warren_exit: NAT-PMP server listening bind=10.66.0.1:5351
INFO warren_exit: DNS forwarder listening listen=10.66.0.1:53 upstream=9.9.9.9:53
```

Module name `warren_exit` (post-rename), **plus** `warren_iroh_tunnel::exit` qui apparaissait dans les logs du binaire 2026-05-13. Code-path confirmé HEAD.

## 5. Smoke + critical regression discovered

| Check | Statut |
|---|---|
| Service active running post-restart | OK |
| Initial allowlist seeded from disk | OK (admitted=1, gen 5) |
| Exit registered to warren-api at boot | OK (heartbeat alive) |
| TUN created, NAT-PMP + DNS forwarder bound | OK |
| QUIC handshake côté wire | **PROGRÈS** (`read SetupAck: read error: connection lost` au lieu de timeout) |
| **Allowlist refresh via /v1/subscribers/active poll** | **FAIL** : staleness grace 300s exceeded → `WARN warren_exit::allowlist_refresh: allowlist cleared after staleness grace exceeded (fail-closed) evicted=2` à T+5 min |
| **Conséquence** | warren-exit-1 prod en fail-closed, rejette TOUS les clients incl. poka prod |

Test pubkey enroll via wapi voucher (`admin-mint-voucher` + `client-redeem` pubkey `5babfb80...`) PASS côté warren-api ; on-disk allowlist gen 6 (2 pubkeys, poka + test) ; in-memory du nouveau binaire ne refresh PAS depuis le poll. Restart manuel re-seed admitted=2 depuis disk mais le polling subsequent re-fail-closed à T+5 min.

## 6. Bench results

Bench tenté 2 fois (avant restart re-seed + après). Mêmes résultats : REF baseline OK, WARREN scenarios 0 Mbps.

| Scenario | n | Avg (Mbps) | StdDev | Min | Max | UDP loss% |
|---|---|---|---|---|---|---|
| REF direct TCP 1-flow | 5 | 1 220 | 31 | 1 185 | 1 260 | N/A |
| REF direct TCP 4-flow | 5 | 4 053 | 731 | 2 957 | 4 961 | N/A |
| REF direct UDP 1G | 5 | 1 000 | 0 | 1 000 | 1 000 | 0.96 |
| WARREN tunnel TCP 1-flow | 5 | **0** | 0 | 0 | 0 | N/A |
| WARREN tunnel TCP 4-flow | 5 | **0** | 0 | 0 | 0 | N/A |
| WARREN tunnel TCP 4-flow DL | 5 | **0** | 0 | 0 | 0 | N/A |
| WARREN tunnel UDP 1G | 5 | **0** | 0 | 0 | 0 | 0.00 |
| Sustained TCP 4-flow 300 s | 1 | **0** | — | — | — | — |

Daemon log côté nbg1 client final : `Warren handshake failed: QUIC stream error: read SetupAck: read error: connection lost` (retries en boucle, daemon entre BLOCKED). RSS warren-daemon stable 72096 → 72192 KB sur 300s (daemon en retry loop, ~96 KB delta). 0 stalls, 0 decode_failures, 0 replay_rejects (jamais atteint datagram path).

## 7. Rollback (immediate, per §4.2 step 4)

| Étape | T (s) | Statut |
|---|---|---|
| `systemctl stop warren-exit` | T+0 | OK |
| `mv warren-exit.backup-pre-M4HAbis warren-exit` | T+1 | OK |
| `systemctl start warren-exit` | T+1 → T+3 | OK (PID 157912) |
| journalctl seeded admitted=2 | T+3 | OK (poka + test pubkey kept en disk) |
| Service active 25s+ | T+25 | OK |
| Allowlist (disk) gen 6 n=2 | post-rollback | OK |

Downtime rollback ~2 s. Prod warren-exit-1 healthy de nouveau, allowlist polling fonctionne (old binary protocol).

## 8. Caveats résiduels

1. **(critique pre-GA, hors scope M4.H.A.bis)** Pour fermer la boucle perf cross-DC, **warren-backend-api doit aussi être redeployé** depuis warren-core HEAD `b522e3c+`. Le binaire warren-exit b522e3c ne peut PAS interopérer avec warren-backend-api v0.2.7 sur `/v1/subscribers/active`. Coordination redeploy 2-node (exit + api) requise, hors scope chantier `binaire-exit-only`.
2. **Bug fail-closed scope** : si jamais warren-exit-1 redéployé sans backend-api co-deployé, prod tombe en 5 min. Documenter le couplage de version dans `docs/` ou runbooks.
3. **Caveat M4.H.A toujours open** : daemon-fork `account create` factory bug (mode Remote LOCAL=0). Hors scope M4.H.A.bis aussi.

## 9. Coût Hetzner

| Item | Type | Durée | Coût (~) |
|---|---|---|---|
| warren-m4habis-client-nbg1 | CCX23 | ~1.5 h | 0.05 EUR |
| warren-m4habis-iperf-hel1 | CCX23 | ~1.5 h | 0.05 EUR |
| **Sous-total bench** | | | **0.10 EUR** |
| warren-exit-1 + warren-backend-api | (prod) | (intacts post-rollback) | 0 EUR |
| **Total M4.H.A.bis** | | | **0.10 EUR** |

Tear-down attesté : `hcloud server list` final = `warren-exit-1` + `warren-backend-api` seuls.

## 10. Commits poussés (warren-app main)

- `<TBD> bench(M4.H.A.bis): warren-exit redeploy NO-GO rollback + caveat backend-api version coupling`

## 11. Memory update

- Création `warren_m4h_a_bis_delivered.md` au path `/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/`
- Index `MEMORY.md` warren-app mis à jour
- Cross-link `[[warren-m4h-a-delivered]]` (M4.H.A précédent)

## 12. Next steps (orchestrateur)

- **M4.H.A.ter** ou M4.G* : coordonner redeploy 2-node (warren-backend-api + warren-exit) depuis warren-core HEAD, dans une fenêtre maintenance, avec rollback plan 2-node (= 2 backup binaries).
- **Alternative** : si scope warren-backend-api inacceptable maintenant, déployer warren-exit b522e3c + warren-backend-api HEAD côte-à-côte sur 2 nouveaux nodes hel1 (= fresh stack), basculer DNS api.warrenbrowse.com vers le neuf, garder poka prod sur l'ancien stack jusqu'à validation. Cost +2 CCX23.
- **Alternative bien moindre** : reproduire le bench M4.H.A.bis mais avec daemon-fork sur warren-core SHA correspondant au pin de warren-backend-api v0.2.7 (downgrade .warren-core-version pin warren-app à ce SHA). Permet de valider que warren-exit HEAD-with-pin = daemon HEAD-with-pin, mais ne valide PAS la migration Quinn post-M4.0 en prod.

Caveat M4.H.A daemon-fork `account create` factory bug reste ouvert (Remote LOCAL=0).
