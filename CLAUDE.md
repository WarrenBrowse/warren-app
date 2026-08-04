# Warren App: Project Rules for Claude Code

Warren VPN desktop/mobile app, a fork of Mullvad VPN.

> Shared Warren rules (single source of truth: WarrenBrowse/warren-workspace).
> They resolve when this repo is checked out inside the workspace (mani sync);
> cloned standalone, the imports just warn harmlessly.
@../shared/rules/00-conventions.md
@../shared/rules/10-tdd.md
@../shared/rules/20-errors-secrets.md
@../shared/rules/30-git-commits.md
@../shared/rules/50-release-channels.md

## `gh` here talks to Mullvad unless you tell it otherwise

This fork keeps two remotes: `origin` (WarrenBrowse/warren-app) and `upstream`
(mullvad/mullvadvpn-app). With no `gh` default repo set, `gh run list` and
`gh run view` resolve to **upstream**, so you read Mullvad's CI and take it for
ours. The output is masked (`***vpn-app`, `net.***.***vpn.test.e2e`), which
hides the giveaway. On 2026-08-02 that produced a confident and wrong report
that our Android CI had been red for two days: it was Mullvad's nightly, on a
commit absent from our history.

Pass `--repo WarrenBrowse/warren-app`, or run
`gh repo set-default WarrenBrowse/warren-app` once per clone. Sanity check: if a
run's `headSha` is not an object here (`git cat-file -t <sha>` fails), you are
looking at the wrong repository.

## Android CI: what is gated, and what cannot be

`.github/workflows/android-checks.yml` runs the JVM unit tests on push/PR
touching `android/**` or `warren-jni/**`. Before it existed (2026-08-02) an
android-only change triggered **nothing**: the inherited `android-*.yml`
workflows are all dispatch-only in this fork and `warren-checks.yml` is
paths-filtered to the daemon/desktop tree. That is how a blocking HTTPS call on
the UI thread reached main.

Two suites are excluded on purpose, so do not "fix" the workflow by adding them:
`:test:arch` enforces inherited Mullvad test-naming conventions this fork never
adopted (205 of 330 existing names violate them), and `detekt` reports 174
pre-existing findings against an empty baseline. Gating either today paints main
red for reasons unrelated to the change under review. To check you introduced no
detekt regression, compare against that 174 with
`./gradlew detekt --rerun-tasks`; a plain `detekt` prints nothing when
up-to-date, which reads as a false clean.

## Scenery art is generated, never hand-converted

The connect screen of all three clients stacks three pre-registered full-frame
layers (landscape, burrow, Bula), so they only line up if every client ships the
same canvas. When new master art arrives, drop it in `new-da/calques/`
(git-ignored, unlike a folder named anything else) and run
`desktop/packages/mullvad-vpn/scripts/process-scenery.sh`, which emits desktop,
Android and iOS in one pass. Converting a layer by hand, or regenerating one
platform alone, is what left Singapore photoreal on desktop for a whole release.
`test/unit/scenery-assets.spec.ts` gates it.

iOS backgrounds are **JPEG**, not PNG: asset catalogs reject WebP, the masters
are already JPEG so a PNG imageset conserves nothing, and it cost 10 MB of
`Assets.car`. The two alpha layers stay PNG. Formats, measurements and the
procedure for adding a country: [`docs/SCENERY-ART.md`](docs/SCENERY-ART.md).

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
  upstream); uniform English keeps cherry-picks clean (no diff noise from
  translated comments) and the fork accessible to non-French contributors. This
  repo STOPPED rebasing on upstream in June 2026 and now cherry-picks a named set
  of platform layers only: read
  [`docs/UPSTREAM-DETACH.md`](docs/UPSTREAM-DETACH.md) before touching anything
  upstream-adjacent.
- The English-only rule does **not** apply to: French-team-scoped markdown
  docs, user-facing translations / i18n message values (their own
  translation flow), or assistant chat output.
- Opportunistic cleanup: when you touch a file, translate stray French comments
  to English and replace stray dashes.

## Dependency layout: three Warren siblings (fully off warren-core)

The fork consumes Warren crates by `path` from three sibling repos checked out
next to `warren-app`. It has **no dependency on `warren-core`** (the private
backend), not even in tests: the SDK-client-to-server wire conformance lives in
warren-core itself (`warren-core/conformance/`), so nothing here needs it.

- **`../warrenguard/`** (AGPL data-plane engine): `warrenguard-transport`,
  `-transport-core`, `-route-split`, `-config`, `-wire`, `-multihop`, `-relay`,
  `-natpmp-client`, `-natpmp-protocol`, `-backoff`, `-daita`, `-pump`. Pinned in
  `.warrenguard-version`.
- **`../warren-sdk-rs/`** (client SDK): `warren-api` (signed account client),
  `warren-identity`. Pinned in `.warren-sdk-version`.
- **`../warren-contract/`** (neutral client<->server contract): `warren-contract`
  (SS58, X-Warren signing, `/v1` DTOs) and `warren-discovery-core`. Pinned in
  `.warren-contract-version`.

The quinn fork is the published `WarrenBrowse/warren-quinn` git-dep (pinned by
tag `v0.11.16-fork.8`), wired through `[patch.crates-io]` in this repo's root
`Cargo.toml`. The crates are renamed (`warren-quinn`/`-proto`/`-udp`) but their
lib names stay `quinn`/`quinn_proto`/`quinn_udp`, so every `use quinn` and the
mullvad-logging crate-name filters are unchanged; warrenguard consumes the same
fork. There is no vendored tree and no setup script.

Sibling pins move together with the `Cargo.lock` when bumping any of them (keep
the warren-quinn patch in place so the GSO and Initial-fragmentation obfuscation
knobs stay present; `build.sh` and CI fail loudly if the lock stops pinning the
fork). Do NOT reintroduce the old shim names (`warren-protocol`,
`warren-multihop`, `warren-natpmp-*`, `warren-backoff`, `warren-relay`); the
engine equivalents now live under `warrenguard-*`.

**Regenerate the lock with `scripts/dev/regen-lockfile.sh`, never a bare
`cargo update -w`.** CI checks the siblings out at the pinned SHAs, but your dev
machine has them at branch HEAD (newer). A plain `cargo update` resolves against
HEAD and bakes crates the pinned siblings do not pull (e.g. the anonymous-token
`rsa` / `blind-rsa` tree), so `cargo metadata --locked` in the CI coherence gate
then fails with "Cargo.lock is out of sync". The script rebuilds the lock the way
CI sees it, using throwaway worktrees at the pins (your checkouts untouched), and
verifies the quinn fork stays pinned. Run it after any `.warren*-version` bump,
then commit `Cargo.lock`.

## Linux packaging: five artifacts, one build

`build.sh` produces `.deb`, `.rpm` and `.pacman` through electron-builder/fpm.
Two more are derived from the `.deb` afterwards, by the `build-linux-sysvinit`
and `build-nixos` jobs in `release.yml`, on the docker-capable runner. Neither
recompiles anything, so both cost minutes, not another Rosetta build hour.

- **`-linux-amd64-sysvinit.deb`** (MX Linux, antiX, Devuan). The systemd
  package's postinst runs `systemctl enable` under `set -e`, so on a host with
  no systemd dpkg leaves it half-configured. `ci/build-sysvinit-deb.sh` repacks
  it with the LSB init scripts in `dist-assets/linux/sysvinit/`, renames it
  `<pkg>-sysvinit` and declares Provides/Conflicts/Replaces on `<pkg>` so the
  two flavours are exclusive. `ci/test-sysvinit-deb.sh` then installs, starts,
  crashes, stops and purges it for real inside a systemd-less Debian container.
- **`-linux-x86_64-nixos.tar.gz`** (NixOS). A tarball flake pinning that
  release's `.deb` by URL and hash, plus a NixOS module. Sources in
  `nix/warren-vpn/`, staged by `ci/stage-nixos-flake.sh` and built for real by
  `ci/build-nixos-flake.sh` (a `nix build` in a container, the binaries run out
  of the store, the module evaluated into a unit).

The same build also ships `warren-nm-vpn-service`, a NetworkManager VPN
service plugin, because GNOME and KDE light their VPN indicator for exactly one
thing: an active NetworkManager connection whose type is `vpn`. Only a
registered VPN plugin can produce one, so a tunnel the engine built itself is
invisible to the desktop without it. The plugin describes a tunnel that already
exists (it never builds one), the daemon publishes and withdraws the connection
from `mullvad-daemon/src/nm_vpn_indicator.rs`, and both sides are best effort:
a machine with no NetworkManager simply gets nothing. Three traps, each paid for
by a measurement on NM 1.46:

- **The postinst must reload dbus.** The system bus reads
  `/usr/share/dbus-1/system.d` once and denies ownership of a name it has no
  policy for, so without the reload the plugin cannot take its name until the
  next boot. NetworkManager itself does pick up a new `.name` file live.
- **The plugin watches its interface.** NetworkManager keeps a VPN connection
  `activated` after the interface is gone, so a daemon that dies would leave the
  desktop claiming a VPN. The plugin retracts it by index, not by name.
- **The published config must match the interface exactly, and claim nothing
  else.** NetworkManager reconciles what it is handed onto the live interface;
  `never-default` plus `preserve-routes` keep it off the engine's routing, and
  `auto-route-ext-gw` (NM 1.42+ only, or the connection is rejected) keeps it
  from adding a host route to the peer.

Three things to keep in mind when touching any of it:

- **sysvinit has no supervisor.** The daemon exits fail-closed, so
  `warren-daemon-supervise` respawns it forever, mirroring the unit's
  `Restart=always` / `StartLimitIntervalSec=0`. Dropping that would let a crash
  strand the machine offline with no daemon to unblock it.
- **`autoPatchelfHook` is deliberately unused** in `nix/warren-vpn/package.nix`.
  Its worker is a Python program, and the release runner builds under x86_64
  emulation where the interpreter loses `argv[0]` across `exec` and cannot find
  its own modules. The interpreter and RPATH are set with `patchelf` directly,
  and an explicit check fails the build on a NEEDED entry nothing provides.
- **The flake pins the update host, never a GitHub asset.** The repository is
  private, so a release asset answers 404 without a token and `nix build` would
  fetch nothing.

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

`../warrenguard`, `../warren-sdk-rs` and `../warren-contract` must be checked out
next to this repo at the SHAs pinned in `.warrenguard-version` /
`.warren-sdk-version` / `.warren-contract-version` (see the dependency layout
section above). The quinn fork needs no local setup: it is the published
`WarrenBrowse/warren-quinn` git-dep pinned by tag in this repo's root
`[patch.crates-io]`, fetched by cargo like any other git dependency. There is no
vendored tree to regenerate and no setup script to run.

### Build

Both wrappers take the product environment as `--prod`, `--beta` or
`--staging` (falling back to `WARREN_PRODUCT_ENV`, which is what the VM's
`wbuild.cmd` / `wbuildbeta.cmd` set) and print their help without building
anything when neither is given.

- Full app + NSIS installer: `scripts/dev/windows/build-app.sh --beta [--optimize]`
  (installer lands in `dist/`).
- Daemon + CLI only, for fast dev iteration:
  `scripts/dev/windows/build-daemon.sh --beta`. It refuses to stage a
  `winfw.dll` whose stamped environment differs from the daemon's: the WFP
  object keys are salted per environment, so a mismatched pair arms the kill
  switch under keys this environment's teardown never sweeps. Rebuild the dll
  with `./build-windows-modules.sh --beta winfw`.

These wrappers source `scripts/vcvars.sh` (locates `vcvarsall.bat` via vswhere, so
Community *or* Build Tools works) and put `msbuild.exe` on PATH (vcvarsall does
not). `scripts/utils/host` detects the host arch from the OS architecture string
**locale-independently** (it must match `*ARM*64*`; a French Windows reports
"Processeur ARM 64 bits"). `build-app.sh` sets `TARGETS=<host triple>` so
electron-builder packages for the host arch (its `pack-windows` defaults to x64
otherwise). Native routing (the split-default that sends traffic through the TUN)
runs through the engine's `warrenguard-winroute` crate (Win32 IP Helper API),
not PowerShell.

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
