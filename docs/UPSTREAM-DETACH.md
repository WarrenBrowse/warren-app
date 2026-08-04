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
- `mullvad-leak-checker/`
- split tunneling (`talpid-core` split-tunnel modules, `mullvad-exclude`)

Everything else (tunnel backend, relay selection, API client, account/identity,
UI) is Warren-owned and no longer chases upstream.

## How to operate this

- Periodically review upstream commits touching the watch list above; pick the
  security/correctness fixes.
- Do NOT attempt whole-tree rebases or merges.
- This unblocks the dead-weight purge that was previously kept "for rebase
  hygiene" (legacy RelaySelector, settings migrations, access-method/shadowsocks
  stack), tracked separately. Note the explicit carve-out: code that is dead
  today but encodes a Warren product capability (e.g. the MASQUE obfuscation
  transport) is kept as reference, not deleted. See
  `.planning/PRODUCT-obfuscation-transport.md`.

## Reversibility

Reversible: the remote and tag remain. If upstream ships something large that
Warren wants wholesale, a targeted merge of specific paths is still possible.
The decision only drops the *expectation* of continuous rebase.
