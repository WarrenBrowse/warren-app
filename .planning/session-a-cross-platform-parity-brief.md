# Session A — Cross-platform parity (macOS + Windows) + Auto-update + Pinning pubkey exit

> Brief d'agent autonome cross-repo warren-core + warren-app.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> Grosse session multi-phases : l'agent enchaîne A.1 → A.2 → A.3 → A.4 sans escalade.

**Effort estimé** : wall-clock 8-10 jours (4 sous-phases).
**Coût Hetzner** : 0 EUR (tests unitaires + intégration suffisent, pas de bench cross-DC).
**Pré-conditions** :
- warren-app `main` HEAD `583581dae5+` (post-M4.H.G + wip wireguard-go submodule)
- warren-core `main` HEAD `478a5f5+` (post-audit dedae7a)
- ⚠️ working tree warren-core a 1 fichier modified non committé :
  `crates/warren-tunnel/tests/d3_allowlist_dynamic.rs`. **Préserver, ne pas toucher.**

**Objectif** : amener Warren VPN à la parité fonctionnelle Linux/macOS/Windows pour la bêta multi-plateforme, plus deux features sécurité hardening (auto-update prod-grade + pinning pubkey exit anti-MITM).

Sous-phases (séquentielles autonomes) :

1. **A.1 — macOS daemon wiring + smoke E2E** (~2-3j)
2. **A.2 — Windows daemon wiring + smoke E2E** (~3-5j)
3. **A.3 — Auto-update mechanism prod-grade** (~2-3j)
4. **A.4 — Pinning pubkey exit client-side (TOFU + UI warning)** (~2-3j)

---

## 0.0 INVIOLABLE — pas de commande git destructive

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

**Pré-existant à préserver** : `warren-core/crates/warren-tunnel/tests/d3_allowlist_dynamic.rs` est modified non-committé (wip poka). Ne pas toucher au contenu, ne pas committer. Si tu touches ce fichier comme effet de bord, escalade.

Violation = scope error CRITIQUE. Incident M4.H.F 2026-05-20 : agent autonome a exécuté `git checkout a7159d94 -- .` warren-core, 5 fichiers WIP poka perdus.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat pour atteindre GO. Diagnostic 30 min → fix tactique TDD → commit + push → reprise. PAS de rollback, PAS d'escalade timide.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak (clé privée, mnemonic, API token)
2. Coût > 0.30 EUR (n/a, pas de Hetzner ici)
3. Breaking change /v1 wire format
4. Signing key prod touchée
5. **Spécifique session A** : si tu découvres qu'une approche A.x nécessite un design upstream (modif majeure Mullvad upstream qui casserait la stratégie rebase), escalade avant push

Décisions tactiques agent autorisées :
- Format de l'auto-update channel (`stable`/`beta` vs single channel)
- TOFU pubkey storage path (sqlite warren-tunnel cache vs config file)
- UI placement du warning pubkey-changed (banner global vs modal)
- macOS launch daemon vs LaunchAgent vs login item
- Windows service start mode (auto vs delayed-auto vs manual+autostart UI)

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # main HEAD 583581dae5+ (wip wireguard-go bump)
git remote -v                                # origin = github.com/WarrenBrowse/warren-app
git fetch origin && git log --oneline origin/main..HEAD || true
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # 1 file modified d3_allowlist_dynamic.rs — préserver
git log --oneline -3                        # HEAD 478a5f5+ (audit dedae7a wired)
```

Si HEAD inattendu : escalade (cf. §0.0, surtout pas de checkout).

---

## 2. Optimisations agent

- Read sources cross-repo en PARALLÈLE
- Tests TDD groupés en fin de sous-tâche (pas après chaque edit)
- Push warren-core + warren-app au fil de l'eau (ne pas accumuler 10+ commits avant push)
- `cargo check` redondant si `cargo test` ou `cargo clippy` suit immédiatement
- Pin warren-core `.warren-core-version` bump uniquement quand un commit warren-core est requis par warren-app (sinon ne pas bumper)

---

## A.1 — macOS daemon wiring + smoke E2E (~2-3j)

### Contexte

Le port macOS `default_route_split_macos.rs` (423 LOC warren-core) est implémenté avec strategy route-specificity (host-route exit-IP exception + 0.0.0.0/1 + 128.0.0.0/1 via tun). Tests unitaires présents. **Pas encore exercé en daemon réel Mac.**

Mullvad upstream macOS : `mullvad-daemon/src/macos.rs` + `mullvad-daemon/src/macos_launch_daemon.rs` + `talpid-core/src/{firewall,split_tunnel,offline}/macos.rs`. Toute la machinerie OS-level (pf killswitch, launch daemon, exclude apps) est déjà là côté Mullvad.

### Scope A.1

1. **A.1.1 — Wire `talpid-warren-tunnel` macOS** : compile + connecte sur Mac réel (ou VM macOS si Mac physique inaccessible). Le tunnel doit utiliser `default_route_split_macos::DefaultRouteSplitGuard::install` quand `bypass_cidrs` est vide + exit_ip set. Conditional compilation `#[cfg(target_os = "macos")]` pattern existant Linux.

2. **A.1.2 — pfctl killswitch** : vérifier que `talpid-core/src/firewall/macos.rs` (existant Mullvad) coexiste correctement avec `default_route_split_macos`. Le pfctl killswitch doit autoriser explicitement le socket Warren client → exit (port 7000 par défaut, ou le port négocié). Pattern Mullvad existant pour WireGuard à adapter.

3. **A.1.3 — Smoke E2E Mac** :
   - `cargo build --release` warren-app sur Mac OK
   - `mullvad-daemon` launch via `launchctl` OK (LaunchDaemon plist Warren-branded)
   - UI Electron connect/disconnect sur exit FR Hetzner prod (warren-exit-1)
   - DNS leak test : `dig @1.1.1.1 example.com` doit passer via tunnel (push-dns wire)
   - WebRTC leak test : `https://browserleaks.com/webrtc` doit montrer IP exit
   - `curl ifconfig.me` doit retourner IP exit, pas IP host
   - SSH inbound préservé via `--bypass-cidr 192.168.0.0/16` (M4.H.G feature)
   - NAT-PMP qBittorrent : ouvrir port, telnet depuis l'extérieur (vérifie M4.H.F)
   - Suspend/resume Mac : connexion auto-reconnect (vérifie M4.E.D)

4. **A.1.4 — Tests TDD A.1** :
   - Test intégration `talpid-warren-tunnel` Mac : install/uninstall DefaultRouteSplitGuard sans panic
   - Smoke unit test `mullvad-daemon/src/macos.rs` : pas de régression Mullvad upstream
   - Si nouveau code Warren-specific Mac path, tests `#[cfg(target_os = "macos")]` requis

### Critères GO A.1

- Build natif Mac PASS
- Connect/disconnect Mac UI fonctionnel
- 6/6 smoke tests passent
- `cargo test --workspace` PASS + `cargo clippy --workspace --all-targets -- -D warnings` PASS
- Documenter résultats smoke dans `.planning/session-a-report.md` §A.1

### Décisions tactiques A.1

- Si Mac physique pas accessible et VM macOS pas dispo localement : skip smoke E2E, marquer A.1 comme "code path complet + tests unitaires PASS + smoke à valider poka manuel" et continue A.2
- LaunchDaemon plist Warren-branded : copier pattern Mullvad `dist-assets/mullvad-daemon.plist` → `warren-vpn-daemon.plist` avec rebrand (BundleIdentifier `com.warrenbrowse.vpn-daemon`)

---

## A.2 — Windows daemon wiring + smoke E2E (~3-5j)

### Contexte

Le port Windows `default_route_split_windows.rs` (446 LOC warren-core) est implémenté avec PowerShell `New-NetRoute` strategy. Tests unitaires présents. **Pas encore exercé en daemon réel Windows.**

Mullvad upstream Windows : `mullvad-daemon/src/*` (cross-OS) + `talpid-core/src/{firewall,split_tunnel,offline}/windows/`. WinTUN driver + WFP killswitch + Windows service infrastructure déjà présents.

### Scope A.2

1. **A.2.1 — Wire `talpid-warren-tunnel` Windows** : compile + connecte sur Windows réel (VM Windows si pas de machine physique dispo). Tunnel utilise `default_route_split_windows::DefaultRouteSplitGuard::install` quand approprié. Conditional compilation `#[cfg(target_os = "windows")]`.

2. **A.2.2 — WinTUN driver integration** : vérifier que le driver WinTUN signé (présent Mullvad upstream) crée bien l'interface tun avant que `default_route_split_windows` n'essaie de pointer `0.0.0.0/1` dessus. Race condition possible : driver loading async + route install. Pattern Mullvad gère déjà ça pour WireGuard, adapter pour Warren.

3. **A.2.3 — WFP killswitch** : `talpid-core/src/firewall/windows/` existant Mullvad doit coexister avec les routes Warren. WFP filtre par binaire (Warren client.exe → exit IP autorisé, tout autre process bloqué hors tunnel).

4. **A.2.4 — Windows service** : `mullvad-daemon` doit s'installer comme service Windows (`sc create`) avec start mode Auto. Pattern Mullvad existant → rebrand `warren-vpn-daemon` (cf. M4.H.D rebrand WARREN_CSC_*).

5. **A.2.5 — Smoke E2E Windows** :
   - `cargo build --release --target x86_64-pc-windows-msvc` (cross OU build Windows natif) PASS
   - Installer NSIS Warren-branded (M4.H.D pipeline) génère MSI/EXE sur Windows VM
   - Service Warren install + start OK
   - UI Electron connect/disconnect Windows
   - DNS leak test : Windows nslookup
   - WebRTC leak test
   - `curl ifconfig.me` retourne IP exit
   - SSH inbound préservé `--bypass-cidr 192.168.0.0/16`
   - Suspend/resume Windows : connexion auto-reconnect
   - Service auto-start après reboot Windows
   - **PAS de NAT-PMP test Windows** (UPnP est l'alternative Windows, hors scope ce brief — différer M5)

6. **A.2.6 — Tests TDD A.2** :
   - Test intégration `talpid-warren-tunnel` Windows : install/uninstall DefaultRouteSplitGuard sans panic
   - Smoke unit test no-regression Mullvad upstream Windows path
   - Tests `#[cfg(target_os = "windows")]` pour code Warren-specific Windows

### Critères GO A.2

- Build Windows PASS (cross ou natif)
- Connect/disconnect Windows UI fonctionnel
- 8/8 smoke tests passent (NAT-PMP exclu)
- Service Windows install/start/stop OK
- `cargo test --workspace` PASS + clippy strict PASS
- Documenter résultats smoke dans `.planning/session-a-report.md` §A.2

### Décisions tactiques A.2

- Si pas de VM Windows dispo localement : skip smoke E2E, marquer "code path + tests unitaires PASS + smoke à valider poka", continue A.3
- Cross-compile vs natif : essayer cross-compile depuis Linux d'abord (`cargo build --target x86_64-pc-windows-msvc` avec `xwin` crate), si bloque sur deps natives → escalade poka
- WinTUN driver version : utiliser celui pinned Mullvad upstream (`mullvad-vpn-monorepo/wintun/`), pas de bump

---

## A.3 — Auto-update mechanism prod-grade (~2-3j)

### Contexte

Mullvad upstream a déjà un crate `mullvad-update` complet (présent warren-app, dépendance dans `mullvad-daemon/Cargo.toml`). Code path : version check periodic + signature verification ed25519 + delta download + UI banner update-available. Tout est là, **pas encore wired pour pointer vers releases Warren**.

### Scope A.3

1. **A.3.1 — URL update server Warren** : configurer `mullvad-update` pour pointer vers `https://updates.warrenbrowse.com/` (ou GitHub Releases API `https://api.github.com/repos/WarrenBrowse/warren-app/releases` selon décision tactique agent). Pattern Mullvad : `MULLVAD_API_URL` env var équivalent → `WARREN_UPDATE_URL` env var.

2. **A.3.2 — Signature verification** : `mullvad-update` vérifie signature Ed25519 sur version manifest. Régénérer ou réutiliser la signing key Warren existante (M4.H.D `WARREN_CSC_*` ne touche PAS le canal updates, c'est une key séparée pour le manifest). Si key updates Warren pas encore générée → escalade poka (case 4 escalation: signing key prod).

3. **A.3.3 — Version manifest format** : `mullvad-update` consomme un JSON `{ "version": "1.0.0-beta.1", "url": "...", "signature": "..." }`. Adapter pour structure Warren-side (GitHub Releases vs self-hosted CDN).

4. **A.3.4 — UI banner** : composant Electron `UpdateBanner.tsx` (présent upstream Mullvad) doit afficher Warren-branded copy ("Warren VPN 1.0.1 available" → CTA "Update now" / "Later"). i18n FR + EN. Skip si pas de release available.

5. **A.3.5 — Channel beta vs stable** : décision tactique. Recommandation = single channel `beta` jusqu'à 1.0 stable, puis stable + beta. Document dans `.planning/session-a-report.md` §A.3.

6. **A.3.6 — Tests TDD A.3** :
   - Mock update server returns version manifest valide → UI banner appears
   - Manifest signature invalide → no banner + log warning
   - Network error → no banner, retry exponential backoff
   - Version manifest version < current → no banner

### Critères GO A.3

- `mullvad-update` pointe vers Warren update server (env var ou config)
- Signature verification PASS sur manifest test
- UI banner Warren-branded i18n FR+EN
- Tests `mullvad-update` unit + integration PASS
- Documenter résultats + URL update server choisi + signing key status dans report §A.3

### Décisions tactiques A.3

- GitHub Releases API vs self-hosted CDN : GitHub Releases (gratuit, déjà setup M4.H.D, pas d'infra add) sauf si bloqué par signing manifest distinct
- Channel : single `beta` jusqu'à 1.0
- Frequency check : 6h (vs 24h Mullvad) pour bêta active (toggleable post-1.0)

---

## A.4 — Pinning pubkey exit client-side (TOFU + UI warning) (~2-3j)

### Contexte

Audit H.E.5/6/7 warren-core a identifié : client doit pinner pubkey exit après first connect (TOFU pattern Trust On First Use) pour détecter MITM/exit substitution malveillante en cas de compromis backend. Mullvad fait ça pour WireGuard public key (~/.config/mullvad-vpn/account-history.json). Warren doit faire pareil pour pubkey Ed25519 exit + warning UI si pubkey change.

### Scope A.4

1. **A.4.1 — TOFU pinning storage** : ajouter persistance pubkey exit pinned dans cache warren-tunnel (sqlite ou config file). Schema : `{ exit_id, pubkey_ed25519, first_seen_unix, last_seen_unix }`. Décision tactique agent : sqlite (warren-tunnel a déjà sqlite via warren-api-client) vs flat config (JSON dans config dir Warren).

2. **A.4.2 — Verification on connect** : à chaque connect, comparer pubkey exit reçue (signed handshake) avec pubkey pinned. Si match → OK silencieux. Si pas pinned → store + OK silencieux (TOFU). **Si mismatch → REFUSE connect + emit event UI**.

3. **A.4.3 — UI warning** : composant `WarrenPubKeyWarning.tsx` Electron + i18n FR+EN. Affiche modal warning "L'identité du serveur Warren a changé. Cela peut indiquer une attaque ou une mise à jour légitime du serveur." + 3 CTA : "Trust new key (continue)" / "Reject (disconnect)" / "Report to Warren". `WarrenPubKeyLabel.tsx` existant à étendre.

4. **A.4.4 — Override mechanism** : si user "Trust new key" → unpin ancienne + pin nouvelle. Logger l'event pour forensics. Si "Report" → POST `/v1/incidents/pubkey-mismatch` warren-api (endpoint à ajouter, simple log côté backend).

5. **A.4.5 — Settings reset** : Settings → "Reset pinned exit keys" CTA pour user qui veut clear toutes les pinning (changement de wallet, etc.). Confirmation modal.

6. **A.4.6 — Tests TDD A.4** :
   - First connect → pubkey pinned, no warning
   - Re-connect same exit → match, no warning
   - Connect different exit → new pubkey pinned (per exit_id distinct)
   - Mismatch same exit_id → connect refused + event fired
   - Trust new key flow → pinning updated
   - Reset pinned keys → all entries cleared

### Critères GO A.4

- TOFU pinning storage opérationnel (sqlite ou config)
- Mismatch detection refuse connect par défaut
- UI warning modal Warren-branded i18n FR+EN
- Override + Reset flows fonctionnels
- Endpoint `/v1/incidents/pubkey-mismatch` warren-api stub (log only, non-bloquant)
- Tests TDD 6/6 PASS
- Documenter résultats + schema storage choisi dans report §A.4

### Décisions tactiques A.4

- Storage : sqlite warren-tunnel (réutilise infra existante)
- Endpoint /v1/incidents : POST simple, log seulement (pas de DB), retour 200. Pas de PII pour user, juste pubkey old/new + exit_id + timestamp
- Per exit_id pinning vs per exit_pubkey : per exit_id (un exit peut renouveler sa clé legitimement, l'ID stable du backend reste pinning anchor)

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-core
- `crates/warren-client/src/default_route_split_macos.rs` (port Mac existant)
- `crates/warren-client/src/default_route_split_windows.rs` (port Win existant)
- `crates/warren-client/src/default_route_split.rs` (Linux ref + dispatch)
- `crates/warren-tunnel/src/client.rs` (handshake reception)
- `crates/warren-tunnel/src/allowlist.rs` (cf. dirty file `d3_allowlist_dynamic.rs` test — préserver)
- `crates/warren-tunnel/src/exit.rs` (exit state + pubkey reception)

### warren-app
- `crates/talpid-warren-tunnel/src/lib.rs` (adapter daemon ↔ warren-core)
- `mullvad-daemon/src/macos.rs` + `macos_launch_daemon.rs`
- `mullvad-daemon/src/lib.rs` (state machine cross-OS)
- `talpid-core/src/{firewall,split_tunnel,offline}/macos.rs` (Mac OS-level)
- `talpid-core/src/{firewall,split_tunnel,offline}/windows/` (Win OS-level)
- `mullvad-update/` (auto-update crate)
- `desktop/packages/mullvad-vpn/src/renderer/components/WarrenPubKeyLabel.tsx`
- `desktop/packages/mullvad-vpn/src/renderer/features/warren-multi-hop/` (pattern UI)
- `dist-assets/mullvad-daemon.plist` (LaunchDaemon Mac template)
- `dist-assets/binaries/` (WinTUN driver)

### Documentation Mullvad upstream (refs)
- `docs/architecture.md` daemon state machine
- `docs/development.md` Mac/Win build instructions
- `installer/macos/` packaging Mac
- `installer/windows/` packaging Win

---

## 4. Plan d'exécution (séquentiel, autonome)

```
A.1 macOS (2-3j)
  ├── A.1.1 wire talpid-warren-tunnel Mac
  ├── A.1.2 pfctl killswitch coexist
  ├── A.1.3 smoke E2E Mac (ou skip si pas dispo)
  └── A.1.4 tests TDD Mac + commit + push
A.2 Windows (3-5j)
  ├── A.2.1 wire talpid-warren-tunnel Win
  ├── A.2.2 WinTUN driver integration
  ├── A.2.3 WFP killswitch coexist
  ├── A.2.4 Windows service install
  ├── A.2.5 smoke E2E Win (ou skip si pas dispo)
  └── A.2.6 tests TDD Win + commit + push
A.3 Auto-update (2-3j)
  ├── A.3.1 URL update server
  ├── A.3.2 signature verification
  ├── A.3.3 manifest format
  ├── A.3.4 UI banner i18n
  ├── A.3.5 channel choice
  └── A.3.6 tests TDD + commit + push
A.4 Pinning pubkey (2-3j)
  ├── A.4.1 TOFU storage
  ├── A.4.2 verification on connect
  ├── A.4.3 UI warning modal
  ├── A.4.4 override mechanism
  ├── A.4.5 settings reset
  └── A.4.6 tests TDD + commit + push
A.5 Rapport final (1h)
  └── .planning/session-a-report.md avec verdict GO ULTIMATE par sous-phase
```

Push warren-core (si modif) + warren-app (toujours) au fil de l'eau. Bump pin `.warren-core-version` warren-app si nouveaux commits warren-core requis.

---

## 5. Critères GO ULTIMATE session A

Tous les critères suivants doivent passer pour verdict GO ULTIMATE complet :

- ✅ A.1 critères GO PASS
- ✅ A.2 critères GO PASS
- ✅ A.3 critères GO PASS
- ✅ A.4 critères GO PASS
- ✅ `cargo test --workspace` warren-core PASS
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` warren-core PASS
- ✅ `cargo test --workspace` warren-app PASS
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` warren-app PASS
- ✅ `cargo fmt --check` warren-core PASS
- ✅ `cargo fmt --check` warren-app PASS
- ✅ `bash scripts/dev/smoke-build.sh` PASS (26/26)
- ✅ Pas de régression Linux : connect/disconnect Linux toujours OK
- ✅ Working tree warren-core inchangé sur `d3_allowlist_dynamic.rs`
- ✅ Rapport `.planning/session-a-report.md` rédigé avec sections par sous-phase + verdict + caveats

Verdict GO PARTIEL acceptable si :
- A.1 Mac smoke skipped (pas de Mac dispo), tests unitaires + code PASS → "GO code, smoke pending poka"
- A.2 Windows smoke skipped (pas de Win dispo), idem
- Skip explicite documenté dans report

Verdict NO-GO uniquement si fix prouvé impossible (rare, après §0.5 autonomy exhausted).

---

## 6. Doctrine

- §0.0 INVIOLABLE git : ZÉRO destructive command
- §0.5 autonomy : pas d'escalade timide, fix tactique + push
- English-only code comments (memory `feedback_english_only_comments`)
- Pas em-dash (memory `feedback_no_em_dash`)
- Pas secrets in commits (memory `feedback_warren_no_secrets_in_commits`)
- TDD strict warren-core (RED → GREEN → REFACTOR)
- 5 concurrents comparison standard quand pertinent (Mullvad/ProtonVPN/AirVPN/IVPN/Obscura)
- Pas Cure53 mention
- `hcloud --context warren` exclusif si Hetzner touché
- Push warren-core + warren-app au fil de l'eau

---

## 7. Memory updates attendus à la fin

L'agent doit ajouter à `/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/` :

- `warren_session_a_delivered.md` — verdict global + caveats par sous-phase
- Update `MEMORY.md` index

Et côté `/Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-core/memory/` si nouveaux commits warren-core :

- Memory dédié si feature notable (ex: `warren_pubkey_pinning_storage.md` si schema sqlite non-trivial)

---

## 8. Commencer maintenant

Lis ce brief en entier, puis lis les sources §3 en parallèle, puis attaque A.1.1. Ne demande pas confirmation pour démarrer, ne propose pas de plan d'exécution préalable — exécute directement. Tu as plein mandat §0.5. Pousse warren-core + warren-app au fil de l'eau, ne batch pas tout en fin de session.

Bonne route.
