# Decision: detach from upstream Mullvad, cherry-pick the platform layers

Date: 2026-06-12. This is the governing decision for how the fork is
maintained. It lived in `warren-core/docs/archive/` until 2026-08-02, which is
why nothing in this repo referenced it; the reconnaissance baseline it superseded
was deleted at the same time (it described a pre-Quinn state and said so itself).

## Context

warren-app is 782 commits past `upstream-baseline-2026-05-06`, with a delta of
~3,524 files (+179k / -166k). The three UI platforms (Android rewritten without
the daemon, iOS renamed wholesale, desktop flows woven into upstream views) and
the daemon core (`lib.rs`, `device/`, `rest.rs`, proto) are smeared with Warren
changes. Most of what upstream actively develops (WireGuard, DAITA,
obfuscation, the Mullvad API, account flows) is code Warren deleted or replaced.

A full rebase onto upstream HEAD would conflict in essentially every Android
file, every renamed iOS file, the daemon core, and the proto, for several
person-weeks per upstream release, recurring, in security-critical paths.

## Decision

Stop tracking upstream for rebase. Keep the `upstream` remote and the baseline
tag, and **cherry-pick surgically** from the small set of platform-integration
layers where upstream still delivers value Warren consumes and where Warren
barely diverged (so picks apply cleanly):

- `talpid-routing/`
- `talpid-dns/`
- `talpid-net/`
- firewall code: `talpid-core/src/firewall/`, `windows/winfw/`
- split tunneling (`talpid-core` split-tunnel modules, `mullvad-exclude`)
- privilege boundaries, added 2026-09-28 after three upstream security fixes
  landed outside the list above: `talpid-cgroup/` (setuid helpers), the
  install and uninstall scripts, and the IPC trust checks on both ends of the
  management socket

The authoritative list is `scripts/dev/upstream-watch-paths.txt`; the triage
script reads it. Upstream security fixes are taken wherever they land, list or
not.

Everything else (tunnel backend, relay selection, API client, account/identity,
UI) is Warren-owned and no longer chases upstream.

## How to operate this

- Review upstream with the `upstream-sync` skill
  (`.claude/skills/upstream-sync/`). It runs `scripts/dev/upstream-triage.sh`
  from the commit in `.upstream-reviewed`, judges each watch-list commit and
  each upstream security entry, picks what applies, and records every verdict
  in [`UPSTREAM-SYNC-LOG.md`](UPSTREAM-SYNC-LOG.md).
- Cadence: a whole review at least every 30 days, and a review within the week
  of any new `### Security` entry in upstream's changelogs. An applicable
  upstream security fix is ported within seven days of that review, or recorded
  as an open item with what blocks it. Nobody has to remember this: the
  scheduled workflow `upstream-review.yml` runs the triage every Monday and
  opens (or comments on) the issue "Upstream review due" when
  `scripts/dev/upstream-review-due.sh` finds either condition.
- Do NOT attempt whole-tree rebases or merges.
- This unblocks the dead-weight purge that was previously kept "for rebase
  hygiene" (legacy RelaySelector, settings migrations, access-method/shadowsocks
  stack), tracked separately. Note the explicit carve-out: code that is dead
  today but encodes a Warren product capability (e.g. the MASQUE obfuscation
  transport) is kept as reference, not deleted. See
  `.planning/PRODUCT-obfuscation-transport.md`.

## Binaries shipped as Mullvad built them

### The Windows split-tunnel driver

Split tunneling and include-only on Windows run on Mullvad's kernel driver,
shipped unchanged: `dist-assets/binaries/<target>/split-tunnel/mullvad-split-tunnel.sys`
(version 1.3.0.0 in its `.inf`), signed by Mullvad VPN AB and countersigned by
the Microsoft Windows Hardware Compatibility Publisher. Warren keeps it
(decision of 2026-11-17): a driver of its own needs an EV certificate and a
Hardware Dev Center account, and the source it would be built from is the same
`win-split-tunnel` tree. What that decision carries:

- Warren cannot patch or re-sign it. A flaw in it, a revocation of Mullvad's
  certificate or an entry on Microsoft's vulnerable driver blocklist breaks
  split tunneling until Mullvad ships a new build, which then reaches Warren
  through the `dist-assets/binaries` submodule pin.
- Its names are fixed: service `mullvad-split-tunnel`, device
  `\\.\MULLVADSPLITTUNNEL` (`talpid-core/src/split_tunnel/windows/`). It is one
  machine-wide resource. Only the production installer resets it on install
  and uninstall (`dist-assets/windows/installer.nsh`); beta and staging leave it
  alone so they never pull it from under a production install.
- On a machine that also runs Mullvad VPN, the two products drive the same
  service and device, and the driver holds one configuration at a time. The
  production WFP objects of `winfw` also keep Mullvad's GUIDs: its salt is 0 for
  prod (`build-windows-modules.sh`), and only beta and staging derive their own.
  Running Warren and Mullvad VPN side by side on Windows is not supported.
- System tools show the driver under Mullvad's name.
- The driver is GPL: the source offer for a Windows release covers
  `win-split-tunnel` at the commit the `dist-assets/binaries` pin carries.

Wintun (`wintun.dll`, signed by WireGuard LLC) is in the same position, with no
known name collision.

## Inherited names: what gets renamed, and when

A name is renamed when the user or an administrator can see it; an internal
name stays, because the cherry-picks above apply by path.

| family | policy |
|---|---|
| crates `talpid-*` | never renamed |
| crates `mullvad-*` | renamed only if the upstream review is given up |
| protobuf packages `mullvad_daemon.*` | never renamed: a GUI and a daemon of two versions talk across an update |
| environment variables read by the daemon and the GUI | each has a `WARREN_*` name that wins, and its `MULLVAD_*` name stays read until the first production release, then goes: `WARREN_SETTINGS_DIR`, `_LOG_DIR`, `_CACHE_DIR`, `_RPC_SOCKET_PATH`, `_RESOURCE_DIR`, `_MANAGEMENT_SOCKET_GROUP`, `WARREN_BACKTRACE_ON_FAULT`, and the GUI's `WARREN_PATH`, `WARREN_DISABLE_UPDATE_NOTIFICATION` |
| `MULLVAD_API_HOST`, `_ADDR`, `_FORCE_DIRECT`, `_DISABLE_TLS` | no alias: release builds ignore them, so no administrator override exists to carry over |
| `TALPID_*` variables | never renamed |
| npm package `mullvad-vpn` | renamed only in a campaign shared with warren-extension, whose `pnpm design:sync` copies by path |
| Swift modules and types | finished before the first iOS store submission |
| Linux cgroup | renamed and scoped by environment, with a migration of the processes already excluded |
| private WFP objects of `winfw` | renamed with the Windows firewall migration; the two shared sublayers keep Mullvad's names while the 1.2 driver is supported |
| split-tunnel driver service, device and file | kept, see above: renaming needs a driver of Warren's own |
| paths, socket, launchd label, pf anchor, nft table | renamed and scoped by environment |

## Reversibility

Reversible: the remote and tag remain. If upstream ships something large that
Warren wants wholesale, a targeted merge of specific paths is still possible.
The decision only drops the *expectation* of continuous rebase.
