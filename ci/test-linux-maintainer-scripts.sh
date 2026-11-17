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

# The upgrade seals the host (`warren-setup prepare-restart`) before it stops
# the daemon: a stop without the seal reads as a user stop and opens the
# firewall until the new daemon starts. So a seal that fails must abort the
# upgrade before the stop, a daemon that is already gone (warren-setup's exit
# status 3) leaves nothing to seal, and the systemd flavours arm the detached
# update guard before the seal and retire it when they abort. Each script is
# staged with its installed paths pointed at fakes that record their calls.
mkdir "$TMP/init.d"
cat > "$TMP/setup" << EOF2
#!/bin/sh
echo "setup \$1" >> "$TMP/calls"
case "\$1" in prepare-restart) exit "\$(cat "$TMP/seal")" ;; esac
exit 0
EOF2
cat > "$TMP/init.d/warren-daemon" << EOF2
#!/bin/sh
echo "init \$1" >> "$TMP/calls"
EOF2
chmod +x "$TMP/setup" "$TMP/init.d/warren-daemon"

stage() { # stage <script>: copy with the installed paths pointed at the fakes
	sed -e "s#/opt/Warren\\\\ VPN/resources/warren-setup#$TMP/setup#g" \
		-e "s#/opt/Warren VPN/resources/warren-setup#$TMP/setup#g" \
		-e "s#@WARREN_PRODUCT_DIR@/resources/warren-setup#$TMP/setup#g" \
		-e "s#@WARREN_SUFFIX@##g" \
		-e "s#/etc/init.d/#$TMP/init.d/#g" \
		"$SCRIPT_DIR/../$1" > "$TMP/staged"
}

seal_case() { # seal_case <script> <interpreter> <stop call> <arms guard> <seal status> <expect>
	local script="$1" shell="$2" stop="$3" arms="$4" seal="$5" expect="$6" result=ok
	checks=$((checks + 1))
	rm -f "$TMP/calls"
	echo running > "$TMP/state"
	echo "$seal" > "$TMP/seal"
	stage "$script"
	local status=0
	PATH="$TMP/bin" "$(command -v "$shell")" "$TMP/staged" > "$TMP/out" 2>&1 || status=$?
	local calls
	calls="$(cat "$TMP/calls" 2> /dev/null)"
	case "$expect" in
	abort)
		[ "$status" -ne 0 ] || result="installs on"
		! grep -qx "$stop" <<< "$calls" || result="stops the daemon after"
		if [ "$arms" = yes ] && ! grep -qx "setup disarm-deadman" <<< "$calls"; then
			result="leaves the update guard armed after"
		fi
		;;
	proceed)
		[ "$status" -eq 0 ] || result="aborts on"
		grep -qx "$stop" <<< "$calls" || result="never stops the daemon on"
		;;
	esac
	if [ "$arms" = yes ] && [ "$(grep -m1 -E '^setup (arm-deadman|prepare-restart)$' <<< "$calls")" != "setup arm-deadman" ]; then
		result="does not arm the update guard before the seal on"
	fi
	if [ "$result" = ok ]; then
		printf '  ok   %s, seal status %s: %s\n' "$script" "$seal" "$expect"
	else
		failures=$((failures + 1))
		printf '  FAIL %s %s a seal status %s\n' "$script" "$result" "$seal"
	fi
}

for case in "dist-assets/linux/before-install.sh bash systemctl_stop yes" \
	"dist-assets/linux/daemon/preinst bash systemctl_stop yes" \
	"dist-assets/linux/sysvinit/preinst sh init_stop no"; do
	read -r script shell stop arms <<< "$case"
	case "$stop" in
	systemctl_stop) stop="systemctl stop warren-daemon.service" ;;
	init_stop) stop="init stop" ;;
	esac
	seal_case "$script" "$shell" "$stop" "$arms" 1 abort
	seal_case "$script" "$shell" "$stop" "$arms" 3 proceed
	seal_case "$script" "$shell" "$stop" "$arms" 0 proceed
done

# The post-removal scripts. The settings directory holds the wallet, so only
# an explicit purge (`apt purge`) may delete it: `dnf remove` passes 0 and has
# no purge of its own, and erasing a paid identity on a plain removal cannot be
# undone. The AppArmor profile goes on a real removal only: on an rpm upgrade
# the old package's post-uninstall runs after the new package's post-install,
# and would take away the profile the new one just put in place.
cat > "$TMP/bin/apparmor_parser" << EOF2
#!/bin/sh
echo "apparmor_parser \$*" >> "$TMP/calls"
EOF2
chmod +x "$TMP/bin/apparmor_parser"

stage_postrm() { # stage_postrm <script>: system paths pointed under $TMP
	sed -e "s#/var/log/#$TMP/var/log/#g" \
		-e "s#/var/cache/#$TMP/var/cache/#g" \
		-e "s#/etc/warren-vpn#$TMP/etc/warren-vpn#g" \
		-e "s#@WARREN_SUFFIX@##g" \
		-e "s#/etc/passwd#$TMP/passwd#g" \
		-e "s#/etc/apparmor.d/#$TMP/apparmor.d/#g" \
		-e "s#/usr/bin/warren-problem-report#$TMP/warren-problem-report#g" \
		-e "s#update-rc.d#true#g" \
		"$SCRIPT_DIR/../$1" > "$TMP/staged"
}

postrm_case() { # postrm_case <script> <interpreter> <argument> <keeps|erases> <unloads apparmor: yes|no>
	local script="$1" shell="$2" arg="$3" config="$4" apparmor="$5" result=ok
	checks=$((checks + 1))
	rm -rf "${TMP:?}/calls" "${TMP:?}/etc" "${TMP:?}/var"
	mkdir -p "$TMP/etc/warren-vpn" "$TMP/var/log/warren-vpn" "$TMP/var/cache/warren-vpn"
	: > "$TMP/etc/warren-vpn/wallet"
	: > "$TMP/passwd"
	stage_postrm "$script"
	PATH="$TMP/bin" "$(command -v "$shell")" "$TMP/staged" "$arg" > "$TMP/out" 2>&1 || result="fails on"
	case "$config" in
	keeps) [ -f "$TMP/etc/warren-vpn/wallet" ] || result="erases the settings directory on" ;;
	erases) [ ! -e "$TMP/etc/warren-vpn" ] || result="keeps the settings directory on" ;;
	esac
	if [ "$config" = keeps ] && [ "$arg" = 0 ] && ! grep -q "/etc/warren-vpn" "$TMP/out"; then
		result="does not say where the kept settings are on"
	fi
	local unloaded=no
	grep -q "^apparmor_parser -R" "$TMP/calls" 2> /dev/null && unloaded=yes
	[ "$unloaded" = "$apparmor" ] || result="unloads the AppArmor profile=$unloaded on"
	if [ "$result" = ok ]; then
		printf '  ok   %s %s: settings %s, AppArmor unloaded=%s\n' "$script" "$arg" "$config" "$apparmor"
	else
		failures=$((failures + 1))
		printf '  FAIL %s %s argument %s\n' "$script" "$result" "$arg"
	fi
}

postrm_case dist-assets/linux/after-remove.sh bash purge erases yes
postrm_case dist-assets/linux/after-remove.sh bash remove keeps yes
postrm_case dist-assets/linux/after-remove.sh bash 0 keeps yes
postrm_case dist-assets/linux/after-remove.sh bash 1 keeps no
postrm_case dist-assets/linux/after-remove.sh bash upgrade keeps no
# pacman runs it on a removal only, with the removed version as argument.
postrm_case dist-assets/linux/after-remove.sh bash 1.1.57-1 keeps yes
postrm_case dist-assets/linux/sysvinit/postrm sh purge erases yes
postrm_case dist-assets/linux/sysvinit/postrm sh remove keeps yes
postrm_case dist-assets/linux/sysvinit/postrm sh upgrade keeps no
postrm_case dist-assets/linux/daemon/postrm bash purge erases no
postrm_case dist-assets/linux/daemon/postrm bash 0 keeps no

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
