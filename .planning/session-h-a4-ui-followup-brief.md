# Session H — A.4 UI follow-up (modal + gRPC + persistance + multi-hop pinning + forensic)

> Brief d'agent autonome warren-app (surface principale) + warren-core (mineur si nécessaire).
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session courte : surface UI Electron + gRPC + persistance, déblocage UX A.4 livré daemon.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : 0 EUR (pas de bench/redeploy nécessaire).
**Pré-conditions** :
- warren-app `main` HEAD `3c371fb8+` (post-Session E pin bump 8b0e345)
- warren-core `main` HEAD `8b0e345+` (post-Session E exit_id + verify hook)
- Verify hook A.4 actif daemon-side (`produce_warren_tunnel_params` channel mpsc présent, pas consommé)

**Objectif** : finaliser la surface UI Electron du pinning pubkey A.4 livré daemon-side Session E. Sans cette session, le verify hook actif refuse silencieusement les connexions mismatch (surface `NoMatchingRelay` daemon error) sans modal user-facing + sans Trust/Reject CTAs. UX cassée jusqu'au fix.

Sous-phases (séquentielles autonomes) :

1. **H.1 — Setup worktree warren-app dédié** (~30 min)
2. **H.2 — UI modal Electron `WarrenPubKeyWarning.tsx` + i18n** (~1-1.5j)
3. **H.3 — gRPC `TrustNewExitKey` + `ResetPinnedExitKeys` RPCs** (~1j)
4. **H.4 — Persistance settings.json depuis verify hook (channel consumer)** (~0.5-1j)
5. **H.5 — Multi-hop pubkey pinning (design + impl si court)** (~0.5-1j)
6. **H.6 — Forensic country/city blank-on-insert fix** (~0.5j)
7. **H.7 — Tests E2E + rapport + memory** (~0.5j)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard. Préserver fichiers modified/untracked. Violation = scope error CRITIQUE.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût Hetzner > 0.30 EUR (n/a)
3. Breaking change /v1 wire format ou gRPC management_interface.proto majeur (note : ajout 2 RPCs non-breaking, OK)
4. Signing key prod
5. **Spécifique session H** : si multi-hop pubkey pinning (H.5) demande refactor cross-repo majeur (> 2-3j seul), escalader pour décision split en session séparée vs continuer

Décisions tactiques agent autorisées :
- UI modal placement : modal overlay full-screen vs banner top vs floating notification — recommandation modal overlay (force user attention pour security event)
- gRPC RPC payload format : `TrustNewExitKey(exit_id, new_pubkey)` vs flexible (Tonic message)
- Persistance schema : ajouter dans settings.json existant (Mullvad pattern) vs sqlite warren-tunnel cache (Session A.4 storage)
- Multi-hop pinning : par exit only OR par (entry, exit) tuple — recommandation par exit only (entry rotation autorisée sans warning)
- Forensic country/city : SQLite COALESCE vs default values, choix idiomatique

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

Cf. memory `feedback_parallel_agents_same_worktree`. Worktree dédié :

```bash
cd /Users/poka/dev/warrenBros/warren-app
git fetch origin
git worktree add ../warren-app-a4-ui main
cd ../warren-app-a4-ui
git status                                  # clean main 3c371fb8+
```

Cleanup en fin de session :
```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree remove ../warren-app-a4-ui
```

---

## 1. Setup initial

```bash
# 0.6 worktree
cd /Users/poka/dev/warrenBros/warren-app
git fetch origin
git worktree add ../warren-app-a4-ui main
cd ../warren-app-a4-ui

git log --oneline -5
git remote -v
```

Lire sources Session E :
```bash
cat .planning/session-e-report.md
cat .planning/a4-pubkey-pinning-design.md
```

---

## 2. Optimisations agent

- Read warren-app surface Electron + Rust daemon + gRPC proto en PARALLÈLE
- Tests TDD groupés en fin de sous-phase
- Push warren-app au fil de l'eau

---

## H.1 — Setup worktree warren-app dédié (~30 min)

Cf. §0.6. Worktree créé.

### Critères GO H.1

- Worktree `../warren-app-a4-ui` opérationnel
- `cargo check --workspace` PASS
- Sources Session E + design A.4 lues

---

## H.2 — UI modal Electron `WarrenPubKeyWarning.tsx` + i18n (~1-1.5j)

### Scope H.2

1. **H.2.1** Composant `desktop/packages/mullvad-vpn/src/renderer/components/WarrenPubKeyWarning.tsx` :
   - Modal overlay full-screen Electron (z-index élevé, focus trap)
   - Title : "Server identity changed" / "Identité du serveur changée"
   - Body : "The Warren exit server you previously trusted has a different cryptographic identity now. This may indicate a legitimate server key rotation OR a man-in-the-middle attack."
   - Details collapsible : exit_id (hex truncated 8 chars + tooltip full), pubkey old (truncated) + pubkey new (truncated), first_seen + last_seen timestamps
   - 3 CTAs :
     - **"Trust new key"** — primary action, calls gRPC TrustNewExitKey, dismiss modal, retry connect
     - **"Reject (disconnect)"** — secondary, dismiss modal + remain disconnected
     - **"Report to Warren"** — tertiary, POST /v1/incidents/pubkey-mismatch (déjà câblé Session E) + dismiss + remain disconnected
2. **H.2.2** Trigger logic : composant écoute WarrenStatus update via Redux/Context. Si `WarrenStatus.pubkey_mismatch_pending: Option<{exit_id, old_pubkey, new_pubkey}>` set, monter le modal. Surface via gRPC streaming Updates (existant Session B M4.H.C).
3. **H.2.3** i18n FR + EN strings dans `desktop/packages/mullvad-vpn/locales/{en,fr}/messages.po` :
   - "Server identity changed" / "Identité du serveur changée"
   - "Trust new key" / "Faire confiance à la nouvelle clé"
   - "Reject" / "Refuser"
   - "Report to Warren" / "Signaler à Warren"
   - etc.
4. **H.2.4** A11y : `role="dialog"`, `aria-labelledby`, focus initial sur primary CTA, ESC pour Reject
5. **H.2.5** Tests UI : `desktop/packages/mullvad-vpn/test/renderer/components/WarrenPubKeyWarning.test.tsx` (Jest + RTL) :
   - Modal monté si pubkey_mismatch_pending set
   - Modal demonté si null
   - 3 CTAs déclenchent les actions correctes
   - i18n FR+EN render OK

### Critères GO H.2

- Composant `WarrenPubKeyWarning.tsx` livré
- 3 CTAs câblés
- i18n FR+EN complète
- Tests UI 5/5 PASS

### Décisions tactiques H.2

- Modal overlay (vs banner) : force attention security event
- Truncation pubkey : afficher 8 hex chars + ... + 8 hex chars (recognizable mais lisible)
- Pas de "Don't show again" CTA (chaque mismatch = decision explicit)

---

## H.3 — gRPC `TrustNewExitKey` + `ResetPinnedExitKeys` RPCs (~1j)

### Scope H.3

1. **H.3.1** `mullvad-management-interface/proto/management_interface.proto` :
   - `rpc TrustNewExitKey(TrustNewExitKeyRequest) returns (TrustNewExitKeyResponse);`
   - `rpc ResetPinnedExitKeys(google.protobuf.Empty) returns (ResetPinnedExitKeysResponse);`
   - Messages :
     - `TrustNewExitKeyRequest { exit_id: bytes (16), new_pubkey: bytes (32) }` 
     - `TrustNewExitKeyResponse { result: enum { Ok, ExitNotFound, PubkeyMismatch, IoError } }`
     - `ResetPinnedExitKeysResponse { reset_count: uint32 }`
2. **H.3.2** Daemon handlers `mullvad-daemon/src/management_interface.rs` (ou équivalent) :
   - `TrustNewExitKey` : load PinnedKeyStore, find entry for exit_id, replace pubkey, save, return Ok
   - `ResetPinnedExitKeys` : load store, count entries, clear all, save, return count
3. **H.3.3** Daemon-side : send `WarrenStatus { pubkey_mismatch_pending: Some({exit_id, old, new}) }` lorsque verify hook trigger mismatch (channel mpsc Session E à consommer)
4. **H.3.4** Renderer Electron `desktop/packages/mullvad-vpn/src/main/ipc/grpc-handlers.ts` : expose IPC handler pour invoke depuis renderer
5. **H.3.5** Tests Rust daemon : unit tests handlers (mock store)
6. **H.3.6** Tests Rust gRPC integration : Tonic in-process test send RPC, assert response

### Critères GO H.3

- 2 RPCs ajoutées proto + handlers
- WarrenStatus étendu pubkey_mismatch_pending
- IPC bridge Electron
- Tests Rust handlers + gRPC PASS

### Décisions tactiques H.3

- exit_id wire format : `bytes` (16) protobuf, parsé en `ExitId([u8;16])` Rust (cf. Session E)
- pubkey wire format : `bytes` (32) protobuf, Ed25519
- Non-breaking : ajout RPCs additif

---

## H.4 — Persistance settings.json depuis verify hook (channel consumer) (~0.5-1j)

### Scope H.4

Le verify hook Session E crée un channel `mpsc::Sender<PubkeyEvent>` mais aucun consumer. Conséquence : events fired daemon-side mais perdus, pas persistés sur disque, pas surface vers UI.

1. **H.4.1** Identifier le channel mpsc + producer (Session E `produce_warren_tunnel_params`)
2. **H.4.2** Créer consumer task dans `mullvad-daemon` (ou warren-tunnel adapter) :
   - `tokio::spawn` task qui poll le channel
   - On PubkeyEvent::Pinned → persiste dans PinnedKeyStore (déjà fait Session A.4 scaffold ? vérifier) + sauvegarde settings.json
   - On PubkeyEvent::Mismatch → set WarrenStatus.pubkey_mismatch_pending + n'autorise pas connect (déjà câblé)
   - On PubkeyEvent::Trusted (post TrustNewExitKey) → update store + save
   - On PubkeyEvent::Reset → clear store + save
3. **H.4.3** PinnedKeyStore persistence schema : settings.json `warren.pinned_exit_keys: { [exit_id_hex]: { pubkey_hex, first_seen_unix, last_seen_unix } }`
4. **H.4.4** Migration : si settings.json existant n'a pas la key `warren.pinned_exit_keys`, defaulte à `{}` (no crash)
5. **H.4.5** Tests Rust : restart daemon → pinned keys persistés, mismatch detection cohérente

### Critères GO H.4

- Consumer task câblé
- PinnedKeyStore persisté settings.json
- Migration backward compat
- Tests restart-survival PASS

### Décisions tactiques H.4

- Stockage settings.json (Mullvad pattern) vs sqlite séparé : settings.json (réutilise infrastructure existante mullvad-settings crate)
- Sync write vs async write : sync OK pour storage léger (~10-100 entries)

---

## H.5 — Multi-hop pubkey pinning (design + impl si court) (~0.5-1j)

### Scope H.5

Session E pinning : single-hop exit pubkey only. Multi-hop = pas câblé.

1. **H.5.1** Design : pinning par (entry_relay_pubkey, exit_pubkey) tuple OR exit_pubkey only ?
   - Recommandation : exit_pubkey only (entry rotation backend authorized sans warning user, ne sert qu'à blinder route)
   - Threat model : adversaire qui swap exit_pubkey = high-severity (déchiffre traffic). Adversaire qui swap entry_pubkey = low-severity (juste rerouting blind hop).
2. **H.5.2** Adapter verify hook Session E pour multi-hop path : `produce_warren_tunnel_params` doit accepter multi-hop config + extraire exit_pubkey final + pinner sur exit_id stable (déjà via Session E)
3. **H.5.3** Test : connect multi-hop avec exit X → pubkey pinned. Reconnect multi-hop avec exit X (même entry OU entry rotation) → match. Reconnect multi-hop avec exit X different pubkey → mismatch detected.
4. **H.5.4** Si H.5 demande > 1j (refactor verify hook majeur) : ESCALADE case 5, défer en session H.5-followup, continuer H.6

### Critères GO H.5

- Design documenté
- Implémentation simple OK si effort < 1j
- Tests multi-hop pinning PASS

### Décisions tactiques H.5

- Si refactor verify hook > 1j : ESCALADE pour décision split session
- Pinning par exit_pubkey only (pas entry)

---

## H.6 — Forensic country/city blank-on-insert fix (~0.5j)

### Scope H.6

Mentionné Session E §4.3 caveat 5. Lors du POST `/v1/incidents/pubkey-mismatch`, certains champs `country` / `city` sont blank-on-insert dans SQLite warren-api côté backend.

1. **H.6.1** Identifier dans warren-api `handlers.rs` ou `incidents.rs` le code path POST
2. **H.6.2** Default values : si client n'envoie pas country/city (mobile par exemple), backend infer via GeoIP IP source OR store `Unknown` string
3. **H.6.3** Test integration warren-api : POST avec country/city absent → row inserted avec defaults non-blank

### Critères GO H.6

- Schema warren-api fix défaut country/city
- Test integration PASS

### Décisions tactiques H.6

- Si requires warren-core commit (warren-api handlers vivent warren-core) : OK, commit warren-core dans worktree warren-core ad-hoc OR via warren-app pin bump si déjà committé par autre session
- GeoIP : utiliser MaxMind GeoLite2 si déjà présent warren-core, sinon "Unknown" plain string

---

## H.7 — Tests E2E + rapport + memory (~0.5j)

### Scope H.7

1. **H.7.1** Tests E2E warren-app :
   - First connect to exit X → no modal (TOFU pin)
   - Reconnect to X same pubkey → no modal (match)
   - Reconnect to X different pubkey → modal monté
   - Click "Trust new key" → gRPC TrustNewExitKey → reconnect OK
   - Click "Reject" → reste disconnected
   - Click "Report" → POST /v1/incidents + reste disconnected
   - Settings "Reset pinned keys" → tous cleared + connect retraite TOFU
2. **H.7.2** Rapport `.planning/session-h-report.md` : verdict + caveats + memory updates
3. **H.7.3** Memory `warren_session_h_delivered.md` + update MEMORY.md
4. **H.7.4** Commit final + push warren-app
5. **H.7.5** Cleanup worktree

### Critères GO H.7

- 7+ tests E2E PASS
- Rapport rédigé
- Memory updated
- Worktree cleaned

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-app
- `.planning/session-e-report.md` (caveats §4.3 source)
- `.planning/a4-pubkey-pinning-design.md` (design doc)
- `mullvad-daemon/src/management_interface.rs` (gRPC handlers pattern)
- `mullvad-management-interface/proto/management_interface.proto` (proto schema)
- `mullvad-daemon/src/lib.rs` (state machine cross-OS)
- `crates/talpid-warren-tunnel/src/lib.rs` (verify hook Session E)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/warren-multi-hop-settings/WarrenMultiHopSettingsView.tsx` (pattern Warren UI)
- `desktop/packages/mullvad-vpn/locales/{en,fr}/messages.po` (i18n)
- `mullvad-settings/` crate (settings.json persistence pattern)

### warren-core (lecture seule, pas de modif sauf H.6 si nécessaire)
- `crates/warren-tunnel/src/exit.rs` (PinnedKeyStore ?)
- `crates/warren-api/src/handlers.rs` (incidents endpoint Session E)

---

## 4. Plan d'exécution (séquentiel)

```
H.1 Worktree setup (30 min)
H.2 UI modal + i18n (1-1.5j)
H.3 gRPC RPCs + handlers (1j)
H.4 Persistance settings + consumer task (0.5-1j)
H.5 Multi-hop pinning (0.5-1j ou escalade)
H.6 Forensic fix (0.5j)
H.7 Tests E2E + rapport + cleanup (0.5j)
```

Total ~3-5j wall-clock.

---

## 5. Critères GO ULTIMATE session H

- ✅ H.1-H.7 critères GO PASS (H.5 GO PARTIAL OK si escalade)
- ✅ UI modal Warren-branded i18n FR+EN livré
- ✅ 2 gRPC RPCs livrées + handlers
- ✅ Persistance settings.json fonctionnelle
- ✅ Forensic country/city blank fixé
- ✅ Tests E2E 7/7 PASS
- ✅ `cargo test --workspace` warren-app PASS + clippy strict PASS
- ✅ `npm test` desktop UI PASS
- ✅ Rapport + memory rédigés
- ✅ Worktree cleaned

Verdict GO PARTIEL acceptable si :
- H.5 multi-hop pinning escaladé (sera livré en H.5-followup)

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé obligatoire
- English-only code comments (Rust + TypeScript)
- Pas em-dash
- Pas secrets in commits
- 5 concurrents comparison standard quand pertinent
- Pas Cure53 mention

---

## 7. Memory updates attendus

À ajouter dans warren-app memory :
- `warren_session_h_delivered.md`
- Update MEMORY.md

---

## 8. Commencer maintenant

Worktree §0.6, sources §3 en parallèle, attaque H.2.1. Plein mandat §0.5.

Sans cette session, le pinning A.4 livré Session E reste UX-cassé (mismatch surface NoMatchingRelay generic, pas de modal user-friendly). Effort court, valeur UX immédiate.

Bonne route.
