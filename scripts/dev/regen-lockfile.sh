#!/usr/bin/env bash
#
# Regenerate Cargo.lock against the PINNED sibling SHAs, exactly as CI resolves.
#
# Why this exists: the fork consumes warrenguard / warren-sdk-rs / warren-contract
# by `path` from sibling checkouts, but CI checks those siblings out at the SHAs
# in .warrenguard-version / .warren-sdk-version / .warren-contract-version. A dev
# machine usually has the siblings at their branch HEAD, which is newer than the
# pins. Running a plain `cargo update -w` there resolves against HEAD and bakes
# crates that the pinned siblings do not pull (e.g. the anonymous-token rsa /
# blind-rsa tree), producing a Cargo.lock that the CI coherence gate rejects with
# "Cargo.lock is out of sync with the workspace" (cargo metadata --locked fails).
#
# This script rebuilds the lock the way CI sees it: it materialises throwaway git
# worktrees of each sibling at its pin (leaving your real sibling checkouts
# untouched), regenerates the lock there, verifies --locked passes and the quinn
# fork stays pinned, then copies the lock back. Run it after bumping any
# .warren*-version pin, then commit the resulting Cargo.lock.
#
# Usage: scripts/dev/regen-lockfile.sh
set -euo pipefail

app_root="$(cd "$(git rev-parse --show-toplevel)" && pwd)"
parent="$(cd "$app_root/.." && pwd)"

read_pin() { tr -d '[:space:]' < "$app_root/$1"; }
guard_sha="$(read_pin .warrenguard-version)"
sdk_sha="$(read_pin .warren-sdk-version)"
contract_sha="$(read_pin .warren-contract-version)"

# repo dir name -> pinned sha
siblings=(
  "warrenguard:$guard_sha"
  "warren-sdk-rs:$sdk_sha"
  "warren-contract:$contract_sha"
)

for pair in "${siblings[@]}"; do
  name="${pair%%:*}"
  if [[ ! -d "$parent/$name/.git" ]]; then
    echo "error: sibling repo not found at $parent/$name (run 'mani sync' first)" >&2
    exit 1
  fi
done

work="$(mktemp -d "${TMPDIR:-/tmp}/warren-relock.XXXXXX")"
created=()
cleanup() {
  for pair in "${siblings[@]}"; do
    name="${pair%%:*}"
    git -C "$parent/$name" worktree remove --force "$work/$name" 2>/dev/null || true
    git -C "$parent/$name" worktree prune 2>/dev/null || true
  done
  git -C "$app_root" worktree remove --force "$work/warren-app" 2>/dev/null || true
  git -C "$app_root" worktree prune 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

echo "==> materialising pinned siblings under $work"
for pair in "${siblings[@]}"; do
  name="${pair%%:*}"; sha="${pair##*:}"
  git -C "$parent/$name" worktree add --detach "$work/$name" "$sha" -q
  echo "    $name @ ${sha:0:12}"
done
git -C "$app_root" worktree add --detach "$work/warren-app" HEAD -q
echo "    warren-app @ HEAD"

echo "==> regenerating Cargo.lock against the pins"
(
  cd "$work/warren-app"
  export CARGO_NET_GIT_FETCH_WITH_CLI=true
  # `cargo metadata` (not `cargo generate-lockfile`) is deliberate: it re-resolves
  # conservatively, pruning entries the pinned siblings no longer pull and adding
  # any they need, WITHOUT upgrading the rest of the tree to the newest crates.io
  # versions. generate-lockfile would bump every dependency, sneaking unrelated
  # upgrades into what should be a pin-alignment-only change.
  cargo metadata --format-version=1 >/dev/null
  cargo metadata --locked --format-version=1 >/dev/null
  grep -q 'warren-quinn' Cargo.lock \
    || { echo "error: regenerated lock does not pin the warren-quinn fork" >&2; exit 1; }
)

cp "$work/warren-app/Cargo.lock" "$app_root/Cargo.lock"
echo "==> Cargo.lock updated. Review 'git diff Cargo.lock' and commit it."
