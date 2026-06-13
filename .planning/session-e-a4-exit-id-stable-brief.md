# Session E, A.4 follow-up : exit_id stable cross-repo (débloquer pinning pubkey)

> Brief d'agent autonome cross-repo warren-core + warren-app + warren-backend-api.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> Session courte sécurité : 5 sous-phases enchaînées par un seul agent.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : ~0.10 EUR (1 redeploy warren-backend-api dev + tests E2E).
**Pré-conditions** :
- warren-app `main` HEAD `2c49588b22+`
- warren-core `main` HEAD `ba819cf+`
- warren-backend-api accessible (via warren-core repo, IP 204.168.244.76 cf. memory `warren_backend_server`)

**Objectif** : ajouter un `exit_id` stable cross-repo (warren-core wire format /v1 + warren-backend-api génération + warren-app consume) pour débloquer le verify hook + UI activation A.4 pinning pubkey (livré scaffold session A, bloqué par cette absence).

**Contexte découverte session A.4** : agent a tenté de livrer le pinning pubkey complet mais a découvert que Warren /v1 utilise le `pubkey` comme identifiant unique de l'exit (pas de `exit_id` séparé). Conséquence : pinning by pubkey est tautologique (le pubkey est l'identité, on ne peut pas détecter rotation pubkey). Le scaffold storage + design doc sont prêts (`.planning/a4-pubkey-pinning-design.md`), il manque uniquement le field stable cross-repo.

Sous-phases (séquentielles autonomes) :

1. **E.1, Design confirmation + escalade breaking /v1** (~0.5j)
2. **E.2, warren-core wire format /v1 ajoute `exit_id`** (~1-1.5j)
3. **E.3, warren-backend-api génère + persiste `exit_id`** (~1j)
4. **E.4, warren-app consume + cabler verify hook A.4 + UI activation** (~1-1.5j)
5. **E.5, Tests cross-repo E2E + rapport** (~0.5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard. Préserver tout fichier modified ou untracked. Idem warren-backend-api repo (sous /Users/poka/dev/warrenBros/warren-core, branche main, working tree).

Violation = scope error CRITIQUE.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak (admin signing key warren-backend-api, mnemonic)
2. Coût > 0.30 EUR (escalader si redeploy bench cumul dépasse 0.30 EUR)
3. **CRITIQUE Spécifique session E** : si l'ajout de `exit_id` au /v1 wire format est confirmé BREAKING (clients existants ne peuvent plus parse) → escalade poka AVANT push warren-core. Cf. E.1.
4. Signing key prod warren-backend-api touchée
5. Si refresher loop / contract mismatch silencieux warren-exit (cf. M4.H.A.ter bug refresher loop découvert) ré-apparaît post-deploy → diagnostic + escalation

Décisions tactiques agent autorisées :
- Format `exit_id` (UUID v4 vs ULID vs `<country>-<seq>`), recommandation UUID v4
- Génération côté backend (auto à register-exit) vs configuration explicite (admin set), recommandation auto-gen
- Migration : auto-générer `exit_id` pour les exits déjà register (filler retroactif via UPDATE SQLite) vs forcer re-register chaque exit, recommandation auto-gen migration script
- Exposition dans /v1/exits : tous les fields actuels + `exit_id` (additif non-breaking si client tolère unknown fields JSON)
- TTL pinning warren-app : par défaut illimité (manual reset Settings) vs rotate quarter

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main 2c49588b22+
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean ba819cf+
```

Lire le design doc :
```bash
cat /Users/poka/dev/warrenBros/warren-app/.planning/a4-pubkey-pinning-design.md
```

---

## 2. Optimisations agent

- Read sources cross-repo en PARALLÈLE
- Tests TDD groupés en fin de sous-phase
- Push warren-core + warren-app + warren-backend-api au fil de l'eau
- Pin warren-core bump warren-app obligatoire (modif wire format)

---

## E.1, Design confirmation + escalade breaking /v1 (~0.5j)

### Scope E.1

1. **E.1.1** Vérifier breaking change :
   - Lire `crates/warren-protocol/src/lib.rs` warren-core (wire format /v1)
   - Identifier struct `ExitInfo` / `RelayDescriptor` / `RegisterExitRequest` actuels
   - Tester ajouter `exit_id: Uuid` (ou `Option<Uuid>` pour backward compat) en field optionnel
   - Si `#[serde(default)]` + `Option<>` → non-breaking (clients existants ignorent field absent)
   - Si position imposée structurellement (bincode/postcard length-encoded) → breaking
2. **E.1.2** Décision :
   - SI **non-breaking** (JSON additif avec Option/default) → continuer E.2 directement
   - SI **breaking** → ESCALADE poka via AskUserQuestion : "Ajout `exit_id` /v1 est breaking, propose : (A) bump /v1 → /v2 wire format (lourd), (B) accept breaking change + coordinated redeploy warren-exit-1 prod (downtime ~3s), (C) abandon, garder pinning A.4 dormant"
3. **E.1.3** Documenter décision dans `.planning/session-e-report.md` §E.1

### Critères GO E.1

- Verdict : non-breaking confirmé OU escalade poka effectuée
- Décision documentée

### Décisions tactiques E.1

- Format `exit_id` : `Uuid v4` standard, stable, 128-bit, négligeable size cost
- Si Option<Uuid> dans struct existant : `#[serde(default, skip_serializing_if = "Option::is_none")]`
- /v1 wire format actuellement utilise JSON (cf. canonical_message HTTP signature) majoritairement, bincode pour QUIC datagram payloads (warren-multihop HPKE). Le `exit_id` impacte principalement /v1/exits JSON HTTP API + RegisterExit RPC

---

## E.2, warren-core wire format /v1 ajoute `exit_id` (~1-1.5j)

### Scope E.2

1. **E.2.1** `crates/warren-protocol/src/lib.rs` : ajout `pub exit_id: Uuid` (ou Option) dans :
   - `WarrenExitAddr` / `ExitInfo` (ou équivalent struct exposé /v1)
   - `RegisterExitRequest` (champ optionnel server-genère si absent)
   - `SignedExitDescriptor` (si applicable)
2. **E.2.2** `crates/warren-api-types/src/lib.rs` warren-core : ajout `exit_id` dans DTOs `/v1/exits` response + `/v1/subscribers/active` response + `/v1/register-exit` request/response
3. **E.2.3** `crates/warren-api-client/src/lib.rs` : consume `exit_id` dans response structs
4. **E.2.4** `crates/warren-tunnel/src/exit.rs` : `ExitInfo` carry `exit_id` field, propagate vers `WarrenStatusCache` (cross-repo via warren-app)
5. **E.2.5** `crates/warren-relay-selector/src/relay.rs` : `WarrenRelay` struct carry `exit_id`
6. **E.2.6** Tests warren-core :
   - Serialize/deserialize ExitInfo avec exit_id : round-trip OK
   - Backward compat : deserialize old JSON sans field → default OR Option::None
   - Tests warren-api-client / warren-tunnel mis à jour

### Critères GO E.2

- Ajout `exit_id` cohérent cross-crates warren-core
- `cargo test --workspace` warren-core PASS
- `cargo clippy --workspace --all-targets -- -D warnings` PASS
- Backward compat testé

### Décisions tactiques E.2

- Field obligatoire dans struct interne, optionnel dans wire format JSON (avec default Uuid::nil() ou Option<Uuid>)
- Persister vers SQLite warren-api : ajouter colonne `exit_id TEXT NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'` ou similaire

---

## E.3, warren-backend-api génère + persiste `exit_id` (~1j)

### Scope E.3

1. **E.3.1** `crates/warren-api/src/handlers.rs` warren-core : `register-exit` handler génère `Uuid::new_v4()` si requête sans `exit_id`. Persiste en SQLite `exits` table (nouvelle colonne).
2. **E.3.2** Schema migration SQLite : ajouter colonne `exit_id TEXT NOT NULL DEFAULT ''` (ou similaire), avec script migration `ALTER TABLE exits ADD COLUMN exit_id TEXT NOT NULL DEFAULT '<random-uuid-per-existing-row>'`. Migration auto au boot warren-api si version SQLite < N.
3. **E.3.3** `/v1/exits` GET response : inclut `exit_id` pour chaque exit
4. **E.3.4** `/v1/subscribers/active` GET : exit_id inclus dans entries si applicable (sinon skip, ce endpoint expose pubkeys subscribers, pas exits)
5. **E.3.5** Admin endpoints (`/v1/admin/exits`) : list inclut `exit_id`, GET single par exit_id support
6. **E.3.6** Migration retroactive : script `migrate-exit-ids.sql` auto-run au boot warren-api détecte rows avec `exit_id = ''` (legacy), UPDATE avec `randomblob(16)` ou Rust-side uuid generation
7. **E.3.7** Redeploy warren-backend-api dev (hcloud --context warren) + smoke test `wapi GET /v1/exits` retourne exit_id
8. **E.3.8** Tests warren-api integ : register-exit avec/sans exit_id supplied, response inclut exit_id

### Critères GO E.3

- warren-api code update + migration auto OK
- Redeploy warren-backend-api dev sans drop session (ou downtime <5s)
- Smoke `wapi GET /v1/exits` retourne exit_id populated pour exits existants
- Tests warren-api PASS
- Pas de régression refresher loop (cf. M4.H.A.ter incident bug refresher loop / contract mismatch silencieux)

### Décisions tactiques E.3

- Migration : script Rust au boot warren-api (vs script SQL manual)
- TLS dev warren-backend-api : si pas encore wired (B.1 caveat Mullvad memory roadmap), HTTP plain OK pour smoke
- Hetzner deploy : `hcloud --context warren server list` + redeploy strategy idempotent

---

## E.4, warren-app consume + cabler verify hook A.4 + UI activation (~1-1.5j)

### Scope E.4

1. **E.4.1** Bump pin warren-core `.warren-core-version` warren-app vers HEAD post-E.3
2. **E.4.2** `crates/talpid-warren-tunnel/src/lib.rs` : consume `exit_id` dans `WarrenTunnelParameters` / `WarrenStatusCache`
3. **E.4.3** Activer verify hook A.4 scaffold (déjà présent) :
   - `PinnedKeyStore` schema update : key = `exit_id` (Uuid), value = `(pubkey_ed25519, first_seen_unix, last_seen_unix)`
   - At connect : query stored pubkey for `exit_id`. If exists + mismatch → refuse connect + emit UI event
   - If !exists → store new entry (TOFU)
   - If match → silent OK
4. **E.4.4** Activer UI warning modal A.4 scaffold (déjà présent) : `WarrenPubKeyWarning.tsx` Electron i18n FR+EN, 3 CTAs (Trust new key / Reject / Report)
5. **E.4.5** Settings → "Reset pinned exit keys" CTA fonctionnel
6. **E.4.6** Endpoint `/v1/incidents/pubkey-mismatch` warren-api stub (log only, non-bloquant). Cf. design doc `.planning/a4-pubkey-pinning-design.md` §6.
7. **E.4.7** Tests E2E warren-app :
   - First connect to exit_id X → pubkey pinned, no warning
   - Reconnect to X → match, no warning
   - Connect different exit (different exit_id) → new pubkey pinned (per exit_id distinct)
   - Mismatch same exit_id (different pubkey) → connect refused + event UI
   - Trust new key flow → pinning updated for same exit_id
   - Reset pinned keys → all cleared

### Critères GO E.4

- Pin warren-core bumped
- Verify hook actif + UI warning modal Warren-branded
- Settings reset CTA fonctionnel
- Endpoint /v1/incidents stub
- Tests E2E 6/6 PASS

### Décisions tactiques E.4

- Storage : SQLite warren-tunnel (réutilise infra existante session A.4 scaffold)
- TTL pinning : indéfini par défaut (manual reset Settings)
- Endpoint /v1/incidents : POST simple, pas de DB, log+metric, retour 200

---

## E.5, Tests cross-repo E2E + rapport (~0.5j)

### Scope E.5

1. **E.5.1** Tests E2E cross-repo :
   - Register-exit avec/sans exit_id : génération auto OK
   - Client connect via warren-app : exit_id reçu dans WarrenStatusCache
   - Pinning E2E avec exit_id stable : multi-connect même exit, no warning
   - Smoke prod exit-1 (warren-exit-1 Hetzner) : connect retourne exit_id stable cross-attempts
2. **E.5.2** Migration retroactive validée : warren-exit-1 existing rows ont exit_id non-empty post-deploy
3. **E.5.3** Rapport `.planning/session-e-report.md` :
   - Verdict global GO ULTIMATE / GO partial / NO-GO
   - Caveats par sous-phase
   - Memory updates
4. **E.5.4** Update MEMORY.md warren-app avec entry session E delivered

### Critères GO E.5

- 4+ tests E2E PASS
- Migration retroactive validée prod-exit
- Rapport rédigé
- Memory mise à jour

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-core
- `crates/warren-protocol/src/lib.rs` (wire format /v1)
- `crates/warren-api-types/src/lib.rs` (DTOs HTTP API)
- `crates/warren-api-client/src/lib.rs` (client consumer)
- `crates/warren-api/src/handlers.rs` (backend handler register-exit)
- `crates/warren-api/Cargo.toml` (sqlx/rusqlite deps schema)
- `crates/warren-tunnel/src/exit.rs` (ExitInfo)
- `crates/warren-relay-selector/src/relay.rs` (WarrenRelay)
- Schemas SQLite (migrations folder warren-api)

### warren-app
- `crates/talpid-warren-tunnel/src/lib.rs`
- `.warren-core-version` (pin)
- `.planning/a4-pubkey-pinning-design.md` (design doc complet)
- `desktop/packages/mullvad-vpn/src/renderer/components/WarrenPubKeyLabel.tsx` (UI scaffold)
- `desktop/packages/mullvad-vpn/src/renderer/components/` (chercher `WarrenPubKeyWarning*` ou scaffold A.4)

---

## 4. Plan d'exécution (séquentiel, autonome)

```
E.1 Design confirmation + escalade breaking (0.5j)
  └── Décision continue ou escalade
E.2 warren-core wire format (1-1.5j)
E.3 warren-backend-api (1j) ← inclut redeploy hcloud
E.4 warren-app consume + activate A.4 (1-1.5j)
E.5 Tests E2E + rapport (0.5j)
```

Total ~3-5j wall-clock. Push warren-core + warren-app + warren-backend-api (via warren-core repo) au fil de l'eau.

---

## 5. Critères GO ULTIMATE session E

- ✅ E.1-E.5 critères GO PASS
- ✅ Aucun breaking change non-escaladé (si breaking détecté, escalade obligatoire)
- ✅ `cargo test --workspace` warren-core PASS + clippy strict PASS
- ✅ `cargo test --workspace` warren-app PASS + clippy strict PASS
- ✅ `cargo fmt --check` PASS cross-repo
- ✅ Redeploy warren-backend-api dev sans régression refresher loop (référence M4.H.A.ter bug)
- ✅ Verify hook A.4 actif + UI modal fonctionnelle
- ✅ Tests E2E pinning 6/6 PASS
- ✅ Pas de régression desktop/mobile (si sessions C/D ont touché surfaces overlappant)
- ✅ Migration retroactive validée prod-exit warren-exit-1
- ✅ Rapport `.planning/session-e-report.md` rédigé

Verdict GO PARTIEL acceptable si :
- Redeploy warren-backend-api prod (vs dev) skipped → "GO code, prod deploy pending poka coord"
- Skip explicite documenté

Verdict NO-GO si breaking /v1 ne peut être résolu sans coordination prod warren-exit-1 redeploy.

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- English-only code comments
- Pas em-dash
- Pas secrets in commits (admin signing key warren-backend-api)
- TDD strict warren-core (RED → GREEN → REFACTOR)
- 5 concurrents comparison standard quand pertinent
- Pas Cure53 mention
- `hcloud --context warren` exclusif (jamais poka-perso)
- Push warren-core + warren-app au fil de l'eau

---

## 7. Memory updates attendus

À ajouter dans warren-app memory :
- `warren_session_e_delivered.md`, verdict + scope + caveats
- Update `MEMORY.md` index

À ajouter dans warren-core memory si redeploy warren-backend-api :
- Update `warren_backend_server.md` (version + date redeploy)
- Memory dédié si schema migration non-trivial : `warren_exit_id_schema.md`

---

## 8. Commencer maintenant

Lis le brief en entier, le design doc `.planning/a4-pubkey-pinning-design.md`, les sources §3 en parallèle, attaque E.1.1. Plein mandat §0.5 mais ATTENTION particulière §0.0 INVIOLABLE git ET §0.5 escalade case 3 (breaking /v1).

Cette session débloque le pinning pubkey livré scaffold session A. Sans exit_id stable, le différenciateur sécurité Warren reste dormant. Effort court, valeur sécurité haute.

Bonne route.
