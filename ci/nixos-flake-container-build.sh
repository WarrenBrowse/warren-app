#!/bin/sh
# Runs INSIDE the nixos/nix container started by ci/build-nixos-flake.sh.
set -eu

# filter-syscalls: the seccomp filter Nix installs in every build cannot be
# loaded under the x86_64 emulation this container runs on ("unable to load
# seccomp BPF program: Invalid argument"). It hardens builds against a
# non-native syscall slipping in, which is not what protects anything here: the
# whole run is an ephemeral container on a CI host.
export NIX_CONFIG="experimental-features = nix-command flakes
filter-syscalls = false"

# Locking a `github:` input goes through the GitHub API, which gives an
# anonymous caller 60 requests an hour per address, shared with every other
# job a hosted runner's address serves: beta-v1.1.60 lost its release to that
# 403. The job token lifts the limit; it reaches this script by name only.
if [ -n "${WARREN_GITHUB_TOKEN:-}" ]; then
    NIX_CONFIG="$NIX_CONFIG
access-tokens = github.com=$WARREN_GITHUB_TOKEN"
fi

WORK=/tmp/warren-nixos
FLAKE="$WORK/flake"
OUT="$WORK/out"
DIR_NAME="${WARREN_FLAKE_DIR_NAME:?the caller must pass the published directory name}"

fail() {
    echo "::error::$*" >&2
    exit 1
}

step() { echo; echo "=== $* ==="; }

# This image's /etc/group and /etc/passwd are symlinks into
# /nix/store/...-base-system/etc/, so a /nix cache seeded by a DIFFERENT image
# leaves them dangling. Nix then loses its `nixbld` group and fails minutes
# later on "the group 'nixbld' ... does not exist", which accuses nix rather
# than the cache. Name the real condition here, at once.
[ -r /etc/group ] || fail "the container's /etc is unresolvable: the mounted /nix cache was seeded by a different image. Remove the nix store volume and re-run."

deb="$(find "$WORK" -maxdepth 1 -name '*.deb' | head -1)"
[ -n "$deb" ] || fail "no .deb was copied into the container"

package="$(nix eval --raw --file "$FLAKE/release.nix" packageName)"
suffix="$(nix eval --raw --file "$FLAKE/release.nix" suffix)"
version="$(nix eval --raw --file "$FLAKE/release.nix" version)"
echo "package=$package suffix='${suffix:-<none>}' version=$version"

step "lock nixpkgs"
# Locking here rather than committing a lock keeps the pin honest: every release
# ships the nixpkgs its own build resolved and was tested against.
(cd "$FLAKE" && nix flake lock)

step "seed the store with the artifact this release pins"
# Same fixed-output hash the flake declares, so fetchurl finds it already there.
# A mismatch makes the build reach for a URL that is not published yet and fail,
# which is the check: the pinned hash has to be this artifact's.
seeded="$(nix-store --add-fixed sha256 "$deb")"
echo "seeded $seeded"

step "build"
out="$(cd "$FLAKE" && nix build --no-link --print-out-paths ".#default")"
echo "built $out"

step "the binaries run out of the store"
"$out/bin/warren-daemon${suffix}" --version | grep -q "$version" \
    || fail "warren-daemon${suffix} --version does not report $version"
echo "warren-daemon${suffix}: $("$out/bin/warren-daemon${suffix}" --version)"

step "nothing is left linked against a path that does not exist on NixOS"
# The build itself refuses to finish on an unresolved NEEDED entry (see the
# completeness check in package.nix), and the daemon running above proves its
# own link chain resolves. What is left to confirm is that the binaries nobody
# executed here were relinked at all, rather than still pointing at the
# /lib/x86_64-linux-gnu of a Debian that NixOS does not have.
for binary in \
    "$out/bin/warren-daemon${suffix}" \
    "$out/bin/warren${suffix}" \
    "$out/share/warren/warren-gui${suffix}"; do
    [ -e "$binary" ] || fail "$binary is missing from the store path"
    grep -aq '/nix/store/' "$binary" || fail "$binary was not relinked against the store"
    # `&& fail` would end the script silently under `set -e` on the happy path,
    # where grep exits non-zero.
    if grep -aq '/lib64/ld-linux' "$binary"; then
        fail "$binary still uses the system loader"
    fi
done

step "the desktop entry points into the store"
desktop="$out/share/applications/${package}.desktop"
[ -f "$desktop" ] || fail "$desktop is missing"
grep -q "^Exec=$out/bin/${package}" "$desktop" \
    || fail "the desktop entry still points at /opt: $(grep '^Exec=' "$desktop")"

step "the NixOS module evaluates into a real unit"
# A module that only type-checks is worth little; what matters is that the
# service it defines starts the daemon from this very store path.
exec_start="$(
    nix eval --impure --raw --expr "
    let
      flake = builtins.getFlake \"path:$FLAKE\";
      system = flake.inputs.nixpkgs.lib.nixosSystem {
        system = \"x86_64-linux\";
        modules = [
          flake.nixosModules.default
          {
            services.${package}.enable = true;
            services.${package}.enableEarlyBootBlocking = true;
            boot.loader.grub.enable = false;
            fileSystems.\"/\" = { device = \"/dev/null\"; fsType = \"ext4\"; };
            system.stateVersion = \"26.05\";
          }
        ];
      };
    in system.config.systemd.services.\"warren-daemon${suffix}\".serviceConfig.ExecStart
  "
)"
echo "ExecStart = $exec_start"
case "$exec_start" in
    "$out"/bin/warren-daemon"${suffix}"*) ;;
    *) fail "the module's ExecStart does not point at the built package: $exec_start" ;;
esac

step "the default module protects the boot like the packaged unit"
# dist-assets/linux/warren-daemon.service orders the daemon before
# network-online.target and the packages always enable the early boot blocker.
# A module that started the daemon after the network came online, or left the
# blocker off by default, would give every NixOS host a boot window of traffic
# outside the tunnel that no other distribution has.
ordering="$(
    nix eval --impure --raw --expr "
    let
      flake = builtins.getFlake \"path:$FLAKE\";
      system = flake.inputs.nixpkgs.lib.nixosSystem {
        system = \"x86_64-linux\";
        modules = [
          flake.nixosModules.default
          {
            services.${package}.enable = true;
            boot.loader.grub.enable = false;
            fileSystems.\"/\" = { device = \"/dev/null\"; fsType = \"ext4\"; };
            system.stateVersion = \"26.05\";
          }
        ];
      };
      units = system.config.systemd.services;
      daemon = units.\"warren-daemon${suffix}\";
    in
      builtins.concatStringsSep \" \" (
        map (unit: \"before:\" + unit) daemon.before
        ++ map (unit: \"after:\" + unit) daemon.after
        ++ map (unit: \"wants:\" + unit) daemon.wants
        ++ (if units ? \"warren-early-boot-blocking${suffix}\" then [ \"early-boot\" ] else [ ])
      )
  "
)"
echo "daemon ordering: $ordering"
case " $ordering " in *" before:network-online.target "*) ;;
    *) fail "the module does not start the daemon before network-online.target" ;;
esac
case " $ordering " in *" after:network-online.target "* | *" wants:network-online.target "*)
    fail "the module waits for the network to be online before the daemon starts" ;;
esac
case " $ordering " in *" early-boot "*) ;;
    *) fail "the module leaves the early boot blocker off by default" ;;
esac

step "pack the published tarball"
mkdir -p "$OUT/$DIR_NAME"
cp "$FLAKE/flake.nix" "$FLAKE/flake.lock" "$FLAKE/package.nix" "$FLAKE/module.nix" \
    "$FLAKE/release.nix" "$FLAKE/README.md" "$OUT/$DIR_NAME/"
(cd "$OUT" && tar czf "$DIR_NAME.tar.gz" "$DIR_NAME")

step "the packed tarball is a usable flake"
# The published shape, fetched the way a user's flake input fetches it. The
# tarball fetcher strips the single top-level directory; proving that here beats
# discovering it from a user's bug report.
packed_out="$(nix build --no-link --print-out-paths "tarball+file://$OUT/$DIR_NAME.tar.gz#default")"
[ "$packed_out" = "$out" ] \
    || fail "the packed flake builds a different store path ($packed_out vs $out)"

echo
echo "NixOS flake OK: $DIR_NAME.tar.gz"
