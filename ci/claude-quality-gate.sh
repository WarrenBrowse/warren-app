#!/usr/bin/env bash
# Claude Code quality gate (Stop hook).
#
# Fires when Claude Code finishes a turn. It runs the SAME checks CI runs
# (ci/cargo-ci.sh => RUSTFLAGS=--deny warnings, --locked), but SCOPED to the
# crates whose .rs files changed in the working tree, so the loop stays fast on
# this 40-plus-crate Mullvad-scale workspace. If anything is red it exits 2 and
# prints the failures: Claude Code feeds that back and keeps iterating until the
# gate is green. "Green locally" therefore matches "green in CI".
#
# Escape hatches:
#   WARREN_CLAUDE_SKIP_GATE=1    skip entirely (WIP / large refactor in flight)
#   WARREN_CLAUDE_GATE_TESTS=0   run fmt + clippy only (skip the slower test step)
#
# Exit codes: 0 = clean / nothing to check, 2 = blocked (failures on stderr).
set -uo pipefail

# --- loop guard ------------------------------------------------------------
# The Stop hook receives JSON on stdin. When stop_hook_active is true we are
# already inside a forced continuation: bail out so we never loop forever.
stdin_json="$(cat 2>/dev/null || true)"
case "$stdin_json" in
  *'"stop_hook_active":true'*|*'"stop_hook_active": true'*) exit 0 ;;
esac

[ "${WARREN_CLAUDE_SKIP_GATE:-0}" = "1" ] && exit 0

root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$root" || exit 0

# --- collect changed Rust files (staged + unstaged + untracked) ------------
mapfile -t files < <(
  {
    git diff --name-only -- '*.rs'
    git diff --name-only --cached -- '*.rs'
    git ls-files --others --exclude-standard -- '*.rs'
  } 2>/dev/null | sort -u
)
[ "${#files[@]}" -eq 0 ] && exit 0

# --- map each file to the crate that owns it -------------------------------
crate_of() {
  local d; d="$(dirname "$1")"
  while [ -n "$d" ] && [ "$d" != "." ] && [ "$d" != "/" ]; do
    if [ -f "$d/Cargo.toml" ] && grep -q '^\[package\]' "$d/Cargo.toml"; then
      grep -m1 '^name' "$d/Cargo.toml" | sed -E 's/^name[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/'
      return 0
    fi
    d="$(dirname "$d")"
  done
}

declare -A seen=()
for f in "${files[@]}"; do
  [ -e "$f" ] || continue            # deleted file: nothing to check
  c="$(crate_of "$f")"
  [ -n "${c:-}" ] && seen["$c"]=1
done
[ "${#seen[@]}" -eq 0 ] && exit 0

pkgs=(); for c in "${!seen[@]}"; do pkgs+=(-p "$c"); done
crate_list="${!seen[*]}"

CI="$root/ci/cargo-ci.sh"
fails=""

run() { # label, cmd...
  local label="$1"; shift
  if ! out="$("$@" 2>&1)"; then
    fails+=$'\n'"### ${label} FAILED:"$'\n'"$out"$'\n'
  fi
}

# fmt is cheap; clippy + test are scoped to the changed crates.
run "rustfmt --check" "$CI" fmt "${pkgs[@]}" -- --check
run "clippy (deny warnings)" "$CI" clippy "${pkgs[@]}" --all-targets
if [ "${WARREN_CLAUDE_GATE_TESTS:-1}" != "0" ]; then
  run "cargo test" "$CI" test "${pkgs[@]}"
fi

if [ -n "$fails" ]; then
  {
    echo "Quality gate is RED for changed crate(s): ${crate_list}"
    echo "Fix the failures below, then finish again (the gate re-runs automatically)."
    echo "$fails"
  } >&2
  exit 2
fi

echo "Quality gate green for: ${crate_list}" >&2
exit 0
