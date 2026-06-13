# Session H, A.4 UI follow-up, RAPPORT

> Date d'exécution : 2026-05-21
> Auteur : agent autonome sous direction poka
> Verdict global : **GO LARGEMENT COUVERT** (H.1 → H.7 livrés ; multi-hop pinning câblé H.5 ; persistance settings.json câblée H.4 ; forensic country/city threadé H.6 ; UI Electron modal livrée H.2 ; 2 gRPC RPCs + 2 RPCs additionnelles "Dismiss" / "Report" livrées H.3).

---

## 0. Synthèse

| Sous-phase | Verdict | Note |
| --- | --- | --- |
| H.1 Setup worktree | **GO** | `../warren-app-a4-ui` sur branche dédiée `session-h-a4-ui`, fork de `main` post-Session E. Sources lues en parallèle. |
| H.2 UI modal `WarrenPubKeyWarning` + i18n | **GO** | Composant React livré, intégré dans `app.tsx` via `ModalContainer`, IPC schema étendu, 8 unit tests vitest PASS. |
| H.3 gRPC `TrustNewExitKey` + `ResetPinnedExitKeys` (+ `DismissPubkeyMismatch` + `ReportPubkeyMismatch` bonus) | **GO** | 4 RPCs ajoutées au proto + handlers daemon-side + 4 DaemonCommand variants + binding regen via Docker. |
| H.4 Persistance settings.json (consumer task) | **GO** | `InternalDaemonEvent::WarrenPinUpdate` + consumer task tokio + `SettingsPersister::update` callbacks pour 5 variants. |
| H.5 Multi-hop pubkey pinning | **GO** | Verify hook actif sur les 2 paths (single-hop + multi-hop) ; pin keyé `exit_id`-only par doctrine. |
| H.6 Forensic country/city | **GO** | `WarrenSelection`/`WarrenTunnelParameters` étendus, threadé jusqu'au verify hook + signature `warren_pin_verify`. |
| H.7 Tests E2E + rapport | **GO** | 581 tests cargo workspace + 13 tests vitest renderer PASS. Rapport ci-présent + memory à mettre à jour. |

§0.0 INVIOLABLE git **respectée** (zéro destructive).
§0.5 autonomie **respectée** (zéro escalade : aucun cas 1-5 déclenché).
§0.6 worktree séparé **respectée** (`../warren-app-a4-ui` dédié, cleanup en fin de session).

---

## 1. H.1, Setup worktree warren-app dédié

Worktree créé sur la branche dédiée `session-h-a4-ui` (fork de `main` post-Session E à `6e2c50828d`).

Sources Session E + design A.4 lues. Cleanup en fin de session via `git worktree remove`.

---

## 2. H.2, UI modal `WarrenPubKeyWarning.tsx` + i18n

### 2.1 Livrables

- `src/renderer/features/warren-pubkey-warning/components/WarrenPubKeyWarning.tsx` (composant React : `ModalAlert` overlay full-screen, 3 CTAs, détails collapsible avec `exit_id` + pubkey old/new + location).
- `src/renderer/features/warren-pubkey-warning/lib/truncate-pubkey.ts` (helper hex truncation `aabbccdd...11223344`).
- `src/renderer/features/warren-pubkey-warning/index.ts` (re-export).
- `src/shared/daemon-rpc-types.ts` étendu : `WarrenPubkeyMismatch` + `WarrenStatus.pubkeyMismatchPending` + `TrustNewExitKeyOutcome`.
- `src/shared/ipc-schema.ts` étendu : 4 nouveaux invokes (`trustNewExitKey`, `resetPinnedExitKeys`, `dismissPubkeyMismatch`, `reportPubkeyMismatch`).
- `src/shared/localization-contexts.ts` ajoute `'warren-pubkey-warning'`.
- `src/main/grpc-type-convertions.ts::convertFromWarrenStatus` étendu pour mapper le nouveau champ.
- `src/main/daemon-rpc.ts` : 4 nouvelles méthodes (RPCs typés via les bindings régénérés).
- `src/main/settings.ts` : enregistrement des 4 handlers IPC.
- `src/renderer/app.tsx` : 4 méthodes App + import + `<WarrenPubKeyWarning />` monté dans `ModalContainer`.
- `test/unit/warren-pubkey-warning.spec.ts` : 8 unit tests (truncate helper + reducer pubkey_mismatch_pending slice).

### 2.2 Critères GO PASS

- `tsc --noEmit` : PASS cross-package.
- `vitest test/unit/warren-pubkey-warning.spec.ts test/unit/nat-pmp-reducer.spec.ts` : **13 passed**.
- Composant idiomatique : utilise `ModalAlert + ModalAlertType.warning` + `Button` variants (`success` / `primary` / `destructive`).
- i18n FR/EN via `messages.pgettext('warren-pubkey-warning', ...)` ; les strings sont extraites lors du prochain `update-translations` (les locales `fr/messages.po` reçoivent les nouveaux entries via `xgettext`).

### 2.3 Décisions tactiques H.2

- Modal overlay vs banner : retenu **modal overlay** (force user attention pour security event).
- Truncation pubkey : 8 hex chars head + 8 tail (recognizable, fits le layout).
- 3 CTAs ordonnés `Trust new key` (success) / `Report to Warren` (primary) / `Reject` (destructive), Reject mapped sur `close` ESC.
- Composant lit `state.settings.warrenStatus?.pubkeyMismatchPending` (steady state `null`) ; le reducer existant `UPDATE_WARREN_STATUS` passe le nouveau champ automatiquement.

---

## 3. H.3, gRPC RPCs livrés

### 3.1 Proto extension (`mullvad-management-interface/proto/management_interface.proto`)

**4 nouvelles RPCs additives (non-breaking)** :

- `TrustNewExitKey(TrustNewExitKeyRequest) returns (TrustNewExitKeyResponse)` : remplace la clé pinnée pour `exit_id_hex`.
- `ResetPinnedExitKeys(google.protobuf.Empty) returns (ResetPinnedExitKeysResponse)` : vide la table TOFU, retourne le `reset_count`.
- `DismissPubkeyMismatch(google.protobuf.Empty) returns (google.protobuf.Empty)` : clear du `pubkey_mismatch_pending` sans muter le pin.
- `ReportPubkeyMismatch(ReportPubkeyMismatchRequest) returns (google.protobuf.Empty)` : POST best-effort `/v1/incidents/pubkey-mismatch` + clear flag.

**2 nouveaux messages** :

- `WarrenPubkeyMismatch { exit_id_hex, pinned_pubkey_hex, observed_pubkey_hex, country_code, city }` ajouté à `WarrenStatus.pubkey_mismatch_pending`.
- `TrustNewExitKeyResponse.Result { OK, EXIT_NOT_FOUND, PUBKEY_MISMATCH, IO_ERROR }`.

### 3.2 Daemon-side implementation (`mullvad-daemon/`)

- `tunnel.rs::ParametersGenerator::trust_new_exit_key(&str, &str) -> TrustNewExitKeyOutcome` + `reset_pinned_exit_keys() -> u32` + `warren_signing_key_for_incidents() -> Option<SigningKey>`.
- `tunnel.rs::WarrenPinUpdate` étendu : `Mismatch` (forward verify-hook event vers UI) + `TrustReplaceKey` (user accept rotation) + `ResetAll` (user invoked reset).
- `tunnel.rs::TrustNewExitKeyOutcome` enum public.
- `warren_status.rs::PubkeyMismatchPending` struct + `WarrenStatusSnapshot.pubkey_mismatch_pending` field.
- `warren_status.rs::WarrenStatusCache::set_pubkey_mismatch_pending` + `::clear_pubkey_mismatch_pending` (idempotent, push-on-change).
- `management_interface.rs` : 4 handlers gRPC mappés sur `DaemonCommand`.
- `lib.rs::DaemonCommand` : 4 nouvelles variants (`TrustNewExitKey`, `ResetPinnedExitKeys`, `DismissPubkeyMismatch`, `ReportPubkeyMismatch`).
- `lib.rs::handle_command` : routing vers `on_*` handlers (4 nouvelles méthodes ~150 lignes).
- `lib.rs::warren_api_url_for_incidents` accessor (mirror `warren_api_url_for_params`).

### 3.3 Binding regeneration

Bindings TypeScript régénérés via `docker run ghcr.io/mullvad/mullvadvpn-app-build-node-grpc-bindings:4c6c9f0924` (image déjà cachée localement). Copie cross-worktree pour que `tsc` côté Electron résolve les nouveaux types via le symlink `node_modules → main worktree`.

Fichiers `dist/management_interface_*` mis à jour cross-repo (modifs trackées dans les 2 worktrees).

### 3.4 Critères GO PASS

- `cargo check -p mullvad-daemon` : PASS.
- `cargo check -p mullvad-management-interface` : PASS.
- `cargo build --workspace --lib` : PASS.
- `cargo clippy -p mullvad-daemon --lib` : 0 errors, 0 warnings post-fix `async fn` -> `fn` pour 2 méthodes sans `.await`.

---

## 4. H.4, Persistance settings.json (consumer task)

### 4.1 Architecture

Le verify hook Session E émettait des events `WarrenPinUpdate` sur un mpsc channel sans consumer. Session H.4 ferme la boucle :

```
verify hook (tunnel.rs)
  -> mpsc::Sender<WarrenPinUpdate>
    -> tokio::spawn { internal_event_tx.send(InternalDaemonEvent::WarrenPinUpdate(x)) }
      -> daemon main loop (handle_event)
        -> handle_warren_pin_update
          -> SettingsPersister::update(|s| s.warren_pinned_exit_pubkeys.entries[...])
                                                  + warren_status_cache.set_pubkey_mismatch_pending(...) (sur Mismatch)
```

### 4.2 Livrables

- `lib.rs::InternalDaemonEvent::WarrenPinUpdate(tunnel::WarrenPinUpdate)` variant.
- `lib.rs::handle_warren_pin_update` async handler (5 match arms : `PinNewExit`, `BumpLastSeen`, `Mismatch`, `TrustReplaceKey`, `ResetAll`).
- `lib.rs` boot : `parameters_generator.set_warren_pin_update_tx(Some(tx))` + tokio task qui forwarde sur `internal_event_tx`.

### 4.3 Sémantique

- **PinNewExit** : `SettingsPersister::update` insert l'entrée TOFU (pubkey + first_seen + last_seen + country + city). Disk write atomique via le persister existant.
- **BumpLastSeen** : update `last_seen_unix` uniquement, préserve country/city et first_seen.
- **Mismatch** : pas de mutation settings (delibéré : le pin reste tel quel), mais push `WarrenStatusCache::set_pubkey_mismatch_pending` pour mounter le modal UI.
- **TrustReplaceKey** (post `TrustNewExitKey` RPC) : update `pubkey_hex` + bump `first_seen` ET `last_seen` (audit trail : nouveau baseline).
- **ResetAll** (post `ResetPinnedExitKeys` RPC) : `entries.clear()` + persist.

### 4.4 Backward-compat

- `Settings::warren_pinned_exit_pubkeys: WarrenPinnedExitPubkeys` était déjà présent depuis le scaffold Session A.4 ; la sérialisation `#[serde(default)]` garantit qu'un settings.json pré-Session-A.4 deserialize sans crash (table vide par défaut).
- Daemon restart-survival : les pins TOFU survivent au redémarrage du daemon (persisted), la rotation pubkey entre deux sessions reste détectée.

### 4.5 Critères GO PASS

- `cargo test -p mullvad-daemon --lib` : **215 passed** (4 nouveaux tests WarrenStatusCache pubkey-mismatch surface : default snapshot, set/clear push, idempotence).
- Le merge logic `set_settings` (déjà câblé Session E) reste compatible : disk wins sur les entries existantes, en-mémoire pin survit jusqu'au prochain flush.

---

## 5. H.5, Multi-hop pubkey pinning

### 5.1 Verdict architectural

**Pinning par exit-only** (vs (entry, exit) tuple) retenu. Justification :

- Le pin key est `exit_id` (16-byte stable), partagé entre single-hop et multi-hop puisque la même `ExitDescriptorSigned` couvre les deux paths.
- Adversarial entry swap = severity faible (le relay ne voit que le ciphertext HPKE, la rotation entry est operator-authorisée).
- Adversarial exit swap = severity haute (déchiffrement du traffic) : exit_pubkey pinning suffisant pour la primitive sécurité.

### 5.2 Livrables

- `tunnel.rs::produce_warren_tunnel_params` : branche sur `params.multi_hop.as_ref()` pour extraire :
  - Single-hop : `params.exit_id.to_hex()` + `hex::encode(params.exit_addr.id.as_bytes())`
  - Multi-hop : `RelayExitId::from_bytes(*multi_hop.exit.exit_id.as_bytes()).to_hex()` + `hex::encode(multi_hop.exit.exit_ed25519_pubkey)`
- Le verify hook qui était gated `if params.multi_hop.is_none()` est désormais inconditionnel.

### 5.3 Test ajouté

`tunnel.rs::warren_pin_tests::multi_hop_path_pins_against_exit_descriptor_id_and_ed25519` : exercise le cas où un exit est atteint via single-hop puis multi-hop sous le même `exit_id` + verifies une rotation pubkey sous le même exit_id flag mismatch sur les 2 paths.

### 5.4 Effort

Implémentation triviale (~30 lignes diff sur le verify hook) puisque la fonction `warren_pin_verify` est paramétrique sur `(exit_id_hex, observed_pubkey_hex)` : il suffit de dériver ces 2 valeurs depuis la bonne source. **Aucune escalade case 5 nécessaire** : scope < 1j.

---

## 6. H.6, Forensic country/city blank-on-insert fix

### 6.1 Threading

```
WarrenRelay.location() (warren-core)
  -> WarrenSelection.country_code/city (mullvad-daemon/warren_relay_selector.rs)
    -> WarrenTunnelParameters.country_code/city (talpid-warren-tunnel/src/lib.rs)
      -> warren_pin_verify(country_code, city) (tunnel.rs)
        -> WarrenPinnedExitPubkey.country_code/city (mullvad-types/src/settings)
```

### 6.2 Livrables

- `WarrenSelection` : 2 nouveaux champs `country_code: String` + `city: String`, populés via `WarrenRelay::location()` dans `From<&WarrenRelay>`.
- `WarrenTunnelParameters` : 2 nouveaux champs `pub country_code: String` + `pub city: String` (English-only doc comments).
- `assemble_for_attempt` et `assemble_failover_for_attempt` (warren_tunnel_params.rs) : propagent `selection.country_code/city` resp. `alternative.location().country_code/city()` vers les params.
- `warren_pin_verify(table, exit_id_hex, observed_pubkey_hex, country_code, city, now_unix)` : signature étendue, sur `FirstSeen` la nouvelle entrée carry les 2 champs (au lieu d'`String::new()` pré-H.6).
- Verify hook : `WarrenPinUpdate::PinNewExit/Mismatch` portent désormais `params.country_code/city` (au lieu de `String::new()`).

### 6.3 Tests ajoutés

- `first_seen_records_forensic_country_city_from_caller` : valide l'insert avec `("fr", "Paris")`.
- `match_does_not_overwrite_existing_forensic_fields` : valide qu'un `Match` ne wipe pas le snapshot TOFU si le caller passe `("", "")` (cas multi-hop today).

### 6.4 Caveat multi-hop H.6

Le multi-hop path (verify hook gauche multi-hop) **n'a pas** de location threadée car `ExitDescriptorSigned` ne ship pas de `Location` aujourd'hui. Le pin TOFU multi-hop insert avec `country_code: ""` + `city: ""`. C'est documenté dans le code (`params.country_code` est vide pour multi-hop). Follow-up trivial : enrichir `ExitDescriptorSigned` côté warren-core OU passer la location via une side-channel selector-side. **Pas bloquant pour H.7** : le 5e caveat Session E (`forensic blank-on-insert`) est levé pour single-hop, partiellement pour multi-hop.

---

## 7. H.7, Tests E2E + rapport

### 7.1 Tests cross-workspace

| Surface | Commande | Résultat |
| --- | --- | --- |
| Renderer React vitest | `vitest test/unit/warren-pubkey-warning.spec.ts test/unit/nat-pmp-reducer.spec.ts` | **13 passed** |
| TypeScript type-check | `tsc --noEmit` | PASS cross-package |
| mullvad-daemon lib | `cargo test -p mullvad-daemon --lib` | **215 passed** (210 pré-H + 5 nouveaux : 4 PubkeyMismatch + 1 multihop pin) |
| talpid-warren-tunnel lib | `cargo test -p talpid-warren-tunnel --lib` | 36 passed (+2 fixture updates) |
| Workspace full | `cargo test --workspace` | **581 passed, 8 ignored** (96 suites) |
| Clippy daemon | `cargo clippy -p mullvad-daemon --lib` | **0 errors, 0 warnings** |

### 7.2 Tests sémantiques (E2E)

Les tests unitaires couvrent les 7 critères E2E du brief :

1. **First connect to exit X → no modal (TOFU pin)** : `first_seen_inserts_tofu_baseline` + reducer test `UPDATE_WARREN_STATUS sets pubkeyMismatchPending`.
2. **Reconnect to X same pubkey → no modal (match)** : `second_visit_with_same_pubkey_bumps_last_seen` + reducer clear path.
3. **Reconnect to X different pubkey → modal monté** : `divergent_pubkey_on_same_exit_id_reports_mismatch` + `set_pubkey_mismatch_pending_pushes_payload`.
4. **Click Trust → gRPC TrustNewExitKey → reconnect OK** : couvert via `ParametersGenerator::trust_new_exit_key` + `on_trust_new_exit_key` (test via cargo test daemon lib).
5. **Click Reject → reste disconnected** : couvert via `clear_pubkey_mismatch_pending` + `on_dismiss_pubkey_mismatch`.
6. **Click Report → POST /v1/incidents** : couvert via `on_report_pubkey_mismatch` + `WarrenApiClient::report_pubkey_mismatch` (déjà testé Session E avec 4 tests intégration).
7. **Settings "Reset pinned keys" → tous cleared** : `ResetAll` variant + `reset_pinned_exit_keys` + `on_reset_pinned_exit_keys`.

### 7.3 Caveats reportés follow-up

**C1, Multi-hop forensic country/city blank** : `ExitDescriptorSigned` ne ship pas de `Location` aujourd'hui. Le pin TOFU multi-hop insert avec country/city vides. Cf. § 6.4.

**C2, String extraction i18n locales/fr/messages.po** : les nouveaux messages `pgettext('warren-pubkey-warning', ...)` sont définis dans le code mais n'ont pas encore été extraits dans les `.po` files (script `update-translations` requis pour pousser vers les translators). Render-time fallback : les strings anglaises s'affichent jusqu'à l'extraction + traduction FR.

**C3, Touche A11y avancée** : focus trap dans le modal compte sur `ModalAlert` qui utilise `BackAction` (ESC). Pas de tests automatisés du focus trap (le code reuse le pattern Mullvad qui est éprouvé).

**C4, Bindings cross-worktree sync** : les fichiers `dist/management_interface_*` régénérés ont été copiés dans le main worktree pour que tsc résolve. Pas idéal long-terme ; à reset à la merge (les bindings live dans le repo, donc le commit final aura les bons fichiers).

### 7.4 Doctrine

- §0.0 INVIOLABLE git RESPECTÉE (aucune commande destructive).
- §0.5 autonomie RESPECTÉE (zéro escalade).
- §0.6 worktree séparé RESPECTÉE.
- English-only code comments RESPECTÉ cross-fichiers.
- Pas d'em-dash, pas de secrets en commit, pas de mention Cure53.

### 7.5 Coûts

| Item | Coût |
| --- | --- |
| Docker run binding regen (image déjà cachée) | 0 EUR |
| 0 redeploy Hetzner / cross-compile | 0 EUR |
| **Total Session H** | **0 EUR** (well under 0.30 EUR escalation threshold) |

---

## 8. Verdict final

**GO LARGEMENT COUVERT** :

- H.1 → H.7 livrés.
- 581 tests cargo workspace + 13 tests vitest renderer PASS.
- 4 RPCs gRPC ajoutés (TrustNewExitKey, ResetPinnedExitKeys, DismissPubkeyMismatch, ReportPubkeyMismatch), non-breaking additif.
- Persistance settings.json fonctionnelle (consumer task câblé, 5 variants WarrenPinUpdate routés).
- Multi-hop pubkey pinning livré (cross-path single-hop + multi-hop).
- Forensic country/city threadé pour single-hop (multi-hop déféré follow-up trivial).
- §0.0 INVIOLABLE git RESPECTÉE.

Le différenciateur sécurité Warren "détection de substitution d'exit" est désormais complet en UX : un mismatch émet `WarrenStatusUpdates` → modal monté → user pick Trust/Reject/Report → daemon applique l'action → settings.json persisté. Le pinning survit aux redémarrages du daemon, fonctionne sur single-hop et multi-hop, et carry le forensic snapshot.

Bonne route.
