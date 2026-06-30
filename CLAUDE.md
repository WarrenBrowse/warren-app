# Warren App: Project Rules for Claude Code

Warren VPN desktop/mobile app, a fork of Mullvad VPN.

> Shared Warren rules (single source of truth: WarrenBrowse/warren-workspace).
> They resolve when this repo is checked out inside the workspace (mani sync);
> cloned standalone, the imports just warn harmlessly.
@../shared/rules/00-conventions.md
@../shared/rules/10-tdd.md
@../shared/rules/20-errors-secrets.md
@../shared/rules/30-git-commits.md

## Language and typography: repo-specific notes

The shared conventions rule already mandates English-only code and comments, the
em-dash ban, and "why not what" comments. For this repo specifically:

- The English-only rule and the em-dash ban extend to **every i18n /
  localization resource**, in every language: `.po`, `.pot`, `.xcstrings`,
  Android `strings.xml`, and any other localization file, plus user-facing UI
  copy in source. When a sentence needs an em-dash break, use the language's own
  comma (`،` Arabic, `，` Chinese, `、` Japanese), a colon/period, a hyphen for
  ranges, or a plain space (Thai). The em-dash was bulk-removed from the app once
  already; do not reintroduce it, and do not substitute the en-dash `–` either.
- Rationale for English-only code: Warren is a fork of Mullvad VPN (English-only
  upstream); uniform English keeps upstream rebases clean (no diff noise from
  translated comments) and the fork accessible to non-French contributors.
- The English-only rule does **not** apply to: `.planning/` artifacts,
  French-team-scoped markdown docs, user-facing translations / i18n message
  values (their own translation flow), or assistant chat output.
- Opportunistic cleanup: when you touch a file, translate stray French comments
  to English and replace stray dashes.

## Dependency layout: two Warren siblings (post-cutover)

The fork consumes Warren crates by `path` from two sibling repos checked out next
to `warren-app`:

- **`../warren-core/`** (control-plane + tunnel keepers): `warren-identity`,
  `warren-tunnel`, `warren-client`, `warren-config`, `warren-relay-selector`,
  `warren-api`, `warren-api-client`. Its checkout SHA is pinned in
  `.warren-core-version`. The quinn fork (`vendor/quinn-fork/`) also lives here and
  is wired through `[patch.crates-io]` in `Cargo.toml`.
- **`../warrenguard/`** (carved data-plane engine): `warrenguard-wire` (formerly
  `warren-protocol`), `warrenguard-multihop`, `warrenguard-relay`,
  `warrenguard-natpmp-client`, `warrenguard-natpmp-protocol`, `warrenguard-backoff`.
  Its checkout SHA is pinned in `.warrenguard-version`.

Both pins must move together with the `Cargo.lock` when bumping either sibling
(keep the quinn fork patched so the GSO knobs stay present). Do NOT reintroduce the
old shim names (`warren-protocol`, `warren-multihop`, `warren-natpmp-*`,
`warren-backoff`, `warren-relay`); they were deleted from `warren-core` and the
engine equivalents now live under `warrenguard-*`.

## Deployment rule: ALWAYS bump versions before redeploying exit nodes

Non-negotiable rule (poka, 2026-06-11). Before ANY redeploy of a warren-exit
binary to production (warren-exit-1, warren-exit-sin):

1. **Bump** `version` in `[workspace.package]` of `../warren-core/Cargo.toml`
   FIRST. Without it, two different builds carry the same number and only a
   SHA-256 hash comparison can tell what runs in prod.
2. **Commit before building**: never deploy a `-dirty` binary
   (`git describe` exposes it; `warren-exit --version` prints
   `git describe (semver)` since 2026-06-11).
3. **Verify after**: `ssh root@<exit> '/usr/local/bin/warren-exit --version'`
   must show the new version.
4. **Canary order**: warren-exit-sin first, then warren-exit-1.

Full procedure: `../warren-core/CLAUDE.md` section "Règles de déploiement des
exit nodes".

## Windows development (build + run/debug)

Supported on Windows 10/11, x64 and ARM64 (a Parallels Windows-on-ARM VM is a
valid target). All tooling lives in `scripts/dev/windows/`. Run the `.sh` helpers
from **Git Bash** and the `.ps1` helpers from PowerShell.

### Prerequisites (one-time, via winget; force `--source winget`, msstore has a cert error)

- **VS 2022 Build Tools** with the C++ workload + the target ARM64 *and* x86/x64
  toolsets + a Windows 11 SDK + clang (`Microsoft.VisualStudio.2022.BuildTools`).
- **Rust** (`Rustlang.Rustup`), then `rustup target add aarch64-pc-windows-msvc
  x86_64-pc-windows-msvc i686-pc-windows-msvc` (i686 is needed for the NSIS plugins).
- **zig**, **Go**, **protoc** (`Google.Protobuf`), **volta** (provides the Node/npm
  pinned in `desktop/package.json`), **Git for Windows**.
- `git config --global core.longpaths true` is **required**: some renderer paths
  exceed the 260-char limit and `git checkout` fails without it.
- podman is NOT needed: the gRPC bindings are committed under
  `desktop/packages/management-interface/dist`.

### Siblings + quinn fork

`../warren-core` and `../warrenguard` must be checked out next to this repo at the
SHAs pinned in `.warren-core-version` / `.warrenguard-version` (see the dependency
layout section above). The quinn fork is gitignored and must be regenerated once:
`../warren-core/bench/scripts/setup-quinn-fork.sh`.

### Build

- Full app + NSIS installer: `scripts/dev/windows/build-app.sh [--optimize]`
  (installer lands in `dist/`).
- Daemon + CLI only, for fast dev iteration: `scripts/dev/windows/build-daemon.sh`.

These wrappers source `scripts/vcvars.sh` (locates `vcvarsall.bat` via vswhere, so
Community *or* Build Tools works) and put `msbuild.exe` on PATH (vcvarsall does
not). `scripts/utils/host` detects the host arch from the OS architecture string
**locale-independently** (it must match `*ARM*64*`; a French Windows reports
"Processeur ARM 64 bits"). `build-app.sh` sets `TARGETS=<host triple>` so
electron-builder packages for the host arch (its `pack-windows` defaults to x64
otherwise). Native routing (the split-default that sends traffic through the TUN)
runs through `warren-core`'s `warren-winroute` crate (Win32 IP Helper API), not
PowerShell.

### Run / debug

The daemon MUST run as the SYSTEM user on Windows (it edits the WFP firewall and
creates the WinTUN adapter). Use the dev service:

```powershell
# once (elevates via UAC): registers WarrenVPN as a SYSTEM service and delegates
# start/stop to interactive users, so no further elevation is needed.
powershell -ExecutionPolicy Bypass -File scripts/dev/windows/dev-service.ps1 -Action Install
powershell -ExecutionPolicy Bypass -File scripts/dev/windows/dev-service.ps1 -Action Start   # / Stop / Restart / Status / Logs
```

Daemon logs: `dev-logs/daemon.log` (verbose). The Electron app (hot-reload):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/dev/windows/run-gui.ps1
```

`run-gui.ps1` must run in the logged-on user's interactive session (Chromium does
not render from a background/non-interactive context, you get a white window).
