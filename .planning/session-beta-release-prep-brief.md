# Session Beta-Release-Prep — procurement docs + CI verify + tagging procedure

> Brief d'agent autonome warren-app + warren-core + warrenbrowse-site.
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session courte : préparer tout côté code pour que poka n'ait qu'à pousser le tag une fois les certs procurés.

**Effort estimé** : wall-clock 2-3 jours.
**Coût Hetzner** : 0 EUR.
**Pré-conditions** :
- warren-app `main` HEAD `eced6c8613+`
- warren-core `main` HEAD `fed1c88+`
- Phase 5 external-blockers livré (CI scaffold present `.github/workflows/release.yml`)
- docs/24-CODE-SIGNING.md existe (240 lignes procurement OV/EV + Apple Developer)

**Objectif** : finaliser tout le code/docs/CI pour que poka n'ait qu'à (1) procurer les certs externes, (2) push tag `v0.1.0-beta.1`. CI release.yml exécute verify+build+sign+upload TestFlight+Play Store automatic dès secrets en place.

Sous-phases (séquentielles autonomes) :

1. **Beta.1 — Setup worktree** (~30 min)
2. **Beta.2 — Verify CI release.yml secrets matrix + skip-if-no-secrets logic** (~0.5j)
3. **Beta.3 — Procurement guide consolidé docs/RELEASE-PROCUREMENT.md** (~0.5j)
4. **Beta.4 — Pre-tag verify script `scripts/release/verify-beta.sh`** (~0.5j)
5. **Beta.5 — Release notes template + CHANGELOG.md** (~0.5j)
6. **Beta.6 — Tagging procedure runbook docs/RUNBOOK-RELEASE.md** (~0.5j)
7. **Beta.7 — Test dry-run sans tag (workflow_dispatch manuel)** (~0.5j)
8. **Beta.8 — Rapport + cleanup** (~0.5j)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard. Préserver fichiers modified/untracked.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si :
1. Secret leak
2. Coût > 0.30 EUR (n/a)
3. Breaking change CI workflow majeur
4. Signing key prod touchée (n/a, procurement externe poka)
5. **Spécifique Beta** : si tu détectes que la CI release.yml a un bug bloquant (skip-if-no-secrets ne fonctionne pas, build matrix échoue sans secrets), escalader avant tagging

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-beta-release main
cd ../warren-app-beta-release
```

Cleanup en fin :
```bash
git worktree remove ../warren-app-beta-release
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-beta-release main
cd ../warren-app-beta-release

# Read existing CI + docs
cat .github/workflows/release.yml
cat docs/24-CODE-SIGNING.md
cat .planning/session-d7-play-store.md
ls .github/workflows/
```

---

## 2. Beta.2 — Verify CI release.yml secrets matrix + skip-if-no-secrets (~0.5j)

### Scope

1. Auditer `.github/workflows/release.yml` (Phase 5 cdblc4f scaffold) :
   - Matrix targets : `macos-14` (universal arm64+x86_64), `ubuntu-22.04`, `windows-2022`, `iOS` build + archive `.xcarchive`, `android` `.aab`
   - Skip-if-no-secrets logic : chaque sign step doit `if: ${{ secrets.X != '' }}` (non-block release without sign)
   - Secrets requis (refs docs/24) :
     - `WINDOWS_PFX_BASE64` + `WINDOWS_PFX_PASSWORD` (OV/EV cert)
     - `APPLE_DEVELOPER_ID_BASE64` + `APPLE_DEVELOPER_ID_PASSWORD` + `APPLE_NOTARY_USER` + `APPLE_NOTARY_PASSWORD` + `APPLE_TEAM_ID`
     - `IOS_PROVISIONING_PROFILE_BASE64` + `IOS_DISTRIBUTION_CERT_BASE64` + `IOS_DISTRIBUTION_PASSWORD`
     - `ANDROID_KEYSTORE_BASE64` + `ANDROID_KEYSTORE_PASSWORD` + `ANDROID_KEY_ALIAS` + `ANDROID_KEY_PASSWORD`
     - `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` (upload internal-test via API)
     - `APPSTORECONNECT_API_KEY` (TestFlight upload)
2. Si secrets manquants à wirer dans release.yml : ajouter steps + `if:` guards
3. Verify chaque step a un nom descriptif + comment quelle plateforme

### Critères GO

- release.yml liste tous les secrets requis avec comments
- Skip-if-no-secrets sur chaque sign/upload step
- Workflow build PASS dry-run sans secrets (skip sign, archive unsigned binaries)

---

## 3. Beta.3 — Procurement guide consolidé docs/RELEASE-PROCUREMENT.md (~0.5j)

### Scope

Document unique récap de tous les achats/setup poka :

```markdown
# Warren Beta Release — Procurement Guide

## Costs annuels estimés
- Apple Developer Program: 99 USD/an (iOS TestFlight + macOS notarization)
- Windows OV cert: ~280€/an
- Google Play Console: 25 USD one-time
- Hetzner servers: déjà provisionné prod

## Procurement steps
[Détail step-by-step par plateforme + URLs + screenshots procédure]

## Secrets GitHub Actions à wirer
[Liste des secrets avec format base64 + commande génération]

## Verification post-procurement
[Tests pour valider chaque cert installé correctement]
```

Concatène docs/24-CODE-SIGNING.md + session-d7-play-store.md + nouveau content iOS TestFlight.

### Critères GO

- docs/RELEASE-PROCUREMENT.md ~200 lignes complet
- Step-by-step actionnable pour poka
- Verification commands fournis

---

## 4. Beta.4 — Pre-tag verify script `scripts/release/verify-beta.sh` (~0.5j)

### Scope

Script bash + idempotent vérifiant pre-tag :
- `cargo test --workspace` warren-core PASS
- `cargo test --workspace` warren-app PASS
- `cargo clippy --workspace --all-targets -- -D warnings` PASS cross-repo
- `cargo fmt --check` PASS cross-repo
- `cargo deny check` PASS (advisory bincode noted)
- `bash scripts/dev/smoke-build.sh` PASS (26/26)
- iOS `xcodebuild build -scheme WarrenVPN -destination 'platform=iOS Simulator,name=iPhone 16 Pro'` PASS
- Android `./gradlew app:assembleRelease` PASS (dev keystore OK)
- desktop `npm run build` PASS
- Tous les briefs `.planning/session-*-report.md` mentionnent GO ULTIMATE ou GO PARTIAL acceptable
- Pin warren-core `.warren-core-version` correspond à HEAD warren-core
- `git tag --list v0.1.0-beta.*` empty (premier tag)
- Working tree clean both repos

Output : matrix PASS/FAIL + bloquants listés.

### Critères GO

- Script exécutable + idempotent
- Sortie claire bloquants vs warnings

---

## 5. Beta.5 — Release notes template + CHANGELOG.md (~0.5j)

### Scope

1. `CHANGELOG.md` warren-app + warren-core : suivre Keep a Changelog v1.1.0 format
2. Entry initiale `## [0.1.0-beta.1] - 2026-XX-XX`
3. Sections : Added / Changed / Fixed / Security / Removed
4. Liste consolidée des différenciateurs livrés (Multi-hop HPKE + DAITA + Obfuscation M4.0 + NAT-PMP + Multi-exit failover + TOFU pinning + Wallet Ed25519 + bypass-cidr Linux + Sticky multi-hop IPs + DaitaMetrics)
5. Template release notes GitHub Release :
   - Highlights
   - Download links (Linux .deb/.rpm/AppImage + macOS .pkg + Windows .exe + iOS TestFlight + Android Play Store)
   - SHA-256 checksums table
   - Known limitations
   - Bug report URL

### Critères GO

- CHANGELOG.md cross-repo committed
- Release notes template `.github/RELEASE_NOTES.md` template

---

## 6. Beta.6 — Tagging procedure runbook docs/RUNBOOK-RELEASE.md (~0.5j)

### Scope

```markdown
# Warren Release Runbook

## Pre-tag checklist
1. Run scripts/release/verify-beta.sh — must PASS green
2. Update CHANGELOG.md + commit + push
3. Procurement vérifié (secrets in GitHub Actions)
4. Pin warren-core matches HEAD

## Tag + push
```sh
git tag -a v0.1.0-beta.1 -m "Warren VPN beta 1"
git push origin v0.1.0-beta.1
```

## Post-tag
1. GH Actions release.yml runs automatically
2. Monitor https://github.com/WarrenBrowse/warren-app/actions
3. ~30 min later: artifacts uploaded GitHub Releases + TestFlight + Play Store internal
4. Update warrenbrowse-site /download with new release URLs
5. Announce internal-tester group
```

### Critères GO

- Runbook complet step-by-step
- Rollback procedure documentée si fail

---

## 7. Beta.7 — Test dry-run sans tag (workflow_dispatch manuel) (~0.5j)

### Scope

1. Ajouter `workflow_dispatch:` trigger dans `.github/workflows/release.yml` (en plus de `on: push: tags: ...`)
2. Run manual via `gh workflow run release.yml --ref main`
3. Vérifier matrix run PASS sans secrets (skip-if logic)
4. Archive artifacts unsigned uploaded GitHub Actions run
5. Cleanup test run après valider

### Critères GO

- workflow_dispatch trigger ajouté
- Test run dry réussi sans secrets
- Artifacts unsigned générés OK

### Décisions tactiques Beta.7

- Garder workflow_dispatch trigger permanent (utile debug + manual hotfix release)
- Si quotas GitHub Actions billing épuisés caveat poka (cf. memory M4.H.D) : skip ce test, marquer caveat

---

## 8. Beta.8 — Rapport + cleanup (~0.5j)

### Scope

- Rapport `.planning/session-beta-release-prep-report.md`
- Memory `warren_session_beta_release_prep_delivered.md` warren-app
- Update MEMORY.md
- Cleanup worktree

---

## 9. Sources cross-repo à lire (PARALLÈLE)

- `.github/workflows/release.yml`
- `docs/24-CODE-SIGNING.md`
- `.planning/session-d7-play-store.md`
- `desktop/packages/mullvad-vpn/electron-builder.cjs` (build config Mullvad ref)
- `dist-assets/` (signing assets pattern)
- Memory `warren_phase5_external_blockers_done`

---

## 10. Critères GO ULTIMATE

- ✅ Beta.2-Beta.7 critères GO PASS
- ✅ CI release.yml audit + secrets matrix wired
- ✅ docs/RELEASE-PROCUREMENT.md + docs/RUNBOOK-RELEASE.md complets
- ✅ scripts/release/verify-beta.sh idempotent
- ✅ CHANGELOG.md cross-repo
- ✅ Dry-run workflow_dispatch PASS
- ✅ Rapport rédigé
- ✅ Worktree cleaned

Verdict GO PARTIEL acceptable si :
- Beta.7 dry-run skipped (GH Actions billing épuisé poka caveat)

---

## 11. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé
- English-only code comments
- Pas em-dash
- Pas secrets in commits (ESPECIALLY ici, double vigilance certs)

---

## 12. Memory updates

- `warren_session_beta_release_prep_delivered.md`
- Update MEMORY.md

---

## 13. Commencer maintenant

Worktree §0.6, sources §9 en parallèle, attaque Beta.2 audit CI. Push au fil de l'eau.

Bonne route.
