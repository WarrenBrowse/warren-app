#!/usr/bin/env bash
# Tests for the Linux package maintainer scripts that stop the running daemon
# before an upgrade (dist-assets/linux/before-install.sh for the desktop
# package, dist-assets/linux/daemon/preinst for the headless one).
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
case "\$1" in is-system-running) echo running ;; esac
exit 0
EOF2
chmod +x "$TMP/bin/systemctl"
for tool in grep rm cp; do
	ln -s "$(command -v "$tool")" "$TMP/bin/$tool"
done

for script in dist-assets/linux/before-install.sh dist-assets/linux/daemon/preinst; do
	checks=$((checks + 1))
	rm -f "$TMP/calls"
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

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
