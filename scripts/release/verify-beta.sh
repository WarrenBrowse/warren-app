#!/usr/bin/env bash
#
# Warren VPN, pre-tag verification script.
#
# Run this BEFORE pushing a v*.*.* tag. The script runs every check that
# `.github/workflows/release.yml` would catch but on the local machine, so the
# tag push isn't burned on a broken commit.
#
# Exits 0 on green, non-zero on the first failure. Output is grouped by section.
#
# Usage:
#   bash scripts/release/verify-beta.sh
#
# Optional flags:
#   --skip-ios       Skip the iOS xcodebuild step (useful on Linux/Windows hosts)
#   --skip-android   Skip the Android Gradle step
#   --skip-bench     Skip the smoke-build.sh step (~3 min)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

SKIP_IOS=0
SKIP_ANDROID=0
SKIP_BENCH=0
for arg in "$@"; do
    case "$arg" in
        --skip-ios) SKIP_IOS=1 ;;
        --skip-android) SKIP_ANDROID=1 ;;
        --skip-bench) SKIP_BENCH=1 ;;
        *) echo "Unknown flag: $arg" >&2; exit 2 ;;
    esac
done

FAILURES=()

red() { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }
section() { printf '\n\033[1;34m== %s ==\033[0m\n' "$*"; }

record_fail() {
    FAILURES+=("$1")
    red "  FAIL: $1"
}

record_pass() {
    green "  PASS: $1"
}

run_or_record() {
    local label="$1"; shift
    if "$@" > /tmp/warren-verify-last.log 2>&1; then
        record_pass "$label"
    else
        record_fail "$label (see /tmp/warren-verify-last.log)"
    fi
}

section "Working tree state"

if [[ -n "$(git status --porcelain)" ]]; then
    record_fail "working tree not clean, commit or stash changes before tagging"
    git status --short
else
    record_pass "working tree clean"
fi

current_branch=$(git rev-parse --abbrev-ref HEAD)
if [[ "$current_branch" != "main" ]]; then
    record_fail "not on main branch (currently on $current_branch)"
else
    record_pass "on main branch"
fi

if git tag --list 'v0.1.0-beta.*' | grep -q .; then
    yellow "  NOTE: pre-existing v0.1.0-beta.* tag(s) found:"
    git tag --list 'v0.1.0-beta.*' | sed 's/^/    /'
fi

section "Pin warren-core matches HEAD"

if [[ ! -f .warren-core-version ]]; then
    record_fail ".warren-core-version missing"
else
    pinned=$(cat .warren-core-version)
    warren_core_path="${WARREN_CORE_PATH:-../warren-core}"
    if [[ -d "$warren_core_path/.git" ]]; then
        head_warren_core=$(git -C "$warren_core_path" rev-parse HEAD)
        if [[ "$pinned" == "$head_warren_core" ]]; then
            record_pass "pin $pinned matches warren-core HEAD"
        else
            record_fail "pin $pinned != warren-core HEAD $head_warren_core (bump pin or rollback warren-core)"
        fi
    else
        yellow "  SKIP: warren-core sibling repo not found at $warren_core_path"
    fi
fi

section "Rust workspace: warren-app"

run_or_record "cargo fmt --check (warren-app)" cargo fmt --all -- --check
run_or_record "cargo clippy -D warnings (warren-app)" cargo clippy --workspace --all-targets -- -D warnings
run_or_record "cargo test --workspace (warren-app)" cargo test --workspace --no-fail-fast

section "Rust workspace: warren-core"

if [[ -d "${WARREN_CORE_PATH:-../warren-core}" ]]; then
    pushd "${WARREN_CORE_PATH:-../warren-core}" > /dev/null
    run_or_record "cargo fmt --check (warren-core)" cargo fmt --all -- --check
    run_or_record "cargo clippy -D warnings (warren-core)" cargo clippy --workspace --all-targets -- -D warnings
    run_or_record "cargo test --workspace (warren-core)" cargo test --workspace --no-fail-fast
    if command -v cargo-deny > /dev/null 2>&1; then
        run_or_record "cargo deny check (warren-core)" cargo deny check
    else
        yellow "  SKIP: cargo-deny not installed (cargo install cargo-deny --locked)"
    fi
    popd > /dev/null
else
    yellow "  SKIP: warren-core sibling repo not found"
fi

section "Smoke build"

if [[ "$SKIP_BENCH" -eq 1 ]]; then
    yellow "  SKIP: smoke-build.sh (--skip-bench)"
else
    run_or_record "scripts/dev/smoke-build.sh" bash scripts/dev/smoke-build.sh
fi

section "Desktop UI"

if [[ -d desktop/packages/mullvad-vpn ]]; then
    run_or_record "npm install --no-audit (desktop)" npm --prefix desktop/packages/mullvad-vpn install --no-audit --no-fund
    run_or_record "npm run build (desktop)" npm --prefix desktop/packages/mullvad-vpn run build
    run_or_record "npm test (desktop)" npm --prefix desktop/packages/mullvad-vpn test --silent
else
    yellow "  SKIP: desktop directory missing"
fi

section "iOS"

if [[ "$SKIP_IOS" -eq 1 ]]; then
    yellow "  SKIP: iOS build (--skip-ios)"
elif [[ "$(uname)" != "Darwin" ]]; then
    yellow "  SKIP: iOS build (not running on macOS)"
elif [[ ! -d ios/WarrenVPN.xcodeproj ]]; then
    yellow "  SKIP: ios/WarrenVPN.xcodeproj missing"
else
    run_or_record "xcodebuild build -scheme WarrenVPN (iOS Simulator)" \
        xcodebuild build \
            -project ios/WarrenVPN.xcodeproj \
            -scheme WarrenVPN \
            -destination 'platform=iOS Simulator,name=iPhone 16 Pro' \
            -derivedDataPath ios/build/derived-data
fi

section "Android"

if [[ "$SKIP_ANDROID" -eq 1 ]]; then
    yellow "  SKIP: Android build (--skip-android)"
elif [[ ! -f android/gradlew ]]; then
    yellow "  SKIP: android/gradlew missing"
else
    run_or_record "./gradlew app:assembleDebug" \
        bash -c 'cd android && ./gradlew app:assembleDebug --parallel'
fi

section "Briefs + reports"

planning_dir=".planning"
if [[ -d "$planning_dir" ]]; then
    nogo_count=$(grep -lE 'NO-?GO' "$planning_dir"/session-*-report.md 2>/dev/null | wc -l)
    if [[ "$nogo_count" -gt 0 ]]; then
        record_fail "$nogo_count session reports show NO-GO verdict"
        grep -lE 'NO-?GO' "$planning_dir"/session-*-report.md 2>/dev/null | sed 's/^/    /'
    else
        record_pass "no NO-GO verdicts in session reports"
    fi
fi

section "Summary"

if [[ ${#FAILURES[@]} -eq 0 ]]; then
    green "ALL CHECKS PASSED, ready to tag"
    exit 0
else
    red "FAILURES: ${#FAILURES[@]}"
    for f in "${FAILURES[@]}"; do
        red "  - $f"
    done
    exit 1
fi
