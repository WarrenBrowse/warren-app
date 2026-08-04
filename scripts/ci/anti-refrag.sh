#!/bin/sh
# Anti-refragmentation gate for warren-app (doc 94, warren-core/docs/94-DEDUP-AUDIT).
#
# Each rule detects a RESURRECTED twin definition of a responsibility that has a
# single home, while ALLOWING re-exports, imports and calls of that single home.
# Test files and android/** are excluded (android is out of scope and has UI
# work in flight). A line containing the token `anti-refrag:allow` is ignored,
# so a legitimate exception can be whitelisted inline.
#
# Fast by construction: pure greps, no cargo build.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$ROOT"

ESCAPE='anti-refrag:allow'
status=0

note() { printf '%s\n' "$*" >&2; }

violation() {
  # $1 = rule id, $2 = doc-94 ref, $3 = explanation
  note ""
  note "ANTI-REFRAG VIOLATION [$1] (doc-94 $2)"
  note "  $3"
  status=1
}

# R-D4: single-home the product User-Agent. warren-app must send
# warren_contract::product::USER_AGENT, never define a local `const/static
# USER_AGENT` twin nor hand-write the bare "warren-app" user-agent literal.
# Imports (`use ...::USER_AGENT`) and references are re-exports/calls, allowed.
d4_const=$(grep -rnE '(const|static)[[:space:]]+USER_AGENT[[:space:]]*:' --include='*.rs' . 2>/dev/null \
  | grep -vE '/android/' | grep -vE '/tests?/' | grep -v "$ESCAPE" || true)
d4_literal=$(grep -rn '"warren-app"' --include='*.rs' . 2>/dev/null \
  | grep -vE '/android/' | grep -vE '/tests?/' | grep -v "$ESCAPE" || true)
if [ -n "$d4_const" ]; then note "$d4_const"; fi
if [ -n "$d4_literal" ]; then note "$d4_literal"; fi
if [ -n "$d4_const" ] || [ -n "$d4_literal" ]; then
  violation "R-D4" "D4 (user-agent single-home)" \
    "consume warren_contract::product::USER_AGENT; do not define a local USER_AGENT const/static or a bare \"warren-app\" user-agent literal in warren-app Rust."
fi

# R-A6/48: dependency direction. warren-app is 100% off warren-core (the private
# backend), not even in tests. Fail if any Cargo.toml declares warren-core as a
# dependency (dep-key line, `.workspace`/`.path` shorthand, or a
# `[...dependencies.warren-core]` table).
a6=$(grep -rnE '^warren-core[[:space:]]*[=.]|dependencies\.warren-core\]' --include='Cargo.toml' . 2>/dev/null \
  | grep -vE '/android/' | grep -v "$ESCAPE" || true)
if [ -n "$a6" ]; then
  note "$a6"
  violation "R-A6/48" "A6 / doc 48 (dependency direction)" \
    "a warren-app Cargo.toml declares a warren-core dependency; warren-app must not depend on warren-core."
fi

if [ "$status" -ne 0 ]; then
  note ""
  note "Anti-refragmentation gate FAILED (see warren-core/docs/94-DEDUP-AUDIT)."
  exit 1
fi
printf 'anti-refrag: all rules passed\n'
