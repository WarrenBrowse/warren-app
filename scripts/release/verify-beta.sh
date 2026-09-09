#!/usr/bin/env bash
#
# Warren VPN, pre-tag verification script.
#
# Run this BEFORE pushing a v*.*.* tag. The script runs every check that
# `.github/workflows/release.yml` would catch but on the local machine, so the
# tag push isn't burned on a broken commit.
#
# Runs every check, then exits non-zero if any FAILED. Output is grouped by
# section, and each check keeps its own log file (they used to share one, so
# the second failure erased the evidence of the first).
#
# Usage:
#   bash scripts/release/verify-beta.sh
#
# Optional flags:
#   --skip-ios       Skip the iOS xcodebuild step (useful on Linux/Windows hosts)
#   --skip-android   Skip the Android Gradle step
#   --skip-bench     Skip the smoke-build.sh step (~3 min)
#
# A FAIL HERE MUST MEAN THE COMMIT IS BROKEN. On 2026-09-09 this script
# reported five failures on a release that was fine, and four of them were the
# script's own drift: a clippy scope wider than CI's (so a Linux-only crate's
# code reads as dead on macOS), an `npm --prefix` into a workspace member (which
# cannot resolve the sibling packages), and a hardcoded `iPhone 16 Pro`
# simulator that Xcode no longer ships. A checklist that cries wolf is a
# checklist people stop running, so:
#
#   - a check whose TOOLCHAIN is absent reports SKIP with the reason, never
#     FAIL, because "no JDK on this Mac" says nothing about the commit;
#   - a check is scoped exactly as its CI counterpart, so local and CI verdicts
#     cannot disagree;
#   - nothing hardcodes an environment-specific name (a simulator, a version)
#     that a tool update can retire under it.

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
SKIPPED=()

# One log per check. A single shared file meant the last failure overwrote the
# evidence for every earlier one, which is useless precisely when several fail.
LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/warren-verify-XXXXXX")"

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

# A missing toolchain is not a broken commit. It is recorded, reported at the
# end so nobody mistakes a partial run for a full one, and never a failure.
record_skip() {
    SKIPPED+=("$1: $2")
    yellow "  SKIP: $1 ($2)"
}

log_path() {
    printf '%s/%s.log' "$LOG_DIR" \
        "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | tr -cs '[:alnum:]' '-')"
}

run_or_record() {
    local label="$1"; shift
    local log
    log="$(log_path "$label")"
    if "$@" > "$log" 2>&1; then
        record_pass "$label"
    else
        record_fail "$label (see $log)"
    fi
}

# Runs a check only when every named command exists, so a machine without the
# toolchain reports SKIP instead of a failure that reads as a code problem.
run_if_available() {
    local label="$1" tools="$2"; shift 2
    local tool
    for tool in $tools; do
        if ! command -v "$tool" > /dev/null 2>&1; then
            record_skip "$label" "$tool not found"
            return 0
        fi
    done
    run_or_record "$label" "$@"
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

section "quinn fork is locked"

# A silent re-resolve to upstream quinn would drop the GSO/obfuscation patches.
if grep -q 'warren-quinn' Cargo.lock; then
    record_pass "Cargo.lock pins the warren-quinn fork"
else
    record_fail "Cargo.lock does not pin the warren-quinn fork (regenerate with the fork present)"
fi

section "Rust workspace: warren-app"

run_if_available "cargo fmt --check (warren-app)" cargo cargo fmt --all -- --check

# `warren-nm-vpn-service` is the NetworkManager plugin: every dependency it has
# is behind `cfg(target_os = "linux")`, so off Linux its whole config module is
# unreachable and `-D warnings` turns 17 dead-code lints into errors on code
# nobody touched (it has not changed since the 2026-08-04 squash). CI never sees
# this because its clippy step lists the crates explicitly and this is not among
# them. Excluding it off Linux keeps the verdict identical to CI's while still
# linting it where it actually builds.
clippy_args=(--workspace --all-targets)
if [[ "$(uname -s)" != "Linux" ]]; then
    clippy_args+=(--exclude warren-nm-vpn-service)
    yellow "  NOTE: warren-nm-vpn-service is Linux-only, excluded from clippy on $(uname -s)"
fi
run_if_available "cargo clippy -D warnings (warren-app)" cargo \
    cargo clippy "${clippy_args[@]}" -- -D warnings
run_if_available "cargo test --workspace (warren-app)" cargo \
    cargo test --workspace --no-fail-fast

section "Release notes"

# The three surfaces that show release notes read one file, and the bundled
# what's-new screen is the one that silently went stale for 25 releases.
run_or_record "release-notes reader (ci/test-extract-changelog-section.sh)" \
    bash ci/test-extract-changelog-section.sh
run_or_record "changes.txt matches CHANGELOG.md for the committed version" \
    bash scripts/release/generate-changes-txt.sh --check

section "Smoke build"

if [[ "$SKIP_BENCH" -eq 1 ]]; then
    yellow "  SKIP: smoke-build.sh (--skip-bench)"
else
    run_or_record "scripts/dev/smoke-build.sh" bash scripts/dev/smoke-build.sh
fi

section "Desktop UI"

if [[ ! -d desktop/packages/mullvad-vpn ]]; then
    record_skip "desktop UI" "desktop/packages/mullvad-vpn missing"
elif ! command -v npm > /dev/null 2>&1; then
    record_skip "desktop UI" "npm not found"
else
    # `npm ci` at the WORKSPACE ROOT (`desktop/`), exactly as CI does, for two
    # reasons.
    #
    # The root, never `--prefix desktop/packages/mullvad-vpn`: mullvad-vpn
    # depends on the sibling workspace packages (`management-interface`,
    # `windows-utils`, `nseventforwarder`), which exist only as workspace
    # links. Installing the member on its own sends npm to the public registry
    # for them and fails with `404 Not Found - GET .../management-interface`,
    # which reads as a broken dependency rather than a wrong directory.
    #
    # `ci`, never `install`: this npm rewrites the lockfile's legacy
    # `dependencies` block on every install (it turns handlebars' `version`
    # into a URL and pins a caret range), so a verification run left the tree
    # dirty and the next check reported "working tree not clean". `npm ci`
    # installs from the lockfile without writing to it, and fails when the
    # lockfile and package.json disagree, which is a signal worth having before
    # a tag.
    run_or_record "npm ci (desktop workspace root)" \
        npm --prefix desktop ci --no-audit --no-fund
    run_or_record "npm run build (desktop)" \
        npm --prefix desktop run build -w mullvad-vpn
    run_or_record "npm test (desktop)" \
        npm --prefix desktop test -w mullvad-vpn --silent
fi

section "iOS"

if [[ "$SKIP_IOS" -eq 1 ]]; then
    record_skip "iOS build" "--skip-ios"
elif [[ "$(uname)" != "Darwin" ]]; then
    record_skip "iOS build" "not running on macOS"
elif [[ ! -d ios/WarrenVPN.xcodeproj ]]; then
    record_skip "iOS build" "ios/WarrenVPN.xcodeproj missing"
elif ! command -v xcodebuild > /dev/null 2>&1; then
    record_skip "iOS build" "xcodebuild not found (Command Line Tools only?)"
else
    # Never name a simulator model. This step pinned `iPhone 16 Pro`, and an
    # Xcode update that ships the 17 family retired it, so the step failed with
    # "no available devices matched the request" on a project that builds fine.
    # Ask the installed Xcode for a booted-or-available iOS simulator instead:
    # any iOS simulator proves the target compiles, which is all this check is
    # for.
    ios_destination="$(
        xcodebuild -project ios/WarrenVPN.xcodeproj -scheme WarrenVPN \
            -showdestinations 2>/dev/null |
            sed -n 's/.*{ *platform:iOS Simulator, *\(.*\)}.*/\1/p' |
            grep -v 'placeholder' |
            sed -n 's/.*id:\([0-9A-Fa-f-]*\).*/\1/p' |
            head -1
    )"
    if [[ -z "$ios_destination" ]]; then
        record_skip "iOS build" "no iOS simulator runtime installed"
    else
        run_or_record "xcodebuild build -scheme WarrenVPN (iOS Simulator)" \
            xcodebuild build \
                -project ios/WarrenVPN.xcodeproj \
                -scheme WarrenVPN \
                -destination "platform=iOS Simulator,id=$ios_destination" \
                -derivedDataPath ios/build/derived-data
    fi
fi

section "Android"

if [[ "$SKIP_ANDROID" -eq 1 ]]; then
    record_skip "Android build" "--skip-android"
elif [[ ! -f android/gradlew ]]; then
    record_skip "Android build" "android/gradlew missing"
elif ! /usr/bin/env java -version > /dev/null 2>&1; then
    # Gradle's own failure for this is "Unable to locate a Java Runtime", which
    # is a property of the machine and not of the commit. CI covers Android on
    # every push (android-checks.yml), so a Mac with no JDK is a skip.
    record_skip "Android build" "no Java runtime (CI covers Android on push)"
else
    run_or_record "./gradlew app:assembleBetaRelease" \
        bash -c 'cd android && ./gradlew app:assembleBetaRelease --parallel'
fi

section "Summary"

if [[ ${#SKIPPED[@]} -gt 0 ]]; then
    yellow "SKIPPED: ${#SKIPPED[@]} (not verified here, so not verified at all unless CI covers it)"
    for s in "${SKIPPED[@]}"; do
        yellow "  - $s"
    done
fi

if [[ ${#FAILURES[@]} -eq 0 ]]; then
    green "ALL CHECKS PASSED, ready to tag"
    printf 'logs: %s\n' "$LOG_DIR"
    exit 0
else
    red "FAILURES: ${#FAILURES[@]}"
    for f in "${FAILURES[@]}"; do
        red "  - $f"
    done
    printf 'logs: %s\n' "$LOG_DIR"
    exit 1
fi
