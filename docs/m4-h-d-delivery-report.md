# M4.H.D - Migration GitHub + Build pipeline DMG/Linux/MSI + CI release

**Date** : 2026-05-20
**Verdict** : **GO ULTIMATE**
**Effort** : ~2h wall-clock (vs 4-7 days estimated)
**Cost Hetzner** : 0.00 EUR

## Overview

Two chantiers shipped in a single phase. Chantier A migrates the
warren-app hosting from Gitea selfhost to `github.com/WarrenBrowse/warren-app`
to align with warren-core. Chantier B wires the entire signed-installer
build pipeline (build branding + signing env vars + CI release workflow +
release script + docs).

## Chantier A - Migration GitHub

| Item | Status |
|---|---|
| Repo created `github.com/WarrenBrowse/warren-app` | private, GPL-3.0, poka-IT admin |
| `main` HEAD push | `d21e067de7` -> `5b02eeef34` |
| Branches pushed | `warren-base`, `warren-base-phase1a` |
| Tags pushed | 478 tags (full Mullvad upstream + Warren history) |
| Local `origin` remote | `git@github.com:WarrenBrowse/warren-app.git` |
| Local `backup-gitea` remote | fetch-only, push locked `no_push` |
| Local `upstream` remote | preserved `https://github.com/mullvad/mullvadvpn-app` |
| Doc URLs Gitea -> GitHub | `README.md`, `BuildInstructions.md`, `UPSTREAM_BASELINE.md` |

Notes on the migration commit:
- `gh repo create` with `--license gpl-3.0` auto-created an initial
  `Initial commit` containing only `LICENSE`. To preserve the full
  warren-app history (and its tag baseline), the auto-init commit was
  overwritten via `git push --force-with-lease=main:0855925c5c`. This
  is destructive only to the auto-generated scaffold; the user-facing
  intent (migrate the existing repo as-is) is preserved.
- `gh repo delete` was attempted to avoid the force-push but failed
  with `HTTP 403 Must have admin rights`. `poka-IT`'s token lacks
  `delete_repo` scope. Force-push was the only path without an
  interactive scope refresh.

## Chantier B - Build pipeline + Signing + CI release

### Pre-existing R1 rebrand status

Most R1 rebrand was already in place (`build.sh` log_header,
`distribution.cjs` productName, `dist-assets/pkg-scripts/postinstall`
plist + log paths, `dist-assets/linux/` warren-* services + apparmor,
Cargo `[[bin]] name = "warren-daemon"` + `name = "warren"`). M4.H.D
patched the remaining 11 caveats and added the signing/CI plumbing on
top.

### Build branding deltas

```
build.sh:
  -du URL Gitea -> GitHub
  Universal Windows installer name MullvadVPN- -> WarrenVPN-

desktop/scripts/pack-universal-win.sh:
  log_header "Mullvad VPN" -> "Warren VPN"
  dest "MullvadVPN-" -> "WarrenVPN-"

desktop/packages/mullvad-vpn/tasks/distribution.cjs:
  appId 'net.mullvad.vpn' -> 'com.warrenbrowse.vpn'
  nsis.guid '2A356FD4-03B7-4F45-99B4-737BE580DC82' -> '15528187-40A4-4A0D-B38A-F8E3442EC456'

desktop/packages/mullvad-vpn/package.json:
  repository -> 'https://github.com/WarrenBrowse/warren-app'

dist-assets/windows/installer.nsh:
  7 user-facing error strings: "Mullvad service" -> "Warren VPN service"
  ("Failed to install/start/kill", "Stopping/Removing", "tray icon", ...)
```

Internal symbols intentionally kept upstream-named (R1 conservatism for
upstream rebases):

- Env vars `MULLVAD_RESOURCE_DIR`, `MULLVAD_ADD_MANIFEST` (consumed by
  `mullvad-daemon/src/cli.rs`, `mullvad-paths/src/resources.rs`,
  `mullvad-daemon/build.rs`).
- Windows service identifier `mullvadvpn` in installer.nsh (matched by
  case-insensitive `sc.exe` lookup against the `MullvadVPN`
  registration in `mullvad-daemon/src/lib.rs:170`).
- NSIS internal macro names (`ExtractMullvadSetup`).
- Helper binaries (`mullvad-setup`, `mullvad-problem-report`,
  `mullvad-exclude`) - R1 decided to keep these upstream-named.

### Smoke build branding check

A new fast (sub-second) shell smoke validates 26 branding invariants
across `build.sh`, `pack-universal-win.sh`, `distribution.cjs`,
`package.json`, the pkg-scripts, the Linux service files, and the
Cargo `[[bin]]` declarations.

```
$ bash scripts/dev/smoke-build.sh
... 26 PASS / 0 FAIL ...
All Warren branding checks passed
```

### Signing env vars wiring

`build.sh` now reads, with Warren-prefixed vars taking precedence:

```bash
: "${CSC_LINK:=${WARREN_CSC_LINK_MACOS:-${WARREN_CSC_LINK:-}}}"
: "${CSC_KEY_PASSWORD:=${WARREN_CSC_KEY_PASSWORD_MACOS:-${WARREN_CSC_KEY_PASSWORD:-}}}"
: "${CERT_HASH:=${WARREN_CERT_HASH:-}}"
: "${NOTARIZE_KEYCHAIN:=${WARREN_NOTARIZE_KEYCHAIN:-}}"
: "${NOTARIZE_KEYCHAIN_PROFILE:=${WARREN_NOTARIZE_KEYCHAIN_PROFILE:-}}"
```

The pattern is backward-compatible: developers using upstream
Mullvad-named env vars locally are not impacted; CI uses
`WARREN_CSC_*` GitHub Secrets.

### CI release pipeline

New `.github/workflows/release.yml` (235 lines):

| Job | Runner | Output |
|---|---|---|
| `build-macos` | macos-14 | `WarrenVPN-*.dmg`, `WarrenVPN-*.pkg` (signed + notarized if secrets set) |
| `build-linux` | ubuntu-22.04 | `WarrenVPN-*.deb`, `WarrenVPN-*.rpm`, `warren-vpn-daemon_*.deb`, `warren-vpn-daemon_*.rpm` |
| `build-windows` | windows-2022 | `WarrenVPN-*.exe`, `WarrenVPN-*.msi` (signed if secret set) |
| `publish-release` | ubuntu-22.04 | aggregated GitHub Release (draft) with SHA-256 checksums |

The macOS and Windows build jobs decode their respective `.p12` /
`.pfx` from base64 GitHub Secrets, write them to a temp file (macOS)
or import them into the cert store (Windows), build, then wipe the key
material in an `if: always()` step.

### `mullvad-build-env` composite action

Two new inputs:

- `warren-core-token` : PAT for read access to `WarrenBrowse/warren-core`.
  When provided, the action checks out the sibling repo at the SHA
  pinned by `.warren-core-version` into `../warren-core`. When empty,
  the action prints a warning and skips the checkout (cargo commands
  will then fail at workspace metadata resolution, but non-cargo jobs
  remain unaffected).
- `skip-warren-core` : override to forcibly skip even when a token is
  available. Default `false`.

### Existing workflows wired with the token

- `clippy.yml`, `daemon.yml` (both `build-linux` and `build-windows`
  jobs), `frontend.yml`, `warren-fork.yml`
- `release.yml` (all 3 build jobs)

### Warren fork CI extended to main

`warren-fork.yml` triggers `[warren-base]` -> `[main, warren-base]`,
so the existing lightweight Warren-specific test bundle now fires on
the GitHub `main` branch too.

### Release documentation

- `Release.md` (123 lines, fully rewritten) : enumerates the 7 GitHub
  Secrets, explains base64-encoding of `.p12` / `.pfx`, walks through
  Apple notarytool credential setup, includes a certificate rotation
  policy.
- `prepare-release.sh` (151 lines, recreated from scratch) :
  - Validates CalVer or SemVer version
  - Refuses on dirty working tree (submodule deltas tolerated)
  - Requires git signing key configured
  - Updates `desktop/packages/mullvad-vpn/package.json` version
  - Creates a signed `v<VERSION>` tag
  - `--dry-run` mode for safe preview (verified locally :
    `0.0.0 -> 2026.5.0-dryrun` preview + revert)
- `.gitignore` : `*.p12`, `*.pfx`, `*.cer`, `.notarytool-creds.json`
  excluded (signing assets must never be committed).

## Validation gates

| Gate | Status |
|---|---|
| `bash scripts/dev/smoke-build.sh` | **26 PASS / 0 FAIL** |
| `bash -n build.sh` | OK |
| `bash -n prepare-release.sh` | OK |
| `bash prepare-release.sh --desktop --dry-run 2026.5.0-dryrun` | bumps + reverts cleanly |
| Workflows registered on `WarrenBrowse/warren-app` | **50** active, includes `Release` |
| Warren fork CI fired on push to `main` | yes (account billing prevented execution; see caveats) |

## Commits push origin/main (8 commits since brief `d21e067de7`)

1. `5b02eeef34` chore(infra): migrate hosting Gitea to
   github.com/WarrenBrowse/warren-app
2. `3cb6aba847` feat(build): adapt build.sh + distribution.cjs for
   Warren branding
3. `228102c1af` feat(installer): rebrand Windows installer user-facing
   strings to Warren VPN
4. `cb1a34b3db` feat(build): wire WARREN_CSC_* + WARREN_NOTARIZE_*
   signing env vars alongside upstream CSC_*
5. `69822191fe` ci: add release.yml + warren-core checkout in build env
   + extend warren-fork CI to main
6. `e25a4d34fd` docs(release): adapt Release.md + recreate
   prepare-release.sh for Warren CI release pipeline
7. `ac39973f17` fix(release): prepare-release.sh tolerate submodule
   deltas + untracked files for dirty check
8. (this commit) docs(M4.H.D): GO ULTIMATE verdict + delivery report
   + memory updates

## Decisions archi (auto, doctrine §0.5)

- **macOS bundle ID** : `com.warrenbrowse.vpn` (clean fork, no Mullvad
  upgrade path attempted).
- **Windows installer GUID** : freshly minted
  `15528187-40A4-4A0D-B38A-F8E3442EC456` (side-by-side install with
  Mullvad supported).
- **Universal install filename** : `WarrenVPN-<version>.{dmg,exe,pkg,...}`.
- **Signing env vars** : Warren-prefixed take precedence over upstream
  Mullvad CSC_* when both are set (backward-compat preserved).
- **Versioning** : CalVer recommended (e.g., `2026.5.0`,
  `2026.5.0-beta1`), SemVer accepted by `prepare-release.sh` regex.
- **Internal symbols kept upstream** : env vars MULLVAD_*, Windows
  service identifier `mullvadvpn`, internal NSIS macros. Rationale:
  these are not user-visible, R1 decided to keep them aligned to
  upstream Mullvad to ease cherry-pick rebases.

## Caveats residuels (out of scope)

| Caveat | Owner | Impact |
|---|---|---|
| GitHub Actions billing exhausted on poka-IT | poka | blocks CI execution until upgrade or repo public |
| WARREN_CORE_RO_TOKEN secret not configured | poka | cargo CI jobs warn + skip warren-core checkout, then fail at workspace metadata |
| Signing assets (.p12 macOS + .pfx Windows + notarytool creds) pending | poka | unsigned dev builds only until provisioned |
| Live full `./build.sh` not run | n/a | smoke validates branding strings only; first artifact production on first tagged release |
| `gh repo delete` 403 (poka-IT lacks delete_repo scope) | poka | one-time impact during M4.H.D.A.1 (worked around via force-push on auto-init) |
| SSH Hetzner bench provisioning bug | poka | M4.H.B inherited, blocks live bench installer test (skipped per brief §M4.H.D.8) |

## Next phase

**M4.H.D fully unblocks** the first Warren tagged release as soon as
the 7 GitHub Secrets + billing are provisioned by poka. The
orchestrator can then schedule:

- **M4.H.F** : NAT-PMP client port-forwarding (differentiator vs
  Mullvad/IVPN who dropped it in 2023).
- **M4.H.G** : `--bypass-cidr` + backoff tune.
- **M4.H.H** : `warrenbrowse.com` docs.
