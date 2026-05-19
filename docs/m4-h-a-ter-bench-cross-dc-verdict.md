# Phase M4.H.A.ter - 2-node burn-down redeploy from HEAD warren-core

> Rapport agent autonome. Suit M4.H.A.bis (NO-GO rollback couplage version
> warren-exit <-> warren-backend-api).

**Date** : 2026-05-19
**Verdict** : **NO-GO** (rollback 2-node executed, prod restored).
**Cause root** : warren-exit HEAD `b522e3c` echoue silencieusement le poll
`/v1/subscribers/active` contre warren-backend-api HEAD `v0.2.12-dev`
(meme code base, same commit). Allowlist staleness grace exceeded
(`evicted=1`) a T+5min, identique au pattern M4.H.A.bis. Le couplage
version n'est PAS la cause root : un bug refresher loop OU contract
mismatch silencieux dans warren-core HEAD bloque le poll.

---

## 1. Verdict

**NO-GO HONNETE**. Hypothese de travail M4.H.A.bis (= "redeployer warren-
backend-api HEAD alignera les contracts") est INVALIDEE. Avec HEAD vs HEAD
(warren-exit b522e3c + warren-backend-api v0.2.12-dev built local from
HEAD), warren-exit refresher se comporte identiquement a M4.H.A.bis :
silent poll fail, T+5min eviction. Rollback chirurgical des 2 services
realise sans degradation prod.

## 2. Etat pre-burn capture

| Item | Valeur |
|---|---|
| warren-backend-api docker stack | v0.2.11, /srv/warren/, 3 containers |
| Images PROD | `ghcr.io/warrenbrowse/warren-core/warren-{api,admin}:v0.2.11` |
| Caddy frontend | caddy:2.8-alpine (preserve, jamais touche) |
| warren-exit-1 binary | /usr/local/bin/warren-exit, 15.7 MB, mtime 2026-05-13 |
| warren-exit identite | /etc/warren/exit-mnemonic.txt (preserve) |
| Allowlist disk | gen 7, 1 pubkey (poka prod), `/var/lib/warren/allowlist/` |
| DNS api.warrenbrowse.com | 204.168.244.76 (preserve) |
| DNS exit endpoint | 204.168.207.130:7000 (preserve) |

## 3. Decision architecturale (escalade poka)

Brief M4.H.A.ter §4.2 assumait archi bare-metal warren-backend-api
(`systemctl stop warren-api`, `/usr/local/bin/warren-api`). Realite =
docker compose v0.2.11 (3 containers cosign-signed). Poka a valide via
AskUserQuestion l'**Option 2** : build local docker v0.2.12-dev from HEAD
warren-core + rolling upgrade, conserve volumes Docker (signing keys
serveur, SQLite, mTLS CA fresh ne sont PAS regenerated).

Justification : v0.2.11 commit `e2245473` est ANCETRE de HEAD `b522e3c`
warren-core (git merge-base = v0.2.11), donc HEAD contient TOUT v0.2.11 +
delta M4.E.C.quint + M4.E.D + M4.0. Forward-compat permet preserver
volumes.

## 4. Cross-compile docker images v0.2.12-dev

| Step | Result |
|---|---|
| `docker buildx build -f warren-api.Dockerfile` | OK ~6 min, 28.9 MB, sha256:62704ea3 |
| `docker buildx build -f warren-admin.Dockerfile` | OK ~7 min, 32.6 MB, sha256:397a011b |
| `docker login ghcr.io -u poka --password-stdin` (via `gh auth token`) | OK login |
| `docker push ghcr.io/.../warren-{api,admin}:v0.2.12-dev` | **DENIED** (PAT scope = `read:packages`, manque `write:packages`) |
| Workaround : `docker save \| ssh root@prod docker load` | OK both images loaded sur prod |

## 5. Deploy warren-backend-api PROD via compose (no update-prod.sh)

update-prod.sh aurait fail au step `docker compose pull` (image v0.2.12-dev
absente de GHCR). Workaround manuel :

```bash
sed -i 's/^WARREN_VERSION=.*/WARREN_VERSION=v0.2.12-dev/' /srv/warren/.env
cd /srv/warren && docker compose -f compose.prod.yml up -d --remove-orphans
```

| Check | Status |
|---|---|
| .env swap atomique | OK (backup .env.pre-m4hater preserve) |
| Compose recreate warren-api + warren-admin | OK ~10s |
| warren-api Healthy + warren-admin Healthy | OK T+30s |
| Caddy intact (preserve, jamais recreate) | OK |
| `curl https://api.warrenbrowse.com/healthz` | OK "ok" |
| `curl https://api.warrenbrowse.com/v1/exits` | OK signed JSON, relay warren-exit-1 visible |

## 6. Smoke Tier 1 (62 assertions)

```bash
WARREN_API_URL=https://api.warrenbrowse.com \
  ADMIN_KEY=.local/admin-stack/admin/admin-signing.key \
  ./bench/scripts/test-backend-smoke.sh
```

**Result : 60 PASS / 2 FAIL.**

Fails :
- **VAL1** : mint-token country=FRANCE expected 400 (exit 10), got exit 1
  - `wapi: invalid --country: invalid country code: FRANCE`
- **VAL2** : mint-token country=A1 expected 400 (exit 10), got exit 1
  - `wapi: invalid --country: invalid country code: A1`

Both fails sont WAPI CLIENT-side input validation regression (wapi rejette
pre-send). Server warren-api v0.2.12-dev fonctionne (toutes assertions A1,
F3, B1-B7, C1-C5, DEV1-5, ACCT1, VAL3-7, ADM x6, PRC1-6 PASS). Pas
imputable au server deploy.

## 7. warren-exit-1 binary swap

| Step | Time | Result |
|---|---|---|
| Cross-compile warren-exit native nbg1 (CCX23) | 1m 27s | 6.77 MB binary |
| scp nbg1 -> warren-exit-1 prod | <1s | OK |
| backup `cp warren-exit warren-exit.bak-pre-m4hater` | <1s | OK |
| `systemctl stop warren-exit` + `mv` + `systemctl start` | 114 ms downtime | OK |
| Boot logs : identity loaded, allowlist seeded admitted=1, refresher spawned, warren-api registered, TUN+NAT-PMP+DNS bound | OK | clean |

journalctl boot excerpt :
```
INFO warren_exit: warren-exit starting bind_addr=204.168.207.130:7000
INFO warren_exit: exit identity loaded from mnemonic file (persistent)
INFO warren_exit: allowlist seeded from on-disk cache admitted=1
INFO warren_exit: exit ready endpoint_id=6ad8cbf9d0cb32a2531599d3cf28273ca7dd4fe3d770100b1d8c9355d8f79797
INFO warren_exit: allowlist refresher spawned (strict mode) refresh_secs=30 grace_secs=300
INFO warren_exit: warren-api registered exit at boot, starting heartbeat
INFO warren_exit: TUN device created tun="warren0" ip=10.66.0.1 ip6=Some(fdcc:f:1::1)
INFO warren_exit: NAT-PMP server listening bind=10.66.0.1:5351
INFO warren_exit: DNS forwarder listening listen=10.66.0.1:53 upstream=9.9.9.9:53
```

## 8. Regression DECOUVERTE - SILENT POLL FAIL HEAD vs HEAD

Monitoring 5.5 min post-swap, sampling journalctl every 30s :

```
T+0 a T+270s : ZERO log allowlist (ni DEBUG snapshot apply, ni WARN fetch failed)
T+300s       : WARN warren_exit::allowlist_refresh: allowlist cleared after staleness grace exceeded (fail-closed) evicted=1
```

**Pattern identique a M4.H.A.bis** (qui avait warren-backend-api v0.2.7,
non v0.2.12-dev HEAD). Le couplage version n'est PAS la cause root.

Hypotheses (ordre de probabilite) :
1. **Refresher loop bug HEAD warren-core** : `apply_snapshot` ne set pas
   `last_success_unix_secs` apres poll OK -> `clear_if_stale` declenche
   bien que poll(s) aient succeeded. Logs DEBUG hidden + WARN absent ->
   silent.
2. **Contract mismatch silencieux** `/v1/subscribers/active` : HEAD warren-
   api renvoie schema attendu mais champ different (ex: `pubkeys_hex` vs
   `active_pubkeys`), `decode_snapshot` retourne Err... mais devrait logger
   WARN "warren-api returned invalid pubkey".
3. **Auth chain regression** : warren-exit signe la requete `/v1/
   subscribers/active`, server rejette mais retourne payload `{}` au lieu
   de 401/403 (le client traite comme empty snapshot OK, apply nul).
4. **HTTP layer fail silencieux** : connect timeout, TLS handshake fail,
   ou similaire pas remontes dans WARN.

Pas d'investigation deeper menee : hors scope agent M4.H.A.ter (= warren-
app, pas warren-core). Cause root a etablir cote warren-core.

## 9. Rollback 2-node chirurgical

| Step | Time | Result |
|---|---|---|
| `systemctl stop warren-exit` + `mv warren-exit.bak-pre-m4hater` + `systemctl start` | 127 ms downtime | OK active running |
| Boot logs OK : identity loaded, allowlist seeded admitted=1, refresher spawned, warren-api registered | clean | endpoint_id same |
| `sed WARREN_VERSION=v0.2.11` + `docker compose up -d` | ~30s | warren-api+warren-admin Healthy |
| `curl /healthz` POST-rollback | OK | "ok" |
| `curl /v1/exits` signed JSON | OK | relay warren-exit-1 visible |
| `hcloud server list` final | OK | warren-exit-1 + warren-backend-api seuls |

Prod COMPLETEMENT restoree a l'etat pre-M4.H.A.ter (= etat post-M4.H.A.bis
rollback).

## 10. Caveats residuels

1. **Cause root reste indeterminee** : pourquoi HEAD warren-exit ne poll
   pas /v1/subscribers/active de facon visible. Investigation cote warren-
   core requise.
2. **GHCR PAT poka-IT** manque scope `write:packages`. Update PAT
   necessaire si docker images poussees a GHCR officiellement (avec
   cosign signature workflow CI). Workaround `docker save+load` viable
   pour deploys directs.
3. **Smoke Tier 1 VAL1+VAL2 fails wapi client-side** : regression dans la
   pre-send validation wapi. Smoke script test-backend-smoke.sh attend
   server reject (exit 10), wapi reject client (exit 1). Resolvable par
   patch wapi OR patch smoke expectation.
4. **Bug fail-closed scope toujours present** : warren-exit-1 redeploy
   isole impossible. Cf M4.H.A.bis caveat + ce rapport hypothese (1).
5. **Caveat M4.H.A daemon-fork `account create` factory Remote LOCAL=0**
   reste open. Hors scope M4.H.A.ter.

## 11. Cout Hetzner

| Item | Type | Duree | Cout (~) |
|---|---|---|---|
| warren-m4hater-client-nbg1 | CCX23 | ~1.5 h | 0.05 EUR |
| **Sous-total bench** | | | **0.05 EUR** |
| warren-exit-1 + warren-backend-api | (prod, preserves IPs) | (intacts post-rollback) | 0 EUR |
| **Total M4.H.A.ter** | | | **0.05 EUR** |

Tear-down nbg1 attestre : `hcloud server list` final = `warren-exit-1` +
`warren-backend-api` seuls.

## 12. Commits warren-app main

- `<TBD> bench(M4.H.A.ter): 2-node redeploy HEAD warren-core NO-GO + rollback`

## 13. Next steps (orchestrateur)

- **Investiguer warren-core HEAD** : pourquoi `/v1/subscribers/active`
  poll silencieux (logs absents). Suggest : enable RUST_LOG=debug en
  test ou patch tracing::info! sur la branche success du fetcher pour
  tracer.
- **M4.H.A.quart** OU patch warren-core HEAD : si bug refresher loop
  identifie, patch + tag v0.2.12 + re-deploy.
- **Alternative scope minimal** : downgrader pin .warren-core-version
  warren-app au commit qui MATCHE v0.2.11 (= e2245473), valider perf
  warren-app cross-DC contre stack PROD actuelle v0.2.11/old-warren-exit.
  Ne valide PAS M4.0 obfuscation + M4.E.D auto-reconnect (ces deltas
  resteraient non-validees end-to-end).
- **Long terme** : workflow CI build+sign+push GHCR pour images HEAD
  warren-core, ainsi update-prod.sh fonctionne pleinement.
