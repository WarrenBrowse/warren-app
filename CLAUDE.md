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
adopted (205 of 330 existing names violate them), and `detekt` reports a pile of
pre-existing findings against an empty baseline (197 weighted issues on
2026-09-03; the count moves with every lot). Gating either today paints main
red for reasons unrelated to the change under review. To check you introduced no
detekt regression, run `./gradlew detekt --rerun-tasks` once before your change
and once after, and compare the two weighted counts; a plain `detekt` prints
nothing when up-to-date, which reads as a false clean.

## Android performance: measure before and after, on the release-shaped build

Never profile a debug build (StrictMode, Compose diagnostics, an 18 MB
unoptimised `.so`). The measurable build is `betaBenchmarkRelease` (R8,
profileable, release Rust), and the baseline every performance change is
compared against, scenario by scenario with the exact commands, is
[`android/docs/PERF-BASELINE.md`](android/docs/PERF-BASELINE.md); the
scripts that produced it live in `android/scripts/perf/`. Its thresholds are
proposals, not gates, until a lot promotes one. The on-device tooling (the
Compose stability configuration and its opt-in reports, the `WarrenJank`
logcat accounting, the baseline profile generator on Warren's flow) is
described in `android/docs/BuildInstructions.md`.

## Desktop unit tests run on a Node-only machine, and must stay that way

`warren-tests.yml` runs the vitest suite (`npm run test -w mullvad-vpn`) on a
runner that has Node and nothing else, and `desktop/.npmrc` carries
`ignore-scripts=true`, so no install ever downloads the Electron binary. A spec
that reaches a real Electron API or shells out to `cargo` therefore passes on
your machine and fails there. `electron` is aliased to a stub in the `test`
section of `vite.config.ts`, and `product-env-icons.spec.ts` answers the
packaging config's `cargo run --bin mullvad-version` from a stand-in on `PATH`.
Reproduce the CI conditions before pushing a new spec: hide
`desktop/node_modules/electron/path.txt` and run with a `PATH` that has no
cargo.

The Playwright suites are a different story: `frontend.yml` and
`desktop-e2e.yml` are dispatch-only in this fork, and `test/e2e/mocked` is red
from `main.spec.ts` (it still asserts the Mullvad window title) with
`maxFailures: 1` cutting the rest of the run. Run a single mocked spec with
`npm run build:test && npx playwright test mocked/<name>`.

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

## Linux packaging: eight artifacts, two builds

`build.sh` emits `.deb`, `.rpm` and `.pacman` for the architecture it runs
on, twice (x86_64 and aarch64 pools), and two more artifacts are derived
from the amd64 `.deb`: a sysvinit flavour and a NixOS flake tarball. The
same build ships `warren-nm-vpn-service`, without which GNOME and KDE show
no VPN indicator at all. Artifact names, the per-format architecture
spellings, and the three NetworkManager traps:
[`docs/LINUX-PACKAGING.md`](docs/LINUX-PACKAGING.md).

## Windows development (build + run/debug)

Supported on Windows 10/11, x64 and ARM64. All tooling lives in
`scripts/dev/windows/`, the `.sh` helpers run from Git Bash and the `.ps1`
ones from PowerShell. Prerequisites, the build wrappers and their per-env
flags, and the SYSTEM dev service needed to run the daemon:
[`docs/WINDOWS-DEV.md`](docs/WINDOWS-DEV.md). The VM side is the
`warren-windows-vm` skill.
