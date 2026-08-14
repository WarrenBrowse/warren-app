#!/usr/bin/env bash
#
# Behavioural tests for dist-assets/pkg-scripts/preinstall, the macOS installer
# script that seals the host before an upgrade swaps the app bundle.
#
# Nothing else gates that file. It only ever runs inside a real pkg install, so
# the one regression it can carry, aborting an install that should have
# proceeded, stays invisible until a user double-clicks the installer. On
# 2026-08-14 exactly that shipped: every FIRST install of the macOS pkg failed,
# because the fatal seal ran against the PREVIOUS bundle's warren-setup, which a
# machine without a previous install does not have.
#
# The script under test is the real one, run against stubbed system commands so
# that a developer's machine is never touched: `rm` refuses any operand outside
# the sandbox, and sudo/launchctl/pgrep/pkill are inert.

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
PREINSTALL="$REPO_ROOT/dist-assets/pkg-scripts/preinstall"

failures=0
sandbox_root=$(mktemp -d)
trap 'rm -rf "$sandbox_root"' EXIT

# Builds a sandbox holding the stub PATH, the log dir and a fake /Applications.
# Pass the exit status the fake warren-setup must return for prepare-restart;
# 3 is its DaemonNotRunning status (mullvad-setup/src/main.rs), which is what an
# upgrade sees when no daemon is left to answer.
make_sandbox() {
    local name=$1 prepare_restart_status=$2 with_old_bundle=$3
    local dir="$sandbox_root/$name"

    mkdir -p "$dir/Applications" "$dir/bin" "$dir/log"
    : > "$dir/setup-calls"

    cat > "$dir/bin/rm" <<'STUB'
#!/usr/bin/env bash
# The script under test removes cache files under absolute /Library paths that
# belong to a real install on this machine. Delete only inside the sandbox.
flags=()
operands=()
for arg in "$@"; do
    case "$arg" in
        -*) flags+=("$arg") ;;
        "$WARREN_TEST_SANDBOX"/*) operands+=("$arg") ;;
        *) echo "$arg" >> "$WARREN_TEST_SANDBOX/rm-refused" ;;
    esac
done
[ ${#operands[@]} -eq 0 ] && exit 0
exec /bin/rm ${flags[@]+"${flags[@]}"} "${operands[@]}"
STUB

    cat > "$dir/bin/sudo" <<'STUB'
#!/usr/bin/env bash
[ "$1" = "-u" ] && shift 2
exec "$@"
STUB

    cat > "$dir/bin/launchctl" <<'STUB'
#!/usr/bin/env bash
echo "$*" >> "$WARREN_TEST_SANDBOX/launchctl-calls"
STUB

    # No GUI is running, which keeps the script off the root-owned /var/run
    # marker that a shell redirect writes and no stub could intercept.
    printf '#!/usr/bin/env bash\nexit 1\n' > "$dir/bin/pgrep"
    printf '#!/usr/bin/env bash\nexit 1\n' > "$dir/bin/pkill"

    chmod +x "$dir/bin/"*

    if [ "$with_old_bundle" = "with-old-bundle" ]; then
        local resources="$dir/Applications/Warren VPN.app/Contents/Resources"
        mkdir -p "$resources"
        cat > "$resources/warren-setup" <<STUB
#!/usr/bin/env bash
echo "\$1" >> "$dir/setup-calls"
[ "\$1" = "prepare-restart" ] && exit $prepare_restart_status
exit 0
STUB
        chmod +x "$resources/warren-setup"
    fi

    echo "$dir"
}

# Runs the real preinstall the way the macOS installer does: $2 is the install
# directory. Echoes the exit status.
run_preinstall() {
    local dir=$1 status=0

    # The script traces to stderr until it redirects itself into preinstall.log,
    # so keep that prologue out of the test output and next to the log.
    env -i \
        PATH="$dir/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
        HOME="$dir" \
        WARREN_TEST_SANDBOX="$dir" \
        WARREN_LOG_DIR="$dir/log" \
        bash "$PREINSTALL" "$dir/package.pkg" "$dir/Applications" \
        2> "$dir/prologue.log" || status=$?

    echo "$status"
}

report() {
    local label=$1 expected=$2 actual=$3 dir=$4
    if [ "$expected" = "$actual" ]; then
        echo "ok - $label"
        return
    fi
    echo "FAIL - $label: expected exit $expected, got $actual"
    echo "--- preinstall.log ---"
    cat "$dir/log/preinstall.log" 2>/dev/null || echo "(no log)"
    echo "----------------------"
    failures=$((failures + 1))
}

assert_setup_called() {
    local label=$1 dir=$2 subcommand=$3
    if grep -qx "$subcommand" "$dir/setup-calls" 2>/dev/null; then
        echo "ok - $label"
    else
        echo "FAIL - $label: '$subcommand' was not called"
        failures=$((failures + 1))
    fi
}

assert_setup_not_called() {
    local label=$1 dir=$2 subcommand=$3
    if grep -qx "$subcommand" "$dir/setup-calls" 2>/dev/null; then
        echo "FAIL - $label: '$subcommand' was called"
        failures=$((failures + 1))
    else
        echo "ok - $label"
    fi
}

# A machine with no previous install has no bundle to seal and no daemon owning
# the host, so there is nothing the guard could protect and the install must
# proceed. This is the case that broke: the fatal prepare-restart cannot succeed
# against a binary that does not exist.
dir=$(make_sandbox fresh-install 0 no-old-bundle)
report "a first install proceeds when no previous bundle exists" 0 "$(run_preinstall "$dir")" "$dir"
assert_setup_not_called "a first install seals nothing" "$dir" prepare-restart
assert_setup_not_called "a first install arms no guard" "$dir" arm-deadman

# An upgrade whose daemon is already gone is the state the seal was trying to
# reach, not a failure. Windows tolerates the same status (MVSETUP_DAEMON_NOT_
# RUNNING in dist-assets/windows/installer.nsh).
dir=$(make_sandbox daemon-not-running 3 with-old-bundle)
report "an upgrade proceeds when no daemon is left to answer" 0 "$(run_preinstall "$dir")" "$dir"
assert_setup_not_called "a tolerated status leaves the guard armed" "$dir" disarm-deadman

# The guard this file exists for: the host could not be sealed, so the install
# must not go on to delete the app and kill the daemon.
dir=$(make_sandbox seal-failed 1 with-old-bundle)
report "an upgrade aborts when the host could not be sealed" 1 "$(run_preinstall "$dir")" "$dir"
assert_setup_called "an aborted install retires the guard" "$dir" disarm-deadman

# The nominal upgrade, which must still arm the guard and unload the old daemon.
dir=$(make_sandbox upgrade 0 with-old-bundle)
report "a nominal upgrade proceeds" 0 "$(run_preinstall "$dir")" "$dir"
assert_setup_called "a nominal upgrade arms the guard" "$dir" arm-deadman
assert_setup_called "a nominal upgrade seals the host" "$dir" prepare-restart
if [ -e "$dir/Applications/Warren VPN.app" ]; then
    echo "FAIL - a nominal upgrade removes the old bundle"
    failures=$((failures + 1))
else
    echo "ok - a nominal upgrade removes the old bundle"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures assertion(s) failed"
    exit 1
fi
echo "All preinstall assertions passed"
