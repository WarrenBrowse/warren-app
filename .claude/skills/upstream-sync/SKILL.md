---
name: upstream-sync
description: Review what upstream Mullvad (mullvad/mullvadvpn-app) changed since the last review, pick the security and correctness fixes that apply to this fork, and record every verdict in docs/UPSTREAM-SYNC-LOG.md. Use when asked to check upstream, sync with Mullvad, look for commits to cherry-pick, or review a Mullvad security advisory.
---

# Upstream sync

This fork no longer rebases on Mullvad (`docs/UPSTREAM-DETACH.md`). It takes
surgical picks, and every review leaves a record. Run the whole procedure; a
review that picks without recording, or records without judging every commit,
is incomplete.

State lives in three files:

- `.upstream-reviewed`: the last upstream commit reviewed (full SHA).
- `scripts/dev/upstream-watch-paths.txt`: the paths whose commits are judged
  one by one.
- `docs/UPSTREAM-SYNC-LOG.md`: the registry, with the **Open items** carried
  from one review to the next.

## 1. Prepare

- Read `docs/UPSTREAM-SYNC-LOG.md` whole, Open items first.
- `git pull --rebase --autostash` so the verdicts are about current `main`.
- Look at `git status`. Other agents often work in this checkout: never stage
  with `git add -A`, never cherry-pick into the index, and commit with
  `git commit --only <paths>`, then check `git show --stat HEAD`.

## 2. Triage

```bash
scripts/dev/upstream-triage.sh | tee /tmp/upstream-triage.txt
```

It adds and fetches the `upstream` remote if missing, then prints, for the
range from `.upstream-reviewed` to `upstream/main`:

1. every watch-list commit with a mechanical verdict (PICKED, CLEAN, PRESENT,
   CONFLICT);
2. the `### Security` entries the three upstream changelogs gained;
3. commits outside the watch list whose message looks security-related;
4. the advisories (GHSA, CVE) named in the range.

The verdicts are hints. A CLEAN patch may be irrelevant; a CONFLICT may already
be here, because the fork's import commit carried upstream code dated after the
baseline, and reverse application misses a change whose context moved. For a
CONFLICT, grep the fork for the lines the commit adds before calling it absent.

## 3. Judge

Read each candidate with `git show <sha> -- <paths>` and compare with the
fork's file. Upstream names map to Warren names: `mullvad-daemon` ships as
`warren-daemon`, `mullvad-exclude` as `warren-exclude` (plus Warren's own
`warren-include`), `MULLVAD_*` variables usually have a `WARREN_*` twin, and
`/Applications/Mullvad VPN.app` is `/Applications/Warren VPN.app`.

Take:

- security fixes (advisories, privilege escalation, IPC trust, leaks) that
  reach code Warren ships, **wherever they land**: sections 2 to 4 of the
  triage exist because upstream's fixes do not stay on the watch list;
- correctness fixes on the platform layers Warren runs (routing, DNS,
  firewall, split tunneling, leak checker) and OS compatibility fixes.

Leave, with the reason:

- WireGuard, GotaTun, DAITA, obfuscation, relay selection, the Mullvad API and
  account code: Warren replaced them;
- refactors, dependency moves, formatting, spelling, lints of a Rust newer than
  `rust-toolchain.toml`;
- anything that contradicts a deliberate Warren choice (read the fork's
  comments around the code first).

Keep as an Open item what is relevant but cannot be finished now: it needs a
platform you cannot validate on, a binary Warren does not ship, or a design
decision. Say what blocks it.

## 4. Pick

One commit per upstream fix (or per inseparable group).

- Apply from the working tree only:
  `git format-patch -1 --stdout <sha> -- <paths> | git apply`, or port by
  hand when the fork diverged. Never `git cherry-pick` into a shared index.
- TDD applies to picks. Watch the test go red without the fix. Upstream's own
  tests can be hollow: on 2026-09-28 the cgroup tests passed with the fix
  removed, and were rewritten.
- Validate on the platform the code runs on:
  - macOS: locally.
  - Linux:
    ```bash
    docker run --rm -v "$(cd .. && pwd)":/ws:ro \
      -v warren-upstream-cargo:/usr/local/cargo/registry \
      -v warren-upstream-target:/target -e CARGO_TARGET_DIR=/target \
      -w /ws/warren-app rust:<toolchain> cargo test -p <crate>
    ```
    Mount the workspace root, because the sibling path deps live there.
    A real Linux host, for anything that moves the firewall, routes or the
    service units: the Lima VMs (`limactl list`, `wl-ubuntu-2404` carries the
    beta package). Run each test as a detached `systemd-run` unit with a
    `systemd-run --on-active=<s> warren-beta disconnect` deadman beside it:
    a blocking firewall cuts Lima's own ssh, and a graceful `limactl stop`
    then `start` is the way back in. Measure leaks with a `curl` bound to the
    physical NIC (`--interface`), never `ping` (Lima's network answers ICMP
    itself).
  - Windows: CI's `windows-daemon` job, or the `warren:warren-windows-vm` skill.

  A cross `cargo check --target x86_64-pc-windows-msvc` fails on this Mac
  (ring needs MSVC) whatever the change is.
- Firewall, routing and tunnel changes follow the data-plane rule: validated
  against a real network before they are called done. If that is out of reach,
  the item stays open instead of being committed blind.
- Commit subject: Conventional Commits in Warren's words, ending with the exact
  marker the triage script matches:
  `fix(<scope>): <what it fixes> (upstream <sha10>)`.

## 5. Record

In `docs/UPSTREAM-SYNC-LOG.md`:

- Add a dated section `## <date>: <from10> to upstream <to10>` right under
  Open items, with tables: Picked (upstream, subject, our commit, note),
  Already present, Open, Not taken (with reasons). Group similar skips in one
  row.
- Rewrite the Open items table: close what was picked, keep what still blocks,
  add the new ones.
- Check that every watch-list commit has a verdict:
  ```bash
  grep -E '^[0-9a-f]{10} (PICKED|CLEAN|PRESENT|CONFLICT)' /tmp/upstream-triage.txt \
    | awk '{print $1}' | while read s; do
        grep -q "$s" docs/UPSTREAM-SYNC-LOG.md || echo "MISSING $s"; done
  ```
- Write the reviewed upstream SHA into `.upstream-reviewed`.
- If a security fix landed outside the watch list, extend
  `scripts/dev/upstream-watch-paths.txt` to cover that surface.

Commit the log, `.upstream-reviewed` and any watch-list change together:
`docs(upstream): record the review of upstream up to <to10>`.

## 6. Deliver

`git pull --rebase --autostash && git push`, then check the CI runs with
`gh run list --repo WarrenBrowse/warren-app` (without `--repo`, `gh` reads
Mullvad's CI). Report to poka: the picks with their commits, the open items and
what blocks each, and a one-line summary of what was left and why.

Never push upstream refs or tags to `origin`: a tag on an upstream commit would
publish upstream's whole history into this repo.
