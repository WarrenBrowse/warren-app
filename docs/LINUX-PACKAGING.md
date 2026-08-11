# Linux packaging: eight artifacts, two builds

Moved out of `CLAUDE.md` on 2026-08-11 as repo-specific depth. The rule that
stays binding is only this: the architecture token is read off the file and
mapped, never assumed, and the three NetworkManager traps below were each
paid for by a measurement.

`build.sh` produces `.deb`, `.rpm` and `.pacman` through electron-builder/fpm,
for the architecture of the machine it runs on. `release.yml` runs it twice, on
two pools of the same Mac, each native: `build-linux` on the x86_64 (Rosetta)
one and `build-linux-arm64` on the aarch64 one. Nothing is cross-compiled, so
neither job needs a second sysroot and the arm64 packages are built by the same
path that `warren-tests.yml` already exercises on every push. `release-daemon.yml`
mirrors the split for the headless daemon and CLI.

The architecture spelling differs per format, and per tool. electron-builder
emits `_amd64.deb` / `_x86_64.rpm` / `_x64.pacman` and `_arm64.deb` /
`_aarch64.rpm` / `_aarch64.pacman`; `ci/stage-release-assets.sh` reads that token
off the file and maps it to the release name (`-linux-amd64.deb`,
`-linux-aarch64.rpm`, ...), refusing a token it does not know rather than
shipping an installer whose name lies about its architecture.

Two more artifacts are derived from the **amd64** `.deb` afterwards, by the
`build-linux-sysvinit` and `build-nixos` jobs in `release.yml`, on the
docker-capable runner. Neither recompiles anything, so both cost minutes, not
another Rosetta build hour, and neither has an arm64 counterpart: both target the
x86_64 desktop and neither script is arch-generic.

- **`-linux-amd64-sysvinit.deb`** (MX Linux, antiX, Devuan). The systemd
  package's postinst runs `systemctl enable` under `set -e`, so on a host with
  no systemd dpkg leaves it half-configured. `ci/build-sysvinit-deb.sh` repacks
  it with the LSB init scripts in `dist-assets/linux/sysvinit/`, renames it
  `<pkg>-sysvinit` and declares Provides/Conflicts/Replaces on `<pkg>` so the
  two flavours are exclusive. `ci/test-sysvinit-deb.sh` then installs, starts,
  crashes, stops and purges it for real inside a systemd-less Debian container.
- **`-linux-x86_64-nixos.tar.gz`** (NixOS). A tarball flake pinning that
  release's `.deb` by URL and hash, plus a NixOS module. Sources in
  `nix/warren-vpn/`, staged by `ci/stage-nixos-flake.sh` and built for real by
  `ci/build-nixos-flake.sh` (a `nix build` in a container, the binaries run out
  of the store, the module evaluated into a unit).

The same build also ships `warren-nm-vpn-service`, a NetworkManager VPN
service plugin, because GNOME and KDE light their VPN indicator for exactly one
thing: an active NetworkManager connection whose type is `vpn`. Only a
registered VPN plugin can produce one, so a tunnel the engine built itself is
invisible to the desktop without it. The plugin describes a tunnel that already
exists (it never builds one), the daemon publishes and withdraws the connection
from `mullvad-daemon/src/nm_vpn_indicator.rs`, and both sides are best effort:
a machine with no NetworkManager simply gets nothing. Three traps, each paid for
by a measurement on NM 1.46:

- **The postinst must reload dbus.** The system bus reads
  `/usr/share/dbus-1/system.d` once and denies ownership of a name it has no
  policy for, so without the reload the plugin cannot take its name until the
  next boot. NetworkManager itself does pick up a new `.name` file live.
- **The plugin watches its interface.** NetworkManager keeps a VPN connection
  `activated` after the interface is gone, so a daemon that dies would leave the
  desktop claiming a VPN. The plugin retracts it by index, not by name.
- **The published config must match the interface exactly, and claim nothing
  else.** NetworkManager reconciles what it is handed onto the live interface;
  `never-default` plus `preserve-routes` keep it off the engine's routing, and
  `auto-route-ext-gw` (NM 1.42+ only, or the connection is rejected) keeps it
  from adding a host route to the peer.

Three things to keep in mind when touching any of it:

- **sysvinit has no supervisor.** The daemon exits fail-closed, so
  `warren-daemon-supervise` respawns it forever, mirroring the unit's
  `Restart=always` / `StartLimitIntervalSec=0`. Dropping that would let a crash
  strand the machine offline with no daemon to unblock it.
- **`autoPatchelfHook` is deliberately unused** in `nix/warren-vpn/package.nix`.
  Its worker is a Python program, and the release runner builds under x86_64
  emulation where the interpreter loses `argv[0]` across `exec` and cannot find
  its own modules. The interpreter and RPATH are set with `patchelf` directly,
  and an explicit check fails the build on a NEEDED entry nothing provides.
- **The flake pins the update host, never a GitHub asset.** The repository is
  private, so a release asset answers 404 without a token and `nix build` would
  fetch nothing.
