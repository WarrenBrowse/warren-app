#!/usr/bin/env bash
# Repack a built Warren VPN .deb into its sysvinit flavour, for distributions
# that do not run systemd: MX Linux (the sysVinit editions ship no systemd at
# all since MX 25 dropped systemd-shim), antiX, Devuan.
#
# The systemd .deb is broken on those hosts by construction, and loudly: its
# postinst runs `systemctl enable` under `set -e`, so dpkg leaves the package
# half-configured on a machine that has no systemctl. This script produces a
# package with the same payload and a different init integration:
#
#   - /etc/init.d/warren-daemon[-env]              (LSB, multi-user runlevels)
#   - /etc/init.d/warren-early-boot-blocking[-env] (LSB, rcS, before networking)
#   - /usr/lib/warren-vpn[-env]/warren-daemon-supervise  (Restart=always)
#   - maintainer scripts driving update-rc.d / invoke-rc.d
#   - no systemd units at all
#
# The package is renamed to <pkg>-sysvinit and declares
# Provides/Conflicts/Replaces on <pkg>, so the two flavours are exclusive and
# switching between them is an ordinary apt operation instead of a file fight.
#
# No recompilation: this reads the artifact the Linux build already produced,
# which is what keeps the two flavours bit-identical where it matters.
#
# Usage: ci/build-sysvinit-deb.sh <input.deb> <output.deb>
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ASSETS="$REPO_ROOT/dist-assets/linux/sysvinit"

input="${1:?usage: build-sysvinit-deb.sh <input.deb> <output.deb>}"
output="${2:?missing output path}"

command -v dpkg-deb > /dev/null || {
    echo "::error::dpkg-deb is required to repack the sysvinit .deb" >&2
    exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/root"

echo "Unpacking $input"
dpkg-deb -R "$input" "$root"

package="$(awk '/^Package:/ { print $2; exit }' "$root/DEBIAN/control")"
[ -n "$package" ] || {
    echo "::error::no Package field in the control file of $input" >&2
    exit 1
}

# The compiled product environment is what suffixes every installed name (unit
# names, /usr/bin binaries, /var dirs). Read it back off the package rather than
# taking it as an argument, so the repack can never disagree with the artifact
# it is repacking.
case "$package" in
    warren-vpn) suffix="" ;;
    warren-vpn-*) suffix="-${package#warren-vpn-}" ;;
    *)
        echo "::error::unexpected package name '$package' (expected warren-vpn[-env])" >&2
        exit 1
        ;;
esac

# "Warren VPN", "Warren VPN Beta": the single directory under /opt is the
# product name, and it also names the per-user config dir the purge sweeps.
product_name="$(cd "$root/opt" && ls -1 | head -1)"
[ -n "$product_name" ] && [ -d "$root/opt/$product_name" ] || {
    echo "::error::no product directory under /opt in $input" >&2
    exit 1
}
product_dir="/opt/$product_name"

echo "package=$package suffix='${suffix:-<none>}' product='$product_name'"

# A missing unit means the Linux packaging moved and this repack is patching a
# layout that no longer exists: refuse rather than ship a package whose daemon
# nothing starts.
for unit in "warren-daemon${suffix}.service" "warren-early-boot-blocking${suffix}.service"; do
    [ -f "$root/usr/lib/systemd/system/$unit" ] || {
        echo "::error::$input does not ship usr/lib/systemd/system/$unit; the Linux packaging changed" >&2
        exit 1
    }
done
[ -x "$root/usr/bin/warren-daemon${suffix}" ] || {
    echo "::error::$input does not ship usr/bin/warren-daemon${suffix}" >&2
    exit 1
}

# @PLACEHOLDER@ -> installed name, for every asset this script installs.
substitute() { # substitute <src> <dst> <mode>
    sed -e "s|@WARREN_SUFFIX@|${suffix}|g" \
        -e "s|@WARREN_PRODUCT_DIR@|${product_dir}|g" \
        -e "s|@WARREN_PRODUCT_NAME@|${product_name}|g" \
        "$1" > "$2"
    chmod "$3" "$2"
}

echo "Dropping the systemd units"
rm -f "$root/usr/lib/systemd/system/warren-daemon${suffix}.service"
rm -f "$root/usr/lib/systemd/system/warren-early-boot-blocking${suffix}.service"
rmdir -p --ignore-fail-on-non-empty "$root/usr/lib/systemd/system" 2> /dev/null || true

echo "Installing the init scripts"
mkdir -p "$root/etc/init.d" "$root/usr/lib/warren-vpn${suffix}"
substitute "$ASSETS/warren-daemon.init" "$root/etc/init.d/warren-daemon${suffix}" 755
substitute "$ASSETS/warren-early-boot-blocking.init" \
    "$root/etc/init.d/warren-early-boot-blocking${suffix}" 755
substitute "$ASSETS/warren-daemon-supervise" \
    "$root/usr/lib/warren-vpn${suffix}/warren-daemon-supervise" 755

echo "Installing the maintainer scripts"
for script in preinst postinst prerm postrm; do
    substitute "$ASSETS/$script" "$root/DEBIAN/$script" 755
done

echo "Rewriting the control file"
control="$root/DEBIAN/control"
# Provides/Conflicts/Replaces make the two flavours mutually exclusive: they own
# the same paths, so apt has to see them as one slot rather than let both land.
#
# Depends is the one thing this flavour states that the fpm-built input does not
# (electron-builder passes fpm --no-depends): libdbus is the daemon's only
# unresolved shared library, and without the declaration a host that lacks it
# installs cleanly and then crash-loops the daemon in silence. Every MX desktop
# has it, so the dependency costs nothing and turns the one failure mode that
# would be invisible into an apt message.
awk -v pkg="$package" '
    /^Package:/ { print "Package: " pkg "-sysvinit"; next }
    /^(Provides|Conflicts|Replaces|Depends):/ { next }
    /^Description:/ {
        print "Provides: " pkg
        print "Conflicts: " pkg
        print "Replaces: " pkg
        print "Depends: libdbus-1-3"
        print
        next
    }
    { print }
' "$control" > "$control.new"
mv "$control.new" "$control"
grep -q "^Package: ${package}-sysvinit$" "$control" || {
    echo "::error::failed to rename the package in the control file" >&2
    exit 1
}

echo "Regenerating md5sums"
(
    cd "$root"
    find . -path ./DEBIAN -prune -o -type f -print0 \
        | sed -z 's|^\./||' \
        | xargs -0 -r md5sum > DEBIAN/md5sums
)

mkdir -p "$(dirname "$output")"
# Match the input's compression, which fpm writes as xz: repacking an Electron
# payload with gzip costs the user a quarter of the download for nothing.
dpkg-deb --build -Zxz --root-owner-group "$root" "$output" > /dev/null
echo "Built $output ($(du -h "$output" | cut -f1))"
