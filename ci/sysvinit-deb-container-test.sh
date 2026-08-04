#!/usr/bin/env bash
# Runs INSIDE the systemd-less Debian container started by
# ci/test-sysvinit-deb.sh. Every step is a real dpkg transaction or a real
# daemon process: a lint of the maintainer scripts would not catch the failure
# this test exists for.
set -euo pipefail

DEB=/tmp/warren-sysvinit.deb
SYSTEMD_DEB=/tmp/warren-systemd.deb

fail() {
    echo "::error::$*" >&2
    dump_log
    exit 1
}

dump_log() {
    for log in /var/log/warren-vpn*/daemon.log; do
        [ -f "$log" ] || continue
        echo "----- tail of $log -----"
        tail -50 "$log"
    done
}

step() { echo; echo "=== $* ==="; }

# A container has no init as PID 1, so invoke-rc.d cannot read a runlevel and
# blocks every action ("No init system and policy-rc.d missing! Defaulting to
# block"), which would silently turn the daemon smoke test below into a no-op.
# A permissive policy-rc.d is the documented way to say "act anyway" (the usual
# container recipe is the deny-all inverse of this one). On a real MX host
# sysvinit is PID 1 and none of this applies.
cat > /usr/sbin/policy-rc.d << 'POLICY'
#!/bin/sh
exit 0
POLICY
chmod 755 /usr/sbin/policy-rc.d

# The slim image strips what any real desktop has: procps (the maintainer
# scripts' pkill, and this test's pgrep), iproute2 (the daemon shells out to
# `ip` for routing) and libdbus (the daemon's only unresolved shared library,
# which the package now declares as a dependency). Installing them keeps the
# test honest about the package rather than about the base image.
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq > /dev/null
apt-get install -y -qq procps iproute2 libdbus-1-3 > /dev/null

step "the container has no systemd"
if command -v systemctl > /dev/null 2>&1; then
    fail "systemctl exists in the test image; this test must run on a systemd-less host"
fi

package="$(dpkg-deb -f "$DEB" Package)"
case "$package" in
    warren-vpn-sysvinit) suffix="" ;;
    warren-vpn-*-sysvinit)
        suffix="-${package#warren-vpn-}"
        suffix="${suffix%-sysvinit}"
        ;;
    *) fail "unexpected package name '$package'" ;;
esac
daemon="warren-daemon${suffix}"
early="warren-early-boot-blocking${suffix}"
echo "package=$package daemon=$daemon"

step "the package declares the exclusive relations with the systemd flavour"
for field in Provides Conflicts Replaces; do
    value="$(dpkg-deb -f "$DEB" "$field")"
    [ "$value" = "warren-vpn${suffix}" ] \
        || fail "$field is '$value', expected 'warren-vpn${suffix}'"
done

step "the package ships no systemd unit"
if dpkg-deb -c "$DEB" | grep -q 'usr/lib/systemd/system/.*\.service'; then
    fail "the sysvinit package still ships systemd units"
fi

step "dpkg -i succeeds (this is what the systemd package fails at here)"
dpkg -i "$DEB" || fail "dpkg -i failed on a systemd-less host"

step "the package is fully configured"
state="$(dpkg-query -W -f='${Status}' "$package")"
[ "$state" = "install ok installed" ] || fail "package state is '$state'"

step "the init scripts are installed and enabled"
[ -x "/etc/init.d/$daemon" ] || fail "/etc/init.d/$daemon missing"
[ -x "/etc/init.d/$early" ] || fail "/etc/init.d/$early missing"
compgen -G "/etc/rc2.d/S??$daemon" > /dev/null \
    || fail "no start link for $daemon in the multi-user runlevels"
compgen -G "/etc/rcS.d/S??$early" > /dev/null \
    || fail "no start link for $early in rcS"

step "the daemon was started by the install and answers"
started=0
for _ in $(seq 1 30); do
    # The init script reports on the supervisor; the daemon process itself is
    # the thing that must be up, and a crash-looping daemon leaves a perfectly
    # healthy supervisor behind.
    if "/etc/init.d/$daemon" status > /dev/null 2>&1 && pgrep -f -- "/usr/bin/$daemon" > /dev/null; then
        started=1
        break
    fi
    sleep 1
done
[ "$started" -eq 1 ] || fail "the daemon did not come up after the install"

answered=0
for _ in $(seq 1 30); do
    if "/usr/bin/warren${suffix}" status > /dev/null 2>&1; then
        answered=1
        break
    fi
    sleep 1
done
[ "$answered" -eq 1 ] || fail "the daemon never answered on its management socket"
echo "warren status: $("/usr/bin/warren${suffix}" status 2>&1 | sed -n 1p)"

# `pgrep | head -1` would die of SIGPIPE under `pipefail` and abort this script
# with no message at all, so the first pid is taken from the captured output.
first_daemon_pid() {
    local pids
    pids="$(pgrep -f -- "/usr/bin/$daemon" || true)"
    printf '%s' "${pids%%$'\n'*}"
}

step "the supervisor respawns a crashed daemon"
pid="$(first_daemon_pid)"
[ -n "$pid" ] || fail "no $daemon process found"
kill -9 "$pid"
respawned=0
for _ in $(seq 1 30); do
    new_pid="$(first_daemon_pid)"
    if [ -n "$new_pid" ] && [ "$new_pid" != "$pid" ]; then
        respawned=1
        break
    fi
    sleep 1
done
[ "$respawned" -eq 1 ] || fail "the supervisor did not respawn the daemon after a crash"

step "stop leaves no daemon behind"
"/etc/init.d/$daemon" stop || fail "init script stop failed"
sleep 1
if pgrep -f -- "/usr/bin/$daemon" > /dev/null; then
    fail "a $daemon process outlived the stop"
fi

step "start again"
"/etc/init.d/$daemon" start || fail "init script start failed"
sleep 2
"/etc/init.d/$daemon" status > /dev/null || fail "the daemon is not running after a restart"

if [ "${WARREN_TEST_HAS_SYSTEMD_DEB:-0}" = "1" ] && [ -f "$SYSTEMD_DEB" ]; then
    step "the systemd flavour cannot co-install"
    if dpkg -i "$SYSTEMD_DEB" > /tmp/conflict.log 2>&1; then
        fail "the systemd flavour installed alongside the sysvinit one"
    fi
    grep -qi 'conflict' /tmp/conflict.log \
        || fail "dpkg refused the systemd flavour for the wrong reason: $(tail -3 /tmp/conflict.log)"
    echo "dpkg refused it, as declared"
fi

step "remove drops the rc.d links"
dpkg -r "$package" || fail "dpkg -r failed"
if compgen -G "/etc/rc?.d/[SK]??$daemon" > /dev/null; then
    fail "rc.d links survived the removal"
fi

step "reinstall then purge leaves nothing behind"
dpkg -i "$DEB" || fail "reinstall failed"
dpkg -P "$package" || fail "dpkg -P failed"
[ ! -d "/etc/warren-vpn${suffix}" ] || fail "/etc/warren-vpn${suffix} survived the purge"
[ ! -d "/var/log/warren-vpn${suffix}" ] || fail "/var/log/warren-vpn${suffix} survived the purge"

echo
echo "sysvinit package OK on a systemd-less host"
