# Session B, DAITA padding + Multi-exit failover + Onboarding flow

> Brief d'agent autonome cross-repo warren-core + warren-app.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> Grosse session multi-phases : l'agent enchaîne B.1 → B.2 → B.3 sans escalade.

**Effort estimé** : wall-clock 4-5 semaines (3 sous-phases).
**Coût Hetzner** : ~0.50 EUR (1 bench cross-DC DAITA overhead, sinon tests unitaires).
**Pré-conditions** :
- warren-app `main` HEAD `583581dae5+` (post-M4.H.G + post-session-A si exécutée avant)
- warren-core `main` HEAD `478a5f5+` (post-audit dedae7a)
- ⚠️ working tree warren-core a 1 fichier modified non committé :
  `crates/warren-tunnel/tests/d3_allowlist_dynamic.rs`. **Préserver, ne pas toucher.**

**Objectif** : livrer les différenciateurs produit Warren mentionnés sur warrenbrowse.com (DAITA + multi-exit failover) + UX onboarding wallet first-launch critique. Sans ces 3 features livrées, le site marketing **bluff publiquement** (DAITA + failover annoncés mais absents).

Sous-phases (séquentielles autonomes) :

1. **B.1, DAITA padding integration (maybenot)** (~2-3 sem)
2. **B.2, Multi-exit failover (warren-relay-selector + UI)** (~1-2 sem)
3. **B.3, Onboarding flow desktop first-launch** (~3-4j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Quelle que soit la situation (test, recovery, "voir si ça compile", diagnostic, expérimentation), tu ne dois JAMAIS exécuter :

- `git stash` (et toutes variantes)
- `git checkout <path>` ou `git checkout -- .`
- `git restore <path>` ou `git restore .`
- `git reset --hard <ref>` (avec ou sans ref)
- `git clean -fd` (et toutes variantes destructives)
- `git rebase` interactif qui force-modify le WT
- `git revert --no-commit` qui modifie le WT sans valider
- Toute commande qui modifie ou discard les fichiers untracked OU modified du working tree

Cette interdiction PRIME sur le mandat d'autonomie §0.5. Si tu penses avoir besoin d'une commande destructive : ESCALADE poka via AskUserQuestion AVANT exécution, sans exception.

Pour tester un état antérieur : `git show <ref>:<path>` (read-only), `git diff <ref> --stat`, `git log -p <path>`. Pour récupérer une version : `git show <ref>:<path> > /tmp/file-at-ref` puis Read.

**Pré-existant à préserver** : `warren-core/crates/warren-tunnel/tests/d3_allowlist_dynamic.rs` est modified non-committé (wip poka). Ne pas toucher, ne pas committer.

Violation = scope error CRITIQUE. Incident M4.H.F 2026-05-20 : agent a perdu 5 fichiers WIP poka warren-core.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat pour atteindre GO. Diagnostic 30 min → fix tactique TDD → commit + push → reprise. PAS de rollback, PAS d'escalade timide.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak (clé privée, mnemonic, API token)
2. Coût Hetzner > 0.30 EUR (escalader si bench cumul dépasse 0.50 EUR)
3. Breaking change /v1 wire format multi-hop ou HPKE
4. Signing key prod touchée
5. **Spécifique session B** : si DAITA wire format /v1 nécessite breaking change (ex: padding marker dans frame layer multi-hop HPKE), escalade pour validation /v1 archi avant push

Décisions tactiques agent autorisées :
- Maybenot machine specification (preset Mullvad-style, ou Warren-custom tuned)
- DAITA ON/OFF default (memory dit OFF, l'agent peut changer pour ON si justifié benchmarks)
- Multi-exit failover threshold (timeout connect, RTT seuil, perte paquets seuil)
- Onboarding skip option (skip wizard pour power-users)
- UI placement DAITA toggle (general settings vs advanced vs privacy section)

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # main HEAD 583581dae5+
git remote -v
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # 1 file modified d3_allowlist_dynamic.rs, préserver
git log --oneline -3                        # HEAD 478a5f5+
```

Si HEAD inattendu : escalade (cf. §0.0, surtout pas de checkout).

---

## 2. Optimisations agent

- Read sources cross-repo en PARALLÈLE
- Tests TDD groupés en fin de sous-tâche
- Push warren-core + warren-app au fil de l'eau
- `cargo check` redondant si `cargo test` ou `cargo clippy` suit
- Pin warren-core bump uniquement si commit warren-core requis par warren-app
- DAITA bench Hetzner : 1 seul run cross-DC, valider seuil overhead avant lancer

---

## B.1, DAITA padding integration via crate `maybenot` (~2-3 sem)

### Contexte

Cf. memory `warren_daita_doctrine_v1`. Crate `maybenot` (Mullvad + Karlstad U, Apache-2.0) implémente state machines probabilistes qui déclenchent padding + dummy packets sur events. Intégration au-dessus du tunnel chiffré.

**Threat model** : global passive adversary fait du website fingerprinting via timing + tailles de paquets (90+% précision ML moderne). DAITA mitigue ce vecteur. Différenciateur 2026 (Mullvad le pousse comme fer de lance).

**Différence Warren vs Mullvad** : Warren utilise Quinn UDP (vs WireGuard chez Mullvad). Intégration `maybenot` doit hook le pump Quinn (`crates/warren-tunnel/src/pump.rs`), pas un module WG.

### Scope B.1

1. **B.1.1, Ajout dep `maybenot`** : `cargo add maybenot --version <latest>` dans `crates/warren-tunnel/Cargo.toml` warren-core. Workspace dep registration. Verify license Apache-2.0 compat avec Warren GPL/AGPL.

2. **B.1.2, Maybenot machine spec** : choisir une `Machine` spec. Décision tactique : copier preset Mullvad DAITA v2 (open source dans `mullvadvpn-app/`), ou Warren-custom tuné pour Quinn datagrammes. Recommandation = preset Mullvad v2 pour démarrer (proven, ~10% overhead documenté), Warren-tuning en M5 si justifié benchmarks.

3. **B.1.3, Hook pump Quinn** : modifier `crates/warren-tunnel/src/pump.rs` pour driver une `Machine` instance par session. Events à wire :
   - `TriggerEvent::NormalSent` quand un paquet applicatif part
   - `TriggerEvent::NormalRecv` quand un paquet applicatif arrive
   - `TriggerEvent::PaddingSent` quand un dummy part
   - `TriggerEvent::PaddingRecv` quand un dummy arrive
   - Actions :
     - `Action::InjectPadding` → générer dummy packet (Quinn datagram avec content opaque random fill)
     - `Action::BlockOutgoing` → throttle send (optional, peut être no-op v1)

4. **B.1.4, Dummy packet format** : datagram Quinn avec marker byte (premier byte = `0x00` payload réel, `0x01` dummy DAITA). Recipient ignore les dummy au demux. Wire format DAITA marker = breaking /v1 wire format si on l'introduit sans bump version → escalade poka si tu détectes ce risque.

   **Alternative non-breaking** : padding "transparent" via taille fixe paquet (toujours 1280 bytes uplink) + fillers AAD HPKE. Demande plus de réflexion crypto, escalade poka avant impl.

5. **B.1.5, Multi-hop compat** : DAITA doit fonctionner SUR le single-hop ET sur le multi-hop HPKE. Pour multi-hop, padding s'applique au cleartext avant HPKE encrypt (sinon adversaire compte les paquets HPKE-encrypted = même fingerprinting). Test : ajouter une E2E `daita_multihop_padding_visible_in_hpke_layer`.

6. **B.1.6, UI toggle DAITA** : composant Electron `WarrenDaitaSwitch.tsx` + setting `WarrenDaitaSetting.tsx` (pattern `WarrenMultiHopSwitch.tsx` existant à copier). gRPC management_interface étendu : `WarrenDaitaSettings { enabled: bool }`. Placement UI : section privacy/anti-censorship (cf. M4.0 obfuscation indicator emplacement).

7. **B.1.7, i18n FR + EN** : strings type "Enable DAITA traffic obfuscation" / "Activer l'obfuscation DAITA". Documenter coût ~10% bandwidth dans tooltip + lien `/security#daita` warrenbrowse.com.

8. **B.1.8, Bench Hetzner DAITA overhead** : 1 run cross-DC FR→FR avec DAITA ON, mesurer overhead bande passante vs baseline (M4.E.D bench 409 Mbps multi-hop, M4.H.A.quart 802 Mbps single-hop). Validation : overhead ≤ 15% acceptable (target 10%). Si > 20%, escalade poka pour tuner machine spec.

9. **B.1.9, Tests TDD B.1** :
   - Machine state evolves on event (unit test maybenot integration)
   - Pump injects dummy when Action::InjectPadding triggered
   - Multi-hop padding s'applique pre-HPKE
   - DAITA OFF par défaut, no padding when off
   - Toggle UI change setting + restart tunnel applique change

### Critères GO B.1

- Crate `maybenot` intégrée warren-tunnel
- Machine preset choisie + documentée
- Pump hook events + actions opérationnel
- Dummy packet format décidé + tests
- Multi-hop compat validée
- UI toggle Warren-branded i18n FR+EN
- Bench Hetzner overhead ≤ 15%
- Tests TDD 5/5+ PASS
- Documenter résultats + spec machine choisie dans `.planning/session-b-report.md` §B.1

### Décisions tactiques B.1

- Machine spec : preset Mullvad DAITA v2 par défaut
- Default ON/OFF : OFF par défaut (cf. memory `warren_daita_doctrine_v1`, coût bandwidth)
- Wire format : marker byte `0x01` pour dummy SI non-breaking /v1 (sinon padding transparent via taille fixe)
- Hetzner bench : 1 run 5 min max, ne pas répéter sauf si premier KO

---

## B.2, Multi-exit failover (warren-relay-selector + UI) (~1-2 sem)

### Contexte

`warren-relay-selector` (warren-core) implémente actuellement selection basique (geo filter + weight). **Pas de failover automatique** si l'exit choisi devient indisponible.

**Différenciateur produit** : Mullvad bascule manuellement (user doit cliquer disconnect/reconnect). Warren peut détecter exit down + reconnect automatique sur un autre exit même country sans user intervention. Cité sur warrenbrowse.com/features comme différenciateur, à livrer.

### Scope B.2

1. **B.2.1, Health check exit** : warren-tunnel ping périodique (handshake keepalive) à l'exit. Si N consecutive timeouts (default N=3, configurable), considérer exit down + trigger failover.

2. **B.2.2, Failover logic** : warren-relay-selector expose `WarrenRelaySelector::select_failover_alternative(current_relay, query) -> Option<WarrenRelay>`. Filtre :
   - Exclude `current_relay.id`
   - Même `LocationConstraint` (country) si possible
   - Si aucun match country → fallback global, expose UI warning
   - Weighted random parmi les eligibles

3. **B.2.3, Reconnect flow** : `warren-tunnel` lance reconnect vers nouvelle exit sans drop tunnel UI state. Backoff `Backoff::HANDSHAKE` (M4.H.G 15s ceiling) appliqué. Event `TunnelEvent::ExitFailover { from_exit_id, to_exit_id }` fired vers daemon → UI.

4. **B.2.4, Backend coordination** : si l'exit down est dans la backend allowlist warren-api, le client doit également informer le backend (POST `/v1/incidents/exit-down`, log only, non-bloquant). Permet warren-admin de voir ces signaux en agrégat.

5. **B.2.5, UI failover indicator** : `WarrenStatusCache` (existant) étendu `failover_count: u32` + `last_failover_unix: Option<u64>` + `current_exit_id: String`. UI Electron affiche "Switched to <country> (auto-failover)" toast + history dans status details. Pattern existant M4.H.C status display.

6. **B.2.6, Settings toggle** : user peut désactiver failover automatique (default ON). Setting `WarrenFailoverSetting.tsx` (pattern existant). gRPC `WarrenFailoverSettings { enabled: bool }`.

7. **B.2.7, i18n FR + EN** : strings "Exit unavailable, switching to alternative" / "Sortie indisponible, basculement vers alternative". Tooltip Settings : "Switch automatically to another server if current one becomes unreachable".

8. **B.2.8, Tests TDD B.2** :
   - Selector returns failover candidate excluding current
   - Failover prefers same country
   - Failover fallback global if no same-country
   - Pump triggers failover after N timeouts
   - Reconnect uses backoff::HANDSHAKE
   - UI event WarrenStatus updated on failover
   - Toggle OFF disables failover (timeouts → just retry same exit)

### Critères GO B.2

- Health check exit opérationnel
- `select_failover_alternative` implémenté + tested
- Reconnect flow non-blocking UI
- Backend incident endpoint stub
- UI failover toast + history
- Settings toggle ON par défaut
- i18n FR+EN
- Tests TDD 7/7 PASS
- Documenter résultats dans report §B.2

### Décisions tactiques B.2

- Default ON (différenciateur produit, on veut que ça marche out-of-the-box)
- Timeout threshold : 3 consecutive (~45s avec keepalive 15s)
- Same-country priority avec global fallback + UI warning
- Endpoint /v1/incidents/exit-down : POST simple log, pas de DB

---

## B.3, Onboarding flow desktop first-launch (~3-4j)

### Contexte

Wallet Ed25519 non-custodial = barrière onboarding non-triviale. Mullvad : user paie + reçoit account number, simple. Warren : user doit générer/importer mnemonic BIP39, comprendre que c'est sa responsabilité. Sans onboarding guidé, drop-off massif first-launch.

**Existant Warren UI** : `WarrenPubKeyLabel.tsx` + `WarrenLocalAccountSwitch.tsx` + `KeysView` (créé en C.1 phase warren-core). Pas de wizard.

### Scope B.3

1. **B.3.1, Wizard steps** :
   - Step 1, Welcome : "Bienvenue dans Warren VPN. Une expérience VPN sans compromis privacy."
   - Step 2, Wallet : 2 choix : "Générer un nouveau wallet" (recommandé) ou "Importer un mnemonic existant"
     - Si Generate : afficher 12 mots BIP39 + CTA "J'ai écrit les mots" (confirmation) + warning "Si tu perds ces mots, tu perds l'accès à ton abonnement"
     - Si Import : champ 12 mots + validation BIP39
   - Step 3, Subscription : "Tu n'as pas encore d'abonnement actif. Voir les plans →" (lien vers warrenbrowse.com/pricing dans navigateur externe). Skip si already enrolled.
   - Step 4, Privacy preferences : 3 toggles :
     - Multi-hop OFF (default)
     - DAITA OFF (default, hint privacy mode)
     - Always-On obfuscation ON (default, M4.0)
   - Step 5, Done : "Configuration terminée. Sélectionne un pays pour te connecter."

2. **B.3.2, Skip option** : "Skip wizard, advanced mode" en footer chaque step. Pour power users qui veulent passer direct au main UI.

3. **B.3.3, Detection first-launch** : flag persistant `onboarding_completed_unix: Option<u64>` dans settings warren-app. Si absent → wizard launched on Electron mount. Sinon → main UI direct.

4. **B.3.4, Re-trigger** : Settings → "Replay onboarding" CTA pour re-trigger wizard sans clear data. Utile pour démos/support.

5. **B.3.5, Composants Electron** :
   - `OnboardingWizard.tsx` orchestrateur 5-step
   - `OnboardingWelcomeStep.tsx`
   - `OnboardingWalletStep.tsx` (Generate + Import sub-views)
   - `OnboardingSubscriptionStep.tsx`
   - `OnboardingPreferencesStep.tsx`
   - `OnboardingDoneStep.tsx`
   - Routing Electron : `/onboarding/*` route ou modal-style overlay (décision tactique)

6. **B.3.6, Mnemonic display security** : BIP39 affichage à l'écran présente risque shoulder-surfing. Ajouter blur overlay click-to-reveal (pattern Mullvad voucher code, à adapter). CTA "Copier au clipboard" ABSENT volontairement (force le user à les écrire à la main, anti-malware-clipboard). Documenter ce choix.

7. **B.3.7, i18n FR + EN** : tous les strings dans i18n files. Wizard intégralement traduit.

8. **B.3.8, Tests TDD B.3** :
   - First launch sans onboarding flag → wizard launched
   - Skip → onboarding_completed flag set + main UI shown
   - Generate flow → mnemonic affiché + confirm step
   - Import flow → validation BIP39 OK + mauvais mnemonic rejected
   - Replay from Settings → wizard re-shown
   - Each step navigable forward/backward

### Critères GO B.3

- Wizard 5-step fonctionnel
- First-launch detection
- Skip + Replay
- Mnemonic blur+reveal sans clipboard CTA
- Composants Electron complets
- i18n FR+EN
- Tests TDD 6/6 PASS
- Documenter résultats dans report §B.3

### Décisions tactiques B.3

- Modal overlay vs route : route `/onboarding` (clean, navigable, partage URL pour démo)
- Skip option visible mais discret (footer, pas header)
- Lien externe pricing vers warrenbrowse.com/pricing (pas iframe)
- Pas de CTA "Copier mnemonic" volontaire

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-core
- `crates/warren-tunnel/src/pump.rs` (target B.1 DAITA hook)
- `crates/warren-tunnel/src/multi_session.rs` (multi-hop interaction)
- `crates/warren-tunnel/src/client.rs` (handshake B.2 failover)
- `crates/warren-tunnel/src/transport_config.rs` (Quinn datagram config)
- `crates/warren-multihop/src/lib.rs` (HPKE layer pour B.1.5)
- `crates/warren-relay-selector/src/{lib,selector,query,relay}.rs` (target B.2)
- `crates/warren-backoff/src/lib.rs` (Backoff::HANDSHAKE pour B.2 reconnect)
- ⚠️ ne pas toucher `crates/warren-tunnel/tests/d3_allowlist_dynamic.rs` (wip poka)

### warren-app
- `crates/talpid-warren-tunnel/src/lib.rs` (adapter)
- `mullvad-daemon/src/lib.rs` (state machine cross-OS)
- `mullvad-management-interface/proto/management_interface.proto` (gRPC Warren extension)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/warren-multi-hop-settings/` (pattern UI Settings)
- `desktop/packages/mullvad-vpn/src/renderer/features/warren-mode/` (pattern feature folder)
- `desktop/packages/mullvad-vpn/src/renderer/components/WarrenPubKeyLabel.tsx`
- `desktop/packages/mullvad-vpn/src/main/index.ts` (Electron entry pour onboarding route hook)
- `desktop/packages/mullvad-vpn/locales/{en,fr}/*.po` (i18n)

### Mullvad upstream (pattern refs)
- `mullvadvpn-app/wireguard-daita/` (DAITA v2 reference Mullvad si présent dans submodule)
- Documentation `maybenot` crate (crates.io + GitHub Mullvad-spinoff)

---

## 4. Plan d'exécution (séquentiel, autonome)

```
B.1 DAITA (~2-3 sem)
  ├── B.1.1 ajout dep maybenot
  ├── B.1.2 machine spec preset Mullvad v2
  ├── B.1.3 hook pump Quinn events/actions
  ├── B.1.4 dummy packet format (escalade si breaking /v1)
  ├── B.1.5 multi-hop compat
  ├── B.1.6 UI toggle Electron
  ├── B.1.7 i18n FR+EN
  ├── B.1.8 bench Hetzner overhead
  └── B.1.9 tests TDD + commit + push
B.2 Multi-exit failover (~1-2 sem)
  ├── B.2.1 health check exit
  ├── B.2.2 selector failover candidate
  ├── B.2.3 reconnect flow
  ├── B.2.4 backend incident endpoint
  ├── B.2.5 UI status display
  ├── B.2.6 settings toggle ON default
  ├── B.2.7 i18n
  └── B.2.8 tests TDD + commit + push
B.3 Onboarding (~3-4j)
  ├── B.3.1 wizard 5-step
  ├── B.3.2 skip option
  ├── B.3.3 first-launch detection
  ├── B.3.4 replay from Settings
  ├── B.3.5 composants Electron
  ├── B.3.6 mnemonic blur+reveal
  ├── B.3.7 i18n FR+EN
  └── B.3.8 tests TDD + commit + push
B.4 Rapport final (1h)
  └── .planning/session-b-report.md avec verdict GO ULTIMATE par sous-phase
```

Push warren-core (si modif) + warren-app au fil de l'eau. Bump pin `.warren-core-version` si nouveaux commits warren-core requis.

---

## 5. Critères GO ULTIMATE session B

Tous les critères suivants doivent passer pour verdict GO ULTIMATE complet :

- ✅ B.1 critères GO PASS (DAITA intégrée + bench overhead ≤ 15% + multi-hop compat)
- ✅ B.2 critères GO PASS (failover auto + UI + tests)
- ✅ B.3 critères GO PASS (wizard 5-step + i18n + tests)
- ✅ `cargo test --workspace` warren-core PASS
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` warren-core PASS
- ✅ `cargo test --workspace` warren-app PASS
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` warren-app PASS
- ✅ `cargo fmt --check` warren-core + warren-app PASS
- ✅ `bash scripts/dev/smoke-build.sh` PASS (26/26)
- ✅ Pas de régression Linux/Mac/Win : connect/disconnect fonctionnel
- ✅ Multi-hop M4.E.D + obfuscation M4.0 + NAT-PMP M4.H.F + bypass-cidr M4.H.G inchangés
- ✅ Working tree warren-core inchangé sur `d3_allowlist_dynamic.rs`
- ✅ Rapport `.planning/session-b-report.md` rédigé

Verdict GO PARTIEL acceptable si :
- B.1 DAITA bench KO (overhead > 20%) → "GO code, tuning machine spec pending"
- B.2 failover endpoint backend pas wired (warren-api modification) → "GO client-side, backend stub TODO"
- Skip explicite documenté

Verdict NO-GO uniquement si fix prouvé impossible.

---

## 6. Doctrine

- §0.0 INVIOLABLE git : ZÉRO destructive command
- §0.5 autonomy : pas d'escalade timide
- English-only code comments
- Pas em-dash
- Pas secrets in commits
- TDD strict warren-core
- 5 concurrents comparison standard
- Pas Cure53 mention
- `hcloud --context warren` exclusif
- Push warren-core + warren-app au fil de l'eau

---

## 7. Memory updates attendus à la fin

L'agent doit ajouter à `/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/` :

- `warren_session_b_delivered.md`, verdict global + caveats par sous-phase
- Update `MEMORY.md` index

Et côté warren-core memory si nouveaux commits :

- `warren_daita_integration_v1.md` (spec machine + overhead bench result)
- `warren_failover_doctrine_v1.md` (selector strategy + threshold)

---

## 8. Commencer maintenant

Lis ce brief en entier, puis lis les sources §3 en parallèle, puis attaque B.1.1. Ne demande pas confirmation pour démarrer, ne propose pas de plan d'exécution préalable, exécute directement. Tu as plein mandat §0.5. Pousse warren-core + warren-app au fil de l'eau.

DAITA + multi-hop + obfuscation = trinité privacy Warren. Avec ces 3 livrés, Warren a un narratif produit complet vs Mullvad/ProtonVPN/IVPN/AirVPN/Obscura : full-QUIC + HPKE multi-hop + DAITA + obfuscation HTTP/3 mimicry + port-forwarding NAT-PMP, dans un seul produit. Pas de bluff possible.

Bonne route.
