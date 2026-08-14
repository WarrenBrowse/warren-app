#!/usr/bin/env bash
# Assemble a headless (daemon + CLI, no GUI) bundle for the host platform and
# leave it in release-assets/, ready to be published on the warren-cli
# distribution repo.
#
#   ci/build-headless-bundle.sh linux   <version> <warren-cli-checkout>
#   ci/build-headless-bundle.sh macos   <version> <warren-cli-checkout>
#   ci/build-headless-bundle.sh windows <version> <warren-cli-checkout>
#
# Produces  warren-headless[-<env>]-<version>-<os>-<arch>.<tar.gz|zip>
#
# The Linux bundle is the one distributions with neither dpkg nor rpm install
# (Arch, Void, Gentoo, Slackware, openSUSE Tumbleweed's minimal spins, NixOS
# outside the flake). It carries the SAME payload as the .deb, plus the service
# integration for the three init systems those hosts actually run, because the
# .deb's systemd units are useless on a machine with OpenRC.
#
# glibc only, on every platform: the daemon links the vendored libnftnl/libmnl
# against the runner's glibc, so a musl host (Alpine, and any distribution
# built on it) cannot run these binaries. That is stated in the installer and
# in the docs rather than left for the user to discover as a loader error.
#
# The install scripts and the non-systemd service units live in warren-cli
# (the distribution layer owns them); everything else comes from this repo, so
# neither side can ship half a bundle.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

platform="${1:?usage: build-headless-bundle.sh <linux|macos|windows> <version> <warren-cli-dir>}"
version="${2:?missing version (e.g. 1.1.14)}"
cli_dir="${3:?missing warren-cli checkout directory}"

[ -d "$cli_dir" ] || {
    echo "::error::warren-cli checkout not found at $cli_dir" >&2
    exit 1
}

product_env="${WARREN_PRODUCT_ENV:-prod}"
case "$product_env" in
    prod) env_tag="" ;;
    staging | beta) env_tag="-${product_env}" ;;
    *)
        echo "::error::WARREN_PRODUCT_ENV must be prod|staging|beta, got: $product_env" >&2
        exit 1
        ;;
esac

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
OUT="release-assets"
mkdir -p "$OUT"

# The exact bytes build.sh packages into the .deb, so a tarball install and a
# package install boot from the same resources.
bash ci/stage-daemon-resources.sh

# A bundle that silently lacks a binary installs fine and then fails at
# runtime, which is the most expensive place to find out.
require() { # require <path> <what it is>
    [ -e "$1" ] || {
        echo "::error::missing $2: $1" >&2
        exit 1
    }
}

# Everything a support request needs in order to know what is installed,
# without the daemon having to be running to answer.
write_bundle_info() { # write_bundle_info <staging-dir>
    cat > "$1/BUNDLE-INFO" <<INFO
product=Warren VPN headless (daemon + CLI)
version=$version
product_env=$product_env
platform=$platform
built_from=$(git rev-parse --short HEAD 2> /dev/null || echo unknown)
INFO
}

case "$platform" in
    linux)
        arch="$(uname -m)"
        case "$arch" in
            x86_64 | aarch64) ;;
            *)
                echo "::error::unsupported Linux architecture for a headless bundle: $arch" >&2
                exit 1
                ;;
        esac
        name="warren-headless${env_tag}-${version}-linux-${arch}"
        stage="$OUT/$name"
        rm -rf "$stage"
        mkdir -p "$stage/bin" "$stage/resources" "$stage/completions" \
            "$stage/service/systemd" "$stage/service/sysvinit" "$stage/service/openrc"

        rel="$TARGET_DIR/release"
        # Exactly the payload of the daemon-only .deb. warren-nm-vpn-service is
        # deliberately absent from both: it exists to put a VPN indicator in a
        # desktop's network menu, and this bundle installs on machines that
        # have neither.
        for binary in warren warren-daemon warren-exclude; do
            require "$rel/$binary" "release binary"
            install -m 0755 "$rel/$binary" "$stage/bin/$binary"
        done
        # Installed under the resource dir rather than on PATH, exactly as the
        # .deb does: they are helpers the daemon and the problem reporter call,
        # not commands.
        for binary in warren-setup warren-problem-report; do
            require "$rel/$binary" "release binary"
            install -m 0755 "$rel/$binary" "$stage/resources/$binary"
        done

        install -m 0644 dist-assets/ca.crt "$stage/resources/ca.crt"
        install -m 0644 build/relays.json "$stage/resources/relays.json"
        install -m 0644 build/warren-relays.json "$stage/resources/warren-relays.json"
        install -m 0644 CHANGELOG.md "$stage/resources/CHANGELOG.md"

        for completion in warren.bash _warren warren.fish; do
            require "build/shell-completions/$completion" "shell completion"
            install -m 0644 "build/shell-completions/$completion" "$stage/completions/$completion"
        done

        install -m 0644 dist-assets/linux/warren-daemon.service \
            "$stage/service/systemd/warren-daemon.service"
        install -m 0644 dist-assets/linux/warren-early-boot-blocking.service \
            "$stage/service/systemd/warren-early-boot-blocking.service"

        # The sysvinit integration already exists for the sysvinit .deb, so the
        # tarball resolves the same templates rather than growing a second
        # implementation that can drift from it. The tarball installs under the
        # unsuffixed prod names (one Warren daemon per machine), so the
        # environment suffix is empty whatever this build's environment is.
        substitute() { # substitute <src> <dst> <mode>
            sed -e 's|@WARREN_SUFFIX@||g' \
                -e 's|@WARREN_PRODUCT_DIR@|/opt/Warren VPN|g' \
                -e 's|@WARREN_PRODUCT_NAME@|Warren VPN|g' \
                "$1" > "$2"
            chmod "$3" "$2"
        }
        substitute dist-assets/linux/sysvinit/warren-daemon.init \
            "$stage/service/sysvinit/warren-daemon" 755
        substitute dist-assets/linux/sysvinit/warren-early-boot-blocking.init \
            "$stage/service/sysvinit/warren-early-boot-blocking" 755
        substitute dist-assets/linux/sysvinit/warren-daemon-supervise \
            "$stage/service/sysvinit/warren-daemon-supervise" 755

        require "$cli_dir/linux/warren-daemon.openrc" "OpenRC service script"
        install -m 0755 "$cli_dir/linux/warren-daemon.openrc" \
            "$stage/service/openrc/warren-daemon"

        require "$cli_dir/linux/install-linux.sh" "tarball installer"
        install -m 0755 "$cli_dir/linux/install-linux.sh" "$stage/install.sh"

        write_bundle_info "$stage"
        tar -C "$OUT" -czf "$OUT/${name}.tar.gz" "$name"
        rm -rf "$stage"
        ;;

    macos)
        # A single universal bundle, never one per architecture: the arm64-only
        # bundle this job used to produce could not be installed on any Intel
        # Mac, and nothing said so until the binary refused to exec.
        name="warren-headless${env_tag}-${version}-macos-universal"
        stage="$OUT/$name"
        rm -rf "$stage"
        mkdir -p "$stage/bin" "$stage/resources" "$stage/completions"

        arm="$TARGET_DIR/aarch64-apple-darwin/release"
        intel="$TARGET_DIR/x86_64-apple-darwin/release"
        for binary in warren warren-daemon warren-setup warren-problem-report; do
            require "$arm/$binary" "arm64 release binary"
            require "$intel/$binary" "x86_64 release binary"
            lipo -create -output "$stage/bin/$binary" "$arm/$binary" "$intel/$binary"
            chmod 0755 "$stage/bin/$binary"
            # lipo happily writes a single-slice file when handed the same
            # input twice, and a bundle labelled universal that is not one is
            # exactly the failure this job exists to stop.
            lipo -archs "$stage/bin/$binary" | grep -q x86_64 || {
                echo "::error::$binary carries no x86_64 slice" >&2
                exit 1
            }
            lipo -archs "$stage/bin/$binary" | grep -q arm64 || {
                echo "::error::$binary carries no arm64 slice" >&2
                exit 1
            }
        done

        install -m 0644 dist-assets/ca.crt "$stage/resources/ca.crt"
        install -m 0644 build/relays.json "$stage/resources/relays.json"
        install -m 0644 build/warren-relays.json "$stage/resources/warren-relays.json"
        install -m 0644 CHANGELOG.md "$stage/resources/CHANGELOG.md"

        # Generated by build.sh on a packaged build; the headless macOS job
        # runs a plain cargo build, so generate them here when absent.
        if [ ! -d build/shell-completions ]; then
            mkdir -p build/shell-completions
            for sh in bash zsh fish; do
                "$stage/bin/warren" shell-completions "$sh" build/shell-completions/
            done
        fi
        for completion in warren.bash _warren warren.fish; do
            install -m 0644 "build/shell-completions/$completion" "$stage/completions/$completion"
        done

        require "$cli_dir/macos/com.warren.daemon.plist" "launchd service definition"
        install -m 0644 "$cli_dir/macos/com.warren.daemon.plist" "$stage/com.warren.daemon.plist"
        require "$cli_dir/macos/install-macos.sh" "macOS installer"
        install -m 0755 "$cli_dir/macos/install-macos.sh" "$stage/install.sh"

        write_bundle_info "$stage"
        tar -C "$OUT" -czf "$OUT/${name}.tar.gz" "$name"
        rm -rf "$stage"
        ;;

    windows)
        # x64 only, as the GUI ships: Windows on ARM runs x64 user-mode code
        # under emulation, and the WFP and wintun pieces the daemon drives are
        # user-mode too.
        name="warren-headless${env_tag}-${version}-windows-x64"
        stage="$OUT/$name"
        rm -rf "$stage"
        # ONE directory, the way the GUI installs: winfw.dll is a link-time
        # import resolved next to the exe, while wintun.dll and the
        # split-tunnel driver are opened from the resource dir. Splitting them
        # across bin/ and resources/ means one of the two is always wrong.
        mkdir -p "$stage"

        rel="$TARGET_DIR/x86_64-pc-windows-msvc/release"
        for binary in warren.exe warren-daemon.exe warren-setup.exe warren-problem-report.exe; do
            require "$rel/$binary" "release binary"
            install -m 0755 "$rel/$binary" "$stage/$binary"
        done

        # Without winfw.dll the daemon cannot arm or clear the kill switch, and
        # without wintun.dll it cannot create the tunnel adapter. The bundle
        # shipped neither until now, so it installed and then did nothing.
        require windows/winfw/bin/x64-Release/winfw.dll "winfw.dll (build it with build-windows-modules.sh)"
        install -m 0755 windows/winfw/bin/x64-Release/winfw.dll "$stage/winfw.dll"
        install -m 0755 dist-assets/binaries/x86_64-pc-windows-msvc/wintun/wintun.dll \
            "$stage/wintun.dll"
        # Split tunnelling is optional at runtime (the daemon installs the
        # driver service on demand), so ship it and let the user enable it.
        install -m 0644 dist-assets/binaries/x86_64-pc-windows-msvc/split-tunnel/mullvad-split-tunnel.sys \
            "$stage/mullvad-split-tunnel.sys"

        install -m 0644 dist-assets/ca.crt "$stage/ca.crt"
        install -m 0644 build/relays.json "$stage/relays.json"
        install -m 0644 build/warren-relays.json "$stage/warren-relays.json"
        install -m 0644 CHANGELOG.md "$stage/CHANGELOG.md"

        require "$cli_dir/windows/install-windows.ps1" "Windows installer"
        install -m 0644 "$cli_dir/windows/install-windows.ps1" "$stage/install-windows.ps1"

        write_bundle_info "$stage"
        (cd "$OUT" && powershell -NoProfile -Command \
            "Compress-Archive -Path '$name' -DestinationPath '${name}.zip' -Force")
        rm -rf "$stage"
        ;;

    *)
        echo "::error::unknown platform: $platform (linux|macos|windows)" >&2
        exit 1
        ;;
esac

ls -lh "$OUT"
