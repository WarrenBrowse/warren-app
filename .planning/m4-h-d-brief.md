# Phase M4.H.D - Migration GitHub + Build pipeline DMG/AppImage/MSI + Signing + CI

> Brief d'agent autonome warren-app. Doctrine §0.5 full autonomy NO
> timid rollback. Mission : 2 chantiers couplés en une phase.
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 4-7 jours.
**Coût Hetzner** : 0 EUR pour packaging + CI setup. Optionnel ~0.10
EUR si bench installer end-to-end inclus (conditionné caveat SSH
Hetzner résolu).
**Pré-condition** :
- warren-app main HEAD post-M4.H.E (~`b34f29c74e` ou descendant)
- Stack M4.E.D câblée + UI complète + caveats fixés
- `gh` CLI installé, accounts `kilianc411` + `poka-IT` loggés (vérifié
  par orchestrateur 2026-05-20)
- warren-core hébergé sur `github.com/WarrenBrowse/warren-core` (ref)
- poka fournira signing assets (`.p12` macOS + `.pfx` Windows +
  notarytool credentials Apple) **AVANT M4.H.D.5** sinon escalade

**Objectif** :
1. **Chantier A (pré-phase)** : migrer le hosting warren-app de Gitea
   (`git.p2p.legal/warren/warren-app`) vers GitHub
   (`github.com/WarrenBrowse/warren-app`) avec gh CLI user `poka-IT`.
   Préserver branches + tags + upstream Mullvad. Update `origin` remote
   local.
2. **Chantier B (build pipeline)** : adapter le `build.sh` Mullvad
   upstream + scripts existants pour produire des installers Warren
   signés (DMG macOS + .deb/.rpm Linux + MSI Windows). Adapter les
   GHA workflows pour CI release Warren. Setup signing keys.

---

## 0. MANDAT STRICT

Anti-patterns historiques M4.E §7 + TDD warren-core CLAUDE.md §1
(côté scripts shell : tests in `building/` ou `scripts/` si applicable).
/v1 constantes IMMUABLES. Pas de breaking change sur le wire format ou
les API daemon-side (M4.H.D ne touche QUE packaging + hosting).

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein
mandat. Diagnostic 30 min → fix tactique → commit + push → reprise.

Escalade `AskUserQuestion` poka SEULEMENT si :
1. Secret leak découvert
2. Coût Hetzner > 0.30 EUR (n/a si bench installer skipped)
3. Breaking change /v1 wire format
4. Signing key prod doit être touchée
5. **Spécifique M4.H.D** : décision business sur hosting (org
   GitHub `WarrenBrowse` vs alternative), signing assets manquants
   (poka doit fournir `.p12`, `.pfx`, notarytool credentials avant
   M4.H.D.5)

Verdict NO-GO seulement si fix prouvé impossible après 4h investigation.

Décisions tactiques que tu peux prendre seul :
- Structure CI workflows (matrix builds, runners self-hosted vs
  GitHub-hosted, cache strategy)
- Format packages Linux (.deb + .rpm vs AppImage seul vs all)
- Versioning scheme installer (calver `2026.5.0` ou semver `v0.1.0`)
- Choix de `cosign` vs `gpg` pour signing artifacts release
- Renommage scripts existants si nécessaire (`build.sh` → reste
  `build.sh`, juste adapter contenu)

---

## 1. Optimisations agent

- Lectures sources cross-repo en PARALLÈLE en début de phase
- Push origin/main warren-app au fur et à mesure (10+ commits attendus
  vu le scope)
- Tests packaging en LOCAL d'abord (macOS DMG produit local), CI plus
  tard (CI = GHA donc nécessite migration GitHub faite)

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main post-M4.H.E
git log --oneline -3
git remote -v                                # origin = Gitea actuellement
gh auth status                               # 2 accounts kilianc411 + poka-IT
                                             # actif = kilianc411
ls scripts/dev/ 2>/dev/null
node --version && volta list 2>/dev/null
docker --version || podman --version         # gRPC bindings codegen
```

Si pré-requis tooling manquent : escalade.

---

## 3. Sources à lire (PARALLÈLE)

### warren-app build infra existante (Mullvad upstream)

- `build.sh` (531 lignes orchestrateur cross-platform)
- `BuildInstructions.md` (deps système + Volta)
- `Release.md` (process release Mullvad)
- `building/Dockerfile` + `building/containerized-build.sh`
- `installer-downloader/` (loader app upstream)
- `mullvad-nsis/Cargo.toml` + `src/` (Windows installer)
- `dist-assets/pkg-scripts/postinstall` + `preinstall` (macOS)
- `dist-assets/linux/warren-daemon.service` + `apparmor_warren`
  (Linux units, déjà Warren-rebrand R1)
- `.github/workflows/daemon.yml` (CI daemon)
- `.github/workflows/frontend.yml` (CI desktop frontend)
- `.github/workflows/desktop-e2e.yml` (CI Electron E2E)
- `.github/workflows/clippy.yml` (CI lint Rust)

### warren-core référence

- `github.com/WarrenBrowse/warren-core` → vérifier la structure de
  release + CI workflows (si existant)
- Comparer pour cohérence Warren branding sur les 2 repos

### Memory cross-session

- `project_warren_architecture` warren-app (rebrand R1 binary names :
  `warren` / `warren-daemon` via `[[bin]]`, paths système
  `/etc/warren-vpn/`)
- `warren_m4h_e_delivered.md` warren-app (état post-M4.H.E)
- `project_warren_app_state_post_m4hc.md` (state-of-truth orchestrateur,
  ou plus récent post-m4he si créé)
- `feedback_agent_full_autonomy_no_timid_rollback.md`
- `feedback_warren_phase_prompts_no_branch.md`

---

## 4. Plan d'exécution

### CHANTIER A : Migration GitHub (~0.5-1 jour)

#### M4.H.D.A.0 - Pre-flight + verify ownership

1. `gh auth switch --user poka-IT`
2. `gh auth status` : vérifier active account = poka-IT
3. `gh repo list WarrenBrowse --limit 20` : vérifier que poka-IT a
   accès en write à l'org `WarrenBrowse` (warren-core devrait être
   listé)
4. Si poka-IT manque permissions sur WarrenBrowse : escalade poka

#### M4.H.D.A.1 - Create GitHub repo + initial push

1. `gh repo create WarrenBrowse/warren-app --private --description
   "Warren VPN desktop app (fork Mullvad VPN)" --license GPL-3.0`
   (license matches Mullvad upstream)
2. NB : `--private` par défaut (cf. UPSTREAM_BASELINE.md décision
   2026-05-06 : repo privé pendant POC, public au lancement freemium
   GPL-3.0 oblige)
3. Add remote :
   ```bash
   git remote rename origin gitea
   git remote add github git@github.com:WarrenBrowse/warren-app.git
   ```
4. Push complet :
   ```bash
   git push -u github main
   git push github warren-base
   git push github warren-base-phase1a
   git push github --tags
   ```
5. Vérifier `gh repo view WarrenBrowse/warren-app --web` (URL accessible)
6. **NE PAS supprimer le remote Gitea encore** : peut servir pour
   miroir/backup temporaire.

#### M4.H.D.A.2 - Switch origin to GitHub

1. `git remote rename gitea backup-gitea` (préserver pour traçabilité)
2. `git remote rename github origin`
3. `git remote set-url --push backup-gitea no_push` (lock writes Gitea)
4. Vérifier `git remote -v` :
   ```
   origin           git@github.com:WarrenBrowse/warren-app.git
   backup-gitea     ssh://git@git.p2p.legal:10122/warren/warren-app.git
                    (fetch only)
   upstream         https://github.com/mullvad/mullvadvpn-app
   ```
5. `git push origin main` (verify push works post-switch)
6. Commit `chore(infra): migrate hosting Gitea → github.com/WarrenBrowse/warren-app`

#### M4.H.D.A.3 - Switch back gh CLI to kilianc411 si convention

`gh auth switch --user kilianc411` si c'est le user dev habituel.

Doctrine : poka-IT pour ops GitHub admin (créer repo, manage org),
kilianc411 pour dev quotidien.

### CHANTIER B : Build pipeline (~3-6 jours)

#### M4.H.D.0 - Audit build.sh + scripts existants

1. Lire `build.sh` + `building/containerized-build.sh` (Mullvad
   upstream)
2. Identifier les hooks Warren à adapter :
   - Variable `PRODUCT_NAME` (Mullvad VPN → Warren VPN)
   - Variable `MULLVAD_*` à parallel `WARREN_*` (R1 cohérence)
   - Paths `/Library/Application Support/Mullvad VPN/` →
     `/Library/Application Support/Warren VPN/`
   - Bundle ID macOS `net.mullvad.vpn` → `com.warrenbrowse.vpn`
   - GUID Windows installer
3. Documenter inventory dans `/tmp/m4-h-d-build-audit.md`

#### M4.H.D.1 - Adapter build.sh pour Warren

1. Rebrand strings + paths dans `build.sh`
2. Tester localement `./build.sh --dev-build` (sans signing)
3. Vérifier artefacts produits dans `dist/` (DMG ou .deb selon
   platform locale agent)
4. TDD shell : ajouter un script smoke `scripts/dev/smoke-build.sh`
   qui assert que `./build.sh --dev-build` produit bien des artefacts
   nommés `Warren VPN*` (pas `Mullvad VPN*`)
5. Commit `feat(build): adapt build.sh for Warren branding`

#### M4.H.D.2 - DMG macOS packaging

1. Adapter `dist-assets/pkg-scripts/postinstall` + `preinstall` pour
   Warren paths
2. `desktop/packages/mullvad-vpn/scripts/build-mac.js` (à identifier,
   probable existant) → renommage productName
3. Tester localement `./build.sh` sans signing → produit
   `dist/Warren VPN.dmg`
4. Smoke : `hdiutil verify dist/Warren VPN.dmg` PASS
5. Commit `feat(build): produce signed DMG with Warren branding`

#### M4.H.D.3 - .deb / .rpm / AppImage Linux

1. Build script Linux : `dist-assets/linux/` (warren-daemon.service +
   apparmor_warren déjà OK R1)
2. Vérifier que `build.sh` Linux produit `.deb` + `.rpm` Warren
3. Optionnel : AppImage via `appimage-builder` si dispo Mullvad
4. Smoke : `dpkg -I dist/Warren-VPN-*.deb` montre métadonnées Warren
5. Commit `feat(build): produce .deb + .rpm Warren-branded`

#### M4.H.D.4 - MSI Windows via mullvad-nsis

1. Lire `mullvad-nsis/src/` pour identifier les strings hardcodées
   Mullvad
2. Rebrand : product name, install path `C:\Program Files\Warren VPN\`,
   uninstall key, scheduled tasks names
3. Build local Windows (ou cross-build via container si supporté)
4. Smoke : si dev env permet, install + uninstall MSI test
5. Commit `feat(build): produce signed MSI with Warren branding via nsis`

#### M4.H.D.5 - Signing setup (escalade poka pour assets)

**Pre-condition** : poka fournit
- `WARREN_CSC_LINK_MACOS=/path/to/Warren-Developer-ID.p12`
- `WARREN_CSC_KEY_PASSWORD_MACOS` (via keyring/env, jamais en clair)
- `WARREN_CSC_LINK_WIN=/path/to/Warren-Codesign.pfx`
- `WARREN_CSC_KEY_PASSWORD_WIN`
- `WARREN_NOTARIZE_KEYCHAIN` + `WARREN_NOTARIZE_KEYCHAIN_PROFILE`
  (Apple notarytool credentials)

Si poka n'a pas fourni avant M4.H.D.5 : escalade `AskUserQuestion`
avec timeline estimée (1 jour) + documentation pour générer les
certificats (Apple Developer Program + Windows code signing CA).

1. Adapter `build.sh` pour consommer les `WARREN_CSC_*` env vars
   parallel aux `CSC_*` Mullvad upstream (backward-compat)
2. Test signing local (si keys fournies)
3. Notarization Apple : test pipeline `xcrun notarytool submit dist/Warren-VPN.dmg`
4. Commit `feat(build): wire Warren signing keys + Apple notarization`

#### M4.H.D.6 - CI workflows GHA adapté Warren

1. Lire les 30 workflows `.github/workflows/` upstream Mullvad
2. Identifier les workflows critiques pour Warren :
   - `clippy.yml` : Rust lint
   - `daemon.yml` : build + test daemon
   - `frontend.yml` : Electron lint + tsc + test
   - `desktop-e2e.yml` : E2E si applicable
   - `cargo-vendor.yml` : reproducible build
3. **Renommer ou supprimer les workflows non-Warren** :
   - `android-*.yml` → garder (pour future M4.H mobile)
   - `ios-*.yml` → garder (pour future M4.H mobile)
   - Spécifiques Mullvad (e.g. signature internal) → adapter ou supprimer
4. **Créer workflow `release.yml`** pour M4.H.D :
   - Trigger sur tag `v*.*.*`
   - Matrix : macos-14 (DMG signed + notarized) + ubuntu-22.04
     (.deb + .rpm + AppImage) + windows-2022 (MSI signed)
   - Secrets nécessaires : `WARREN_CSC_*` configurés via GitHub Secrets
     (poka set up GitHub Settings)
   - Upload artifacts to GitHub Release
5. Workflows Warren-spécifiques :
   - `quinn-fork-sync.yml` : verify `.warren-core-version` pin
     matches HEAD warren-core (si possible accessible CI)
6. Commit `ci: adapt GHA workflows for Warren branding + release pipeline`
7. Push, vérifier workflows triggered visible dans
   `gh run list --limit 5`

#### M4.H.D.7 - Release process documentation

1. Recréer `prepare-release.sh` adapté Warren (Mullvad upstream l'a
   apparemment supprimé) :
   - Validate working tree clean
   - Bump `desktop/package.json` version
   - Tag signed `v<VERSION>` (git tag -s)
   - Push tag → triggers release.yml CI
2. Adapter `Release.md` : remplacer `mullvad` → `warren`, env vars
   `CSC_*` → `WARREN_CSC_*`
3. Document signing certs storage + rotation policy
4. Commit `docs(release): Warren release process + prepare-release.sh`

#### M4.H.D.8 - (Optionnel, conditionnel SSH Hetzner résolu) bench installer

Si SSH Hetzner caveat résolu par poka avant M4.H.D.8 :
1. Provision 1× CCX23 fra1 vierge
2. scp DMG / .deb / .exe Warren depuis local + install
3. Smoke : daemon démarre + UI affiche + connect single-hop fonctionne
4. Tear-down

Sinon : skipper avec caveat documenté "bench installer empirique
déféré jusqu'à résolution SSH Hetzner ops".

#### M4.H.D.9 - Finalize + commits + memory

1. Rapport `/tmp/m4-h-d-report.md` ≤ 250 lignes
2. Commits cumulés poussés origin/main (10+ attendus)
3. Memory `warren_m4h_d_delivered.md` warren-app + index MEMORY.md
4. Update source-of-truth orchestrateur

---

## 5. Règles non-négociables

### Sécurité

- **Signing keys** : `.p12`/`.pfx` paths LOCAL (jamais committed),
  passwords via env/keyring (jamais en clair commit), notarytool
  credentials via xcrun keychain (jamais hardcodés)
- Pas de seeds/mnemonics verbatim
- GitHub Secrets pour CI : poka configure via GitHub Settings UI,
  agent référence les noms (`secrets.WARREN_CSC_LINK_MACOS`) sans
  jamais voir le contenu
- `.gitignore` doit exclure tout fichier de certificat local
  (`*.p12`, `*.pfx`, `*.cer`, `.notarytool-creds.json`)

### Code (scripts)

- Bash strict mode `set -eu` (déjà dans build.sh)
- Pas d'em-dash, anglais comments
- Conventional commits subject-only

### Git

- Push main direct warren-app (GitHub après migration)
- Préserver branches `warren-base`, `warren-base-phase1a`, tags upstream
- Préserver `upstream` remote Mullvad pour cherry-pick futur

### Versioning

- Schéma proposé : calver `<YEAR>.<MILESTONE>.<PATCH>` (cohérent
  Mullvad upstream)
- Première release Warren : à décider (probable `2026.5.0` ou
  `0.1.0-beta1`)
- Signed tags via gpg/ssh (git config user.signingkey)

---

## 6. Pas de validation intermédiaire poka

§0.5. 5 cas d'escalade ONLY (cf. 4 standards + signing assets).

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

**Chantier A** :
- Repo `github.com/WarrenBrowse/warren-app` créé private GPL-3.0
- HEAD `main` + branches + tags pushés
- `git remote -v` montre origin GitHub + backup-gitea + upstream
- Commit chore(infra) migrate hosting poussé GitHub

**Chantier B** :
- `build.sh` produit Warren-branded artifacts (DMG + .deb + .rpm
  + MSI) localement
- Signing wired (test fait OU escalade poka acceptée si assets pas
  fournis)
- CI release.yml workflow déclenché sur tag, matrix 3 OS, upload
  artifacts to GitHub Release
- 4 workflows critiques (clippy + daemon + frontend + desktop-e2e)
  pass green sur le repo GitHub
- `prepare-release.sh` recréé adapté Warren
- `Release.md` adapté Warren

**Tous** :
- 10+ commits atomiques poussés origin/main GitHub
- Memory updates
- Documentation à jour (`Release.md`, `BuildInstructions.md` Warren)

### GO CONDITIONAL

- Migration GitHub PASS + build pipeline 80% (ex: 2/3 OS packages
  produits, signing pending poka), caveats documentés

### NO-GO HONNÊTE (improbable §0.5)

- Org `WarrenBrowse` GitHub permissions refusées poka-IT
- Mullvad upstream build.sh casse fundamentalement post-rebrand
  (probable archi refactor large)

---

## 8. Rapport final attendu

`/tmp/m4-h-d-report.md` ≤ 250 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO
2. **Chantier A migration GitHub** : URL repo + commits push résumé
   branches/tags
3. **Chantier B build pipeline** : artifacts produits par OS, taille,
   signing status
4. **CI workflows** : release.yml + workflows critiques status post-
   migration, gh run list excerpt
5. **prepare-release.sh + Release.md** : adapted Warren
6. **Caveats** : signing assets fournis ou pending escalade, bench
   installer empirique fait ou différé, SSH Hetzner persistant
7. **Commits** : list 10+ commits avec subject
8. **Memory updates**

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → débloque M4.H.F (NAT-PMP client câblage +
  différenciateur produit) puis M4.H.G (caveats --bypass-cidr +
  backoff) puis M4.H.H (doc warrenbrowse.com)
- **CONDITIONAL** → pondérer caveats vs ship readiness
- **NO-GO** → analyse cause root before next phase

Caveats persistants post-M4.H.D :
- SSH Hetzner bench (si non-résolu par poka)
- GHCR PAT poka-IT write:packages (impacte CI cosign si pertinent
  pour Warren artifacts release)

Phases futures :
- M4.H.F : NAT-PMP + UI port-forwarding (différenciateur produit)
- M4.H.G : --bypass-cidr + backoff tune
- M4.H.H : doc warrenbrowse.com

---

## 10. Trace de mémorisation

Warren-app :
- Create `warren_m4h_d_delivered.md`
- Update source-of-truth orchestrateur (`project_warren_app_state_
  post_m4hd.md` ou section ajoutée)
- Index MEMORY.md : `- [M4.H.D delivered](warren_m4h_d_delivered.md) — <verdict> migration GitHub + build pipeline + CI release`

Warren-app + warren-core cohérence :
- Vérifier que `.warren-core-version` est correctement référencé par
  CI Warren-app (verify pin sync sur tag release)
