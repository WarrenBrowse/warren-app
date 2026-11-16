#!/usr/bin/env bash
# Tests for the preinstall scripts of the systemd Linux packages
# (dist-assets/linux/before-install.sh for the desktop package,
# dist-assets/linux/daemon/preinst for the headless one): they stop the running
# daemon before an upgrade, and refuse a host that does not run systemd.
#
#   bash ci/test-linux-maintainer-scripts.sh
#
# Cloud and minimal images of Fedora, openSUSE and Arch ship without `which`.
# A script that looked for systemctl through it skipped the whole stop branch
# there, so an upgrade left the previous daemon running on a deleted binary
# until the next reboot (measured 2026-09-25 on Fedora 43, openSUSE Leap 16 and
# Arch Linux ARM VMs). The scripts are run here against a PATH holding a fake
# systemctl and no `which`, and must reach the daemon handover.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0
checks=0

mkdir "$TMP/bin"
cat > "$TMP/bin/systemctl" << EOF2
#!/bin/sh
echo "systemctl \$*" >> "$TMP/calls"
case "\$1" in is-system-running) cat "$TMP/state" ;; esac
exit 0
EOF2
chmod +x "$TMP/bin/systemctl"
for tool in grep rm cp cat; do
	ln -s "$(command -v "$tool")" "$TMP/bin/$tool"
done

for script in dist-assets/linux/before-install.sh dist-assets/linux/daemon/preinst; do
	checks=$((checks + 1))
	rm -f "$TMP/calls"
	echo running > "$TMP/state"
	PATH="$TMP/bin" "$(command -v bash)" "$SCRIPT_DIR/../$script" > "$TMP/out" 2>&1
	# Asking systemd for the daemon's state is the first step of the branch
	# that hands the machine over and stops it.
	if grep -q "systemctl status" "$TMP/calls" 2> /dev/null; then
		printf '  ok   %s reaches the daemon handover without `which`\n' "$script"
	else
		failures=$((failures + 1))
		printf '  FAIL %s skips the daemon handover without `which`\n' "$script"
	fi
done

# Both postinstalls enable and start the daemon through systemctl. On a host
# whose init is not systemd (MX Linux, antiX, Devuan) that either fails half
# way, leaving dpkg with a half-configured package, or succeeds without the
# daemon ever starting, and the app then waits forever for its system service
# (reported 2026-11-16). The preinstall must refuse such a host before anything
# is unpacked, and say what to install instead.
refuses_host() { # refuses_host <script> <description> <expected advice>
	local script="$1" description="$2" advice="$3"
	checks=$((checks + 1))
	rm -f "$TMP/calls"
	if PATH="$TMP/bin" "$(command -v bash)" "$SCRIPT_DIR/../$script" > "$TMP/out" 2>&1; then
		failures=$((failures + 1))
		printf '  FAIL %s installs on a host %s\n' "$script" "$description"
	elif ! grep -q -- "$advice" "$TMP/out"; then
		failures=$((failures + 1))
		printf '  FAIL %s refuses a host %s without naming %s\n' "$script" "$description" "$advice"
	else
		printf '  ok   %s refuses a host %s and names %s\n' "$script" "$description" "$advice"
	fi
}

for case in "dist-assets/linux/before-install.sh -sysvinit.deb" \
	"dist-assets/linux/daemon/preinst install.sh"; do
	read -r script advice <<< "$case"
	echo offline > "$TMP/state"
	refuses_host "$script" "where systemd is installed but is not the init" "$advice"
	mv "$TMP/bin/systemctl" "$TMP/systemctl.away"
	refuses_host "$script" "without systemctl" "$advice"
	mv "$TMP/systemctl.away" "$TMP/bin/systemctl"
done

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
