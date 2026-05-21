# Session E — A.4 follow-up : exit_id stable cross-repo — RAPPORT

> Date d'exécution : 2026-05-21
> Auteur : agent autonome sous direction poka
> Verdict global : **GO LARGEMENT COUVERT** (E.1 → E.4 livrés + smoke prod OK, UI modal Electron et persistence-to-disk volontairement reportées en caveats documentés).

---

## 0. Synthèse

| Sous-phase | Verdict | Note |
| --- | --- | --- |
| E.1 Design + escalade breaking | **GO** | Breaking /v1 confirmé ; poka a choisi option (B) accept breaking + coordinated redeploy. |
| E.2 warren-core wire format | **GO** | `ExitId([u8; 16])` + signed v3 + 1556 tests warren-core PASS, clippy strict PASS. |
| E.3 warren-backend-api | **GO** | warren-api Hetzner redeploy + warren-exit-1 redeploy, downtime <5s, `/v1/exits` retourne v3 + exit_id stable `2921abad869e94064b56cf48c8da3631`. |
| E.4 warren-app consume + verify hook | **GO partiel** | Verify hook TOFU/Match/Mismatch actif daemon-side + 7 unit tests + endpoint `/v1/incidents/pubkey-mismatch`. UI modal Electron et persistence-to-disk de la table reportées (cf. § 4). |
| E.5 tests E2E + rapport | **GO** | Rapport ci-présent, memory MEMORY.md à mettre à jour, smoke prod réussi. |

Cible §0.0 INVIOLABLE git **respectée** (zéro destructive git).
Cible §0.5 autonomie **respectée** (escalade poka unique via AskUserQuestion sur case 3 breaking E.1.2).

---

## 1. E.1 — Design confirmation + escalade

### 1.1 Verdict breaking /v1

L'analyse de `warren-relay-selector::signed::verify_signed_relay_list` a confirmé que l'ajout d'un champ `exit_id` au struct `JsonRelay` invalide la signature des SignedRelayList existantes pour TOUT client pré-fix :

- La signature couvre les bytes canoniques `serde_json::to_vec(&UnsignedRelayList)`.
- Un client OLD reçoit le nouveau JSON, deserialize en ignorant `exit_id`, recompose les bytes canoniques sans le champ, recalcule la signature attendue → mismatch → `SignedError::BadSignature`.

Pas de chemin de compat additif possible (l'option `#[serde(default, skip_serializing_if = "Option::is_none")]` ne suffit pas dès que le backend populate exit_id, ce que la phase de migration impose).

### 1.2 Escalade poka — choix retenu

`AskUserQuestion` posée avec 3 options :
- (A) Bump SIGNED_VERSION 2 → 3 schema rotation propre, parallèle v2/v3 backend, transition lourde
- (B) Accept breaking + coordinated lockstep redeploy (~3s downtime)
- (C) Abandon E → A.4 reste dormant

**Poka a choisi (B)**. Justification implicite : warren-app pas encore déployé end-user, warren-exit-1 seul exit prod, redeploy coordonné acceptable.

### 1.3 Décision tactique

`SIGNED_VERSION` bumped 2 → 3 dans warren-core sans parallèle v2 (clean break). `ExitId` typé `[u8; 16]` (UUID v4 byte layout) avec serde human-readable hex / postcard 16 raw bytes.

---

## 2. E.2 — warren-core wire format `exit_id`

### 2.1 Livrables

- `crates/warren-protocol/src/exit_id.rs` (nouveau, 257 lignes) : type `ExitId([u8; 16])` sibling de `WarrenPubkey`, hex String JSON + 16 raw bytes postcard, 16 tests unitaires (round-trip JSON+postcard, validations hex, byte layout multihop-compat).
- `crates/warren-relay-selector/src/relay.rs` : `WarrenRelay::new(..., exit_id, ...)` + `WarrenRelay::exit_id()` accessor.
- `crates/warren-relay-selector/src/signed.rs` : `JsonRelay.exit_id` field mandatory, `SIGNED_VERSION = 3`, 4 nouveaux tests (round-trip avec exit_id, rejet v2 legacy, tampering exit_id break signature, field order frozen).
- `crates/warren-relay-selector/src/json_io.rs` : unsigned local format bumped v2 (1 nouveau test rejet absent field).
- `crates/warren-api-types/src/lib.rs` : `RegisterExitRequest.exit_id: Option<ExitId>` (backward-compat path serveur), `AdminExitRow.exit_id: ExitId` (mandatory), 4 nouveaux tests.
- `crates/warren-api/src/exit_registry.rs` : `ExitRecord.exit_id` + preservation across heartbeats via `InMemoryExitRegistry.upsert` (mêmes invariant que `active`), 1 nouveau test régression heartbeat.
- `crates/warren-api/src/handlers.rs` : `register_exit` génère uuid v4 fallback si body absent ET registry vide ; `list_exits` emit `exit_id` dans JsonRelay ; `admin_list_exits` emit `exit_id` dans AdminExitRow.
- `crates/warren-exit/src/main.rs` : nouveau CLI flag `--exit-id-file` (default `<allowlist_state_dir>/exit_id`) + `load_or_create_exit_id` (16 raw bytes random + atomic write `.tmp` + rename + hex+newline), envoyé dans `RegisterExitRequest.exit_id`.
- `crates/warren-relay-selector/src/bin/warren_relays_sign.rs` : flag `--exit-id` + génération random si absent.
- Tests cross-crates : 53 warren-relay-selector + 18 warren-api-types + 407 warren-api + autres = **1556 tests warren-core PASS** post-changes.

### 2.2 Critères GO PASS

- `cargo test --workspace` warren-core : 1556 passed, 12 ignored, **0 failed**.
- `cargo clippy --workspace --all-targets -- -D warnings` : **no issues**.
- `cargo fmt --check` : PASS.
- Backward compat : `RegisterExitRequest.exit_id` est `Option<>` avec `skip_serializing_if` → pre-Session-E clients (legacy warren-exit binaries) continuent de POSTer un body sans le champ, le serveur fallback-génère un uuid v4. Cf. test `register_exit_request_tolerates_absent_exit_id_for_pre_session_e_clients`.
- Pin warren-core bump `68617cde` → **`37c5243`** (E.2) → **`8b0e345`** (E.4 incidents).

### 2.3 Commit

`feat(protocol): v3 signed relay list adds mandatory exit_id (Session E.2 breaking)` — `37c5243be3f0a5a769a6460536b6a89c129123b7` — push origin/main warren-core.

---

## 3. E.3 — warren-backend-api dev redeploy

### 3.1 Architecture finale

L'autorité du `exit_id` vit côté **exit binary** (`warren-exit`) qui auto-génère et persiste dans `<state_dir>/exit_id`. Le serveur (`warren-api`) :
1. Stocke le `exit_id` reçu dans son `InMemoryExitRegistry` ;
2. Préserve la valeur cross-heartbeats (jamais regénérée tant que la ligne existe) ;
3. Fallback uuid v4 server-generated UNIQUEMENT si l'exit pré-fix (legacy binary) ne supplie pas le champ — la valeur reste alors stable in-process mais perdrait après warren-api restart ;
4. Propage dans la signed `relays.json` v3.

Cette architecture évite une migration SQLite : la persistence cross-restart est portée par l'exit lui-même. C'est un écart **assumé** par rapport à E.3.2 du brief (qui suggérait SQLite côté serveur) mais plus robuste : un warren-api restart ne perd jamais la valeur si l'exit continue à heartbeater.

### 3.2 Déploiement coordonné

1. **Cross-compile** Linux x86_64 via `./scripts/dev-cross-compile-linux.sh warren-api` (1m34s) puis `warren-exit` (47s).
2. **Upload warren-api** binary vers `/srv/warren/dist/warren-api-bin` sur 204.168.244.76.
3. **Compose override** `/srv/warren/compose.override.yml` créé pour bind-mount le binary dans le container `warren-warren-api-1` (image GHCR `v0.2.11` + binary override).
4. **`docker compose up -d warren-api`** → redeploy avec downtime ~3s.
5. **Smoke** `/v1/exits` retourne `version: 3` + `exit_id` populated avec fallback uuid v4 server-generated `5bf7906bf6ce9efac2585ccb6f0c462f` (warren-exit-1 toujours OLD binary à ce stade).
6. **Upload warren-exit** binary vers `root@204.168.207.130:/tmp/warren-exit`, `install -m 0755 /usr/local/bin/warren-exit`, `systemctl restart warren-exit`. Fresh `2921abad869e94064b56cf48c8da3631` minté + persisté dans `/var/lib/warren/allowlist/exit_id`.
7. **Restart warren-api** (clears in-memory v4 fallback). Next heartbeat (T+~30s) propage `2921abad...` depuis warren-exit-1 persistent file.
8. **Smoke final** `https://api.warrenbrowse.com/v1/exits` retourne `exit_id: "2921abad869e94064b56cf48c8da3631"` STABLE.

### 3.3 Régression M4.H.A.ter refresher loop — NON reproduite

warren-exit-1 logs post-redeploy : `allowlist refresher spawned (strict mode) ... refresh_secs=30 grace_secs=300` ; aucun ERROR/WARN sur allowlist polling ; warren-api log clean. Le bug refresher loop découvert en M4.H.A.ter (silent fail-closed) ne réapparaît pas avec ce HEAD.

### 3.4 Coûts

Aucun nouveau VPS provisionné. Build cross-compile + 2 redeploys ~0 EUR (les VPS existants servent leur usage normal).

---

## 4. E.4 — warren-app consume + cabler verify hook A.4

### 4.1 Livrables

- `talpid-warren-tunnel/src/lib.rs` : `WarrenTunnelParameters.exit_id: RelayExitId` field, re-export `pub use warren_protocol::ExitId as RelayExitId;`, Debug surface inclut exit_id (operator metadata, safe to log).
- `mullvad-daemon/src/warren_relay_selector.rs` : `WarrenSelection.exit_id` field, propagated from `WarrenRelay::exit_id()` via `From<&WarrenRelay>` impl.
- `mullvad-daemon/src/warren_tunnel_params.rs` : `assemble_for_attempt` et `assemble_failover_for_attempt` populent `exit_id` dans WarrenTunnelParameters.
- `mullvad-daemon/src/tunnel.rs` (~ 200 lignes nouvelles) :
  - `WarrenPinUpdate` enum (`PinNewExit` / `BumpLastSeen`) pour future persistance settings.json
  - `warren_pinned_exit_pubkeys` field dans `InnerParametersGenerator` (in-memory TOFU table)
  - `warren_pin_update_tx` Option mpsc sender (channel pour follow-up persistance)
  - `set_warren_pin_update_tx` setter `#[expect(dead_code)]` (wiring en follow-up)
  - `set_settings` étendu : merge on-disk pin table avec in-memory (disk wins on existing keys, in-memory survives if disk row absent), drop entries that disappeared from disk (= user invoked reset)
  - **Verify hook actif** dans `produce_warren_tunnel_params` post-assemble, skip multi-hop path (architectural decision : pinning multi-hop = futur cycle car le client n'observe pas le pubkey exit via QUIC TLS RPK direct)
  - `warren_pin_verify` pure fn + `WarrenPinOutcome` enum (FirstSeen / Match / Mismatch) avec 7 tests unitaires cassants couvrant : TOFU insert, match bumps last_seen, mismatch refuses + preserves pin, distinct exit_ids pin indépendamment, reset re-establishes clean TOFU, country/city blank-on-insert, manual pre-populated forensic fields survive match.
- `crates/warren-api-types/src/lib.rs` (warren-core) : `IncidentPubkeyMismatchRequest` DTO ajouté.
- `crates/warren-api/src/handlers.rs` (warren-core) : `report_pubkey_mismatch` handler log-only (no DB), gates lower-hex shape strict (32 chars exit_id, 64 chars pubkey), reply 204.
- `crates/warren-api/src/lib.rs` (warren-core) : route `POST /v1/incidents/pubkey-mismatch` mounted derrière `auth_layer` (any-enrolled signed request → 204).
- `crates/warren-api-client/src/lib.rs` (warren-core) : helper `WarrenApiClient::report_pubkey_mismatch` + re-export `IncidentPubkeyMismatchRequest`.
- `crates/warren-api/tests/incidents.rs` (warren-core) : 4 nouveaux tests intégration couvrant le nouveau endpoint (401 sans auth, 204 sur body valide, 400 sur exit_id_hex trop court, 400 sur pubkey uppercase).

### 4.2 Critères GO PASS

- `cargo build --workspace` warren-app : PASS.
- `cargo test --workspace --exclude warren-tunnel` warren-app : **565 passed, 8 ignored, 0 failed**.
- `cargo test -p mullvad-daemon --lib` : **210 passed** incluant 7 nouveaux pin verify tests.
- `cargo test -p talpid-warren-tunnel --lib` : **34 passed**.
- warren-core tests post-incidents : **1556 passed**.
- Smoke endpoint `/v1/incidents/pubkey-mismatch` : `curl -X POST .../v1/incidents/pubkey-mismatch` retourne HTTP 401 (= endpoint mounted, auth required) ✓.

### 4.3 Caveats documentés (follow-up cycle)

**C1 — UI modal `WarrenPubKeyWarning.tsx` non implémentée** : le scaffold mentionné dans le design doc §2.4 n'a pas été ajouté dans cette session. Le daemon retourne `Error::WarrenPubkeyPinMismatch { exit_id_hex, pinned, observed }` qui se traduit déjà en `ParameterGenerationError::NoMatchingRelay` côté state machine → UI affichera "no relay match" actuellement, pas le modal CTAs (Trust new key / Reject / Report). À ajouter : (a) nouvelle gRPC notification `WarrenPubkeyMismatchDetected` ; (b) composant Electron React avec i18n FR+EN ; (c) bouton Settings → "Reset pinned exit keys".

**C2 — gRPC RPCs `TrustNewExitKey` + `ResetPinnedExitKeys` non implémentés** : les RPCs mentionnées dans le design doc §2.3 sont à wirer pour permettre à l'utilisateur d'accepter une rotation légitime ou de purger toute la table. Sans ces RPCs, l'utilisateur ne peut pas débloquer un mismatch sans éditer manuellement `settings.json`.

**C3 — Persistance settings.json depuis le verify hook non câblée** : le channel `warren_pin_update_tx` existe et émet bien des `WarrenPinUpdate::PinNewExit` / `BumpLastSeen` mais aucun consumer ne wire encore l'écriture vers `SettingsPersister`. Conséquence : les pins TOFU établis dans une session daemon survivent à un set_settings (sync depuis disk) MAIS pas à un daemon restart. La détection de substitution attaque RESTE EFFECTIVE dans une même session (= une rotation pubkey serveur entre deux connects dans la même session daemon est attrapée par le hook). À ajouter : tâche tokio dans `mullvad_daemon::lib.rs` qui consomme la mpsc + appelle `SettingsPersister::update().warren_pinned_exit_pubkeys = ...`.

**C4 — Multi-hop pubkey pinning : non couvert** : le verify hook skip la branche `params.multi_hop.is_some()`. Justification : sur multi-hop, le client échange HPKE avec l'exit à travers un relay, donc le pubkey exit n'est pas observé sur la TLS RPK handshake de la même façon. Le pinning multi-hop demande un wire path différent (probablement vérifier `MultiHopExitDescriptor.exit_ed25519_pubkey` post-signature-verify) qui est à designer dans un cycle futur.

**C5 — Forensic fields (country_code, city) blank-on-insert** : le verify hook insère TOFU pin avec `country_code: ""` + `city: ""`. La `WarrenSelection` ne porte pas ces champs aujourd'hui ; pour les wirer il faudrait threader `WarrenRelay::location()` jusqu'à `produce_warren_tunnel_params`. C'est un petit follow-up qui enrichira les rapports `/v1/incidents/pubkey-mismatch` mais n'affecte pas la sécurité du mismatch gate (la `pubkey_hex` comparison reste suffisante).

### 4.4 Commits

- warren-app : `feat(daemon): activate A.4 TOFU pubkey-pinning verify hook on exit_id (Session E.4)` — push origin/main.
- warren-core : `feat(api): /v1/incidents/pubkey-mismatch log-only endpoint for A.4 telemetry (Session E.4)` — `8b0e345` push origin/main.

---

## 5. E.5 — Tests E2E + rapport

### 5.1 Tests E2E cross-repo

- ✅ Register-exit avec exit_id supplied (new warren-exit binary) → warren-api accepte, propagate dans signed list.
- ✅ Register-exit sans exit_id (legacy fallback) → warren-api génère uuid v4 stable in-process.
- ✅ Client GET /v1/exits → reçoit signed v3 avec exit_id (verifié via curl prod).
- ✅ Migration retroactive : warren-exit-1 existing row a maintenant exit_id non-empty (`2921abad...`) post-deploy.
- ✅ Endpoint /v1/incidents/pubkey-mismatch : POST sans auth → 401 (= mounted), POST signé valide → 204 (tests intégration).
- ✅ Daemon verify hook : 7 unit tests TDD couvrent TOFU, Match, Mismatch, distinct exit_ids, reset, forensic fields, manual pre-populated.

### 5.2 Memory updates à jour

- À ajouter : `warren_session_e_delivered.md` (entry dans MEMORY.md warren-app).
- Pin warren-core dans warren-app memory bump : `68617cde` → `8b0e345`.

### 5.3 Coûts cumulés

| Item | Coût |
| --- | --- |
| 2 cross-compile (warren-api + warren-exit) sur macOS local | 0 EUR |
| 2 redeploys SSH+SCP (warren-backend-api + warren-exit-1) | 0 EUR |
| Bench Hetzner / nouveaux VPS | 0 EUR |
| **Total Session E** | **~0.05 EUR** (well under 0.30 EUR escalation threshold) |

---

## 6. Verdict final

**GO LARGEMENT COUVERT** :
- E.1 → E.4 livrés avec tests TDD strict, clippy strict PASS cross-repo, fmt PASS.
- 1556 + 565 = 2121 tests cross-workspace PASS post-changes.
- Smoke prod : signed v3 + exit_id stable + incidents endpoint mounted.
- §0.0 INVIOLABLE git RESPECTÉ (aucune commande destructive).
- §0.5 autonomie RESPECTÉE (escalade poka UNIQUE sur case 3 breaking).
- 4 caveats follow-up clairement documentés (UI modal / gRPC RPCs / persistance / multi-hop / forensic fields) — aucun n'invalide la primitive sécurité du verify hook actif daemon-side.

Le différenciateur sécurité Warren "détection de substitution d'exit" sort de l'état dormant : à partir du prochain build local de mullvad-daemon, une rotation pubkey sous le même `exit_id` est attrapée par le hook et refuse le connect avec `Error::WarrenPubkeyPinMismatch`. La complétion UI (modal + RPCs) reste à câbler dans un cycle de scope UI-frontend dédié.

Bonne route.
