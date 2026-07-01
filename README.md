# Warren VPN desktop app

Welcome to the Warren VPN client app source code repository.

Warren VPN is a **fork of [Mullvad VPN](https://github.com/mullvad/mullvadvpn-app)** that replaces
the WireGuard tunnel backend with a **QUIC** tunnel (Ed25519 TLS handshake, identity derived from a
local BIP39 mnemonic) and can run **without the Mullvad backend** (`api.mullvad.net`) thanks to a
local account mode.

The fork keeps the upstream daemon / frontend architecture: service
([`mullvad-daemon`](mullvad-daemon/), renamed to the `warren-daemon` binary), Electron GUI
([`desktop/`](desktop/)) and CLI ([`mullvad-cli`](mullvad-cli/), renamed to the `warren` binary),
and all the network security guarantees (firewall lockdown, killswitch, split-tunneling) are
preserved. See [`docs/warren-architecture.md`](docs/warren-architecture.md) for the usage guide and
[`UPSTREAM_BASELINE.md`](UPSTREAM_BASELINE.md) for upstream baseline tracking.

## TL;DR: run the app in dev

Prerequisites: clone the three sibling repos **`warrenguard`, `warren-sdk-rs`, `warren-contract`
next to `warren-app`** (same parent directory), plus
`cargo` + `protoc` (daemon) and `node` + `npm` (GUI). The Quinn fork is the published
`WarrenBrowse/warren-quinn` git-dep pinned by tag in the root `[patch.crates-io]`, so cargo
fetches it automatically; there is no local fork to rebuild and no manual step.

```bash
# Terminal 1: Rust daemon in release (real tunnel performance)
./scripts/dev/warren-dev.sh daemon --release

# Terminal 2: Electron GUI (hot-reload)
./scripts/dev/warren-dev.sh app
```

All-in-one: `./scripts/dev/warren-dev.sh both --release` (Ctrl+C stops both). Details and
options: [Running the app from source (dev)](#running-the-app-from-source-dev).

## Fork status

Private POC phase. The fork is being iterated on; **no public release is available yet**. The POC
phase target:

- QUIC tunnel backend operational on Linux and macOS desktop
- Warren BIP39 identity + Ed25519 signature on the migrated REST endpoints
- Account/subscription served by warren-api (`https://api.warrenbrowse.com`)
- Rebranded Electron GUI

Android and iOS are **not migrated** at this stage; the upstream sources are kept as-is to reduce
weekly merge conflicts.

### Supported platforms (fork)

| Platform | Warren fork status |
|---|---|
| Linux (x86_64) | ✅ POC target |
| macOS (arm64 / x86_64) | ✅ POC target |
| Windows | ⏸ inherited from upstream, not fork-tested |
| Android | ⏸ upstream only |
| iOS | ⏸ upstream only |

For the upstream matrix (OS, versions, architectures supported by the Mullvad code), see
[Supported Platforms](docs/supported-platforms.md).

## Features

| | Linux (Warren) | macOS (Warren) | Notes |
|---|:-:|:-:|---|
| **Warren tunnel (QUIC + Ed25519)** | ✓ | ✓ | The fork's only tunnel mode |
| **Warren account/subscription (BIP39 + warren-api)** | ✓ | ✓ | BIP39 identity, subscription served by warren-api |
| Quantum-resistant tunnels (PQ-WG) | ✓ | ✓ | Inherited upstream WireGuard code |
| Split tunneling | ✓ | ✓ | |
| Custom DNS server | ✓ | ✓ | |
| Content blockers (Ads etc) | ✓ | ✓ | |
| Killswitch / lockdown mode | ✓ | ✓ | |
| Local network access (optional) | ✓ | ✓ | |

The upstream obfuscation modes (WireGuard over TCP / Shadowsocks / QUIC / LWO) and DAITA are **not
enabled** on the Warren path: the Warren tunnel runs QUIC on 443 natively.

## User security, privacy, anonymity

The fork inherits the Mullvad client guarantees: it is a privacy-respecting VPN client that does its
best to prevent traffic leaks, with security-oriented defaults. The
[dedicated security document](docs/security.md) describes in detail what the app blocks, what it
allows, and how.

**Warren-specific**: the identity is carried by an Ed25519 key derived from a BIP39 mnemonic
(12 words) stored in the OS secret vault (macOS Keychain / Windows DPAPI / file
`<settings_dir>/secrets/warren_mnemonic.txt` 0600 on Linux). No account number, no bearer token.
That same key authenticates the QUIC TLS handshake to the exit *and* signs Warren API requests
(`X-Warren-{PubKey,Signature,Timestamp,Nonce}` headers). See
[`docs/warren-architecture.md`](docs/warren-architecture.md) section "Crypto handshake".

## Secure development

The fork keeps the upstream signing and review practices:

### Git signatures

Every merge commit on the `main` branch must be PGP-signed. Individual commits in a feature branch
do not need to be signed, unless they modify a *locked-down* file listed in
[`verify-locked-down-signatures`](.github/workflows/verify-locked-down-signatures.yml).

### External audits (upstream)

The upstream Mullvad app is audited every two years by external experts. The results are published
raw in [`audits/`](./audits/README.md). The Warren fork **does not have a dedicated audit yet**; the
changes introduced by the fork are not covered by the existing Mullvad audits. To report a security
issue, see [SECURITY.md](SECURITY.md).

## Getting the code

This repo uses submodules. To clone:

```bash
git clone git@github.com:WarrenBrowse/warren-app.git
cd warren-app
git submodule update --init
```

On Linux and macOS, if you also want the WireGuard fallback path:

```bash
git submodule update --init wireguard-go-rs/libwg/wireguard-go
```

Details in the [`wireguard-go-rs` crate](./wireguard-go-rs/README.md).

### `dist-assets/binaries` submodule

The submodule at `dist-assets/binaries` contains third-party binaries bundled with the app (Wintun,
etc.). It still points at the upstream Mullvad repo; the fork does not (yet) have its own binaries
mirror.

### Warren crates consumed via `path`

The fork pulls Warren crates by path from three sibling repos (no `warren-core`):

- Data-plane engine crates from [`warrenguard/`](../warrenguard/):
  `warrenguard-transport`, `-route-split`, `-config`, `-wire`, `-multihop`,
  `-relay`, `-natpmp-client`, `-natpmp-protocol`, `-backoff`. Pinned in
  [`.warrenguard-version`](.warrenguard-version).
- Client SDK crates from [`warren-sdk-rs/`](../warren-sdk-rs/): `warren-api`,
  `warren-identity`. Pinned in [`.warren-sdk-version`](.warren-sdk-version).
- Neutral contract crates from [`warren-contract/`](../warren-contract/):
  `warren-contract`, `warren-discovery-core`. Pinned in
  [`.warren-contract-version`](.warren-contract-version).

The workspace crate [`talpid-warren-tunnel`](talpid-warren-tunnel/) bridges the talpid state machine
and these crates. The quinn fork is the published `WarrenBrowse/warren-quinn` git-dep (pinned by tag),
consumed via `[patch.crates-io]` in [`Cargo.toml`](Cargo.toml); warrenguard uses the same fork.

## Building the app

See the [build instructions](BuildInstructions.md) (three-sibling layout, Linux native deps,
cross-compile via `Cross.toml`). Fork-specific notes in
[`docs/warren-architecture.md`](docs/warren-architecture.md).

## Running the app from source (dev)

To iterate locally without packaging, the repo provides a dev launcher:
[`scripts/dev/warren-dev.sh`](scripts/dev/warren-dev.sh). It builds and runs the Rust daemon
(`warren-daemon`, with sudo) and the Electron GUI (Vite hot-reload), handling the lifecycle cleanly
(Ctrl+C, socket cleanup, macOS DNS restore if the daemon is killed before restoring it).

Prerequisites: **`warrenguard`, `warren-sdk-rs`, `warren-contract` cloned next to `warren-app`**
(the daemon consumes their path crates; the Quinn fork is the published
`WarrenBrowse/warren-quinn` git-dep fetched automatically by cargo),
`cargo` + `protoc` for the daemon, `node` + `npm` for the GUI (Linux/macOS).

### Two-terminal workflow

```bash
# Terminal 1: daemon in release (real tunnel performance)
./scripts/dev/warren-dev.sh daemon --release

# Terminal 2: Electron GUI (hot-reload)
./scripts/dev/warren-dev.sh app
```

### Single-terminal workflow

`both` runs both with a unified lifecycle (Ctrl+C stops the daemon **and** the app; daemon logs are
prefixed with `[daemon]`):

```bash
./scripts/dev/warren-dev.sh both --release
```

### Commands and options

```
daemon   Build & run the Rust daemon in foreground (sudo)
app      Run the Electron GUI only (Vite hot-reload)
both     Daemon + app, unified lifecycle (Ctrl+C stops both)
stop     Stop a daemon started in the background
status   Show the running components

Daemon options:
  --release        Build the daemon in release mode
  -v / -vv / -vvv  Log verbosity (default: -v / INFO)
  --no-log-file    Log to stdout only
  -- <args>        Pass extra args to warren-daemon
```

> **Note on "release mode"**: `--release` only applies to the **daemon**. The `app` command always
> runs the GUI via `npm run develop` (dev hot-reload); there is no release variant of the GUI in this
> script. The workflow above is therefore "daemon release + GUI dev", useful to measure real tunnel
> performance without recompiling the Rust in debug. For a real packaged GUI build, see
> `npm run pack:<OS>` below.

Useful dev behavior details:

- The dev daemon runs with `WARREN_USE_PLAINTEXT_STORAGE=1`: it persists the mnemonic in a
  `0600 root:root` file under `<settings_dir>/secrets/` instead of the macOS Keychain / Windows
  DPAPI. On an unsigned dev build, the binary hash changes on every `cargo build`, which would
  trigger a macOS authorization prompt on every launch; this variable keeps the dev loop
  friction-free. A signed release build (stable Developer ID) should leave it unset.
- Management socket: `/var/run/warren-vpn`. The GUI warns if the daemon is not up yet.
- Background daemon: logs to `/tmp/warren-daemon-dev.log`, PID in `/tmp/warren-daemon-dev.pid`.

## Releasing the app

The upstream release procedure is documented in [Release.md](Release.md). **No public Warren release
yet**; the repo stays private during the POC phase. See
[`UPSTREAM_BASELINE.md`](UPSTREAM_BASELINE.md) section "Decisions made" for the weekly upstream merge
cadence.

## Environment variables used by the daemon

### Warren-specific

* `WARREN_API_URL`: URL of the warren-api backend (account/subscription/device). Empty = compiled
  default (`https://api.warrenbrowse.com`).

* `WARREN_SETTINGS_DIR`, `WARREN_LOG_DIR`, `WARREN_CACHE_DIR`, `WARREN_RPC_SOCKET_PATH`: Override the
  daemon paths. If unset, the upstream `MULLVAD_*` variants are consulted as a fallback (compat
  aliases).

### Inherited from upstream (all still valid)

* `TALPID_FIREWALL_DEBUG`: Firewall debug helper (Linux: packet counters; macOS: log of matched
  packets on `pflog0`, values `all` / `pass` / `drop`).

* `TALPID_FIREWALL_DONT_SET_SRC_VALID_MARK`, Linux: prevents the daemon from setting
  `net.ipv4.conf.all.src_valid_mark=1` when a tunnel is established. Use only if you understand the
  consequences on strict `rp_filter`.

* `TALPID_FIREWALL_DONT_SET_ARP_IGNORE`, Linux: prevents the daemon from setting
  `net.ipv4.conf.all.arp_ignore=2`. The default protects the in-tunnel IP from ARP probes.

* `TALPID_DNS_MODULE`: Forces the DNS config method. Linux: `static-file` / `resolvconf` /
  `systemd` / `network-manager`. Windows: `iphlpapi` / `netsh` / `tcpip`.

* `TALPID_DISABLE_LOCAL_DNS_RESOLVER`: macOS only. Set to `1` to disable the local DNS resolver.

* `TALPID_NEVER_FILTER_AAAA_QUERIES`: macOS only. Set to `1` to never drop AAAA DNS queries.

* `TALPID_FORCE_USERSPACE_WIREGUARD`: Forces the daemon to use the userspace WireGuard
  implementation (fallback path).

* `TALPID_DISABLE_OFFLINE_MONITOR`: Forces the daemon to always consider the host online.

* `TALPID_CGROUP2_FS`, Linux: overrides the cgroup2 path (default `/sys/fs/cgroup`) used for split
  tunneling.

* `TALPID_NET_CLS_MOUNT_DIR`, Linux: forces the mount point of the `net_cls` controller (cgroup v1
  legacy split tunneling).

* `WARREN_MANAGEMENT_SOCKET_GROUP` (inherited alias: `MULLVAD_MANAGEMENT_SOCKET_GROUP`), Linux/macOS:
  restricts access to the management UDS socket to a given Unix group (= only root and that group can
  drive the CLI/GUI and read the wallet mnemonic phrase). If the variable is set but the group does
  not exist, the daemon refuses to start the socket (fail-closed). If it is not set, the daemon uses
  the `warren` group (created by the installer). If that group is absent, the socket falls back to
  global access (`0o766`) with a warning: in that mode, the wallet/secrets RPCs are restricted to the
  first local uid that connects (trust-on-first-use). For multi-user safety, create the `warren`
  group and add your desktop user to it.

* `MULLVAD_BACKTRACE_ON_FAULT`: On SIGSEGV etc., logs a backtrace to `daemon.log`. Enabled by default
  in debug builds, disabled in release builds. Allocating from the signal handler is technically UB;
  enable at your own risk.

### Development builds only

* `MULLVAD_API_HOST`: Hostname to use for upstream API requests (remote account path).

* `MULLVAD_API_ADDR`: IP:port to use for upstream API requests.

* `MULLVAD_API_DISABLE_TLS`: Forces cleartext HTTP for API requests.

* `MULLVAD_CONNCHECK_HOST`: Hostname used for connection check requests.

* `MULLVAD_ENABLE_DEV_UPDATES`: Enables version checks in dev builds.

### Setting the environment variables

#### Linux

Edit the systemd unit via `systemctl edit warren-daemon.service`:

```ini
[Service]
Environment="WARREN_API_URL=https://api.warrenbrowse.com"
```

Restart the daemon:

```bash
sudo systemctl restart warren-daemon
```

#### macOS

Use `plutil` (plist path to confirm depending on the fork installer):

```bash
sudo plutil -replace EnvironmentVariables -json \
  '{"WARREN_API_URL": "https://api.warrenbrowse.com"}' \
  /Library/LaunchDaemons/net.mullvad.daemon.plist
launchctl unload -w /Library/LaunchDaemons/net.mullvad.daemon.plist
launchctl load   -w /Library/LaunchDaemons/net.mullvad.daemon.plist
```

#### Windows

Inherited from upstream, `setx` from an elevated shell, then `sc.exe stop / start`. Not covered by
the POC fork.

## Environment variables used by the desktop frontend

* `MULLVAD_PATH`: Path of the folder containing the auxiliary tools (`warren-problem-report`) in dev.
  Default: `<repo>/target/debug/`.
* `MULLVAD_DISABLE_UPDATE_NOTIFICATION`: Set to `1` to disable the update notification.

## Electron development commands

- `npm run develop`: develop the app with live-reload
- `npm run lint`: lint the code
- `npm run pack:<OS>`: package the app for distribution (`linux`, `mac`, `win`)
- `npm test`: run the tests

## Tray icon on Linux

The prerequisites vary depending on the desktop environment. If the tray does not appear:

### GNOME

Install the shell extension `AppIndicator and KStatusNotifierItem Support`:
https://extensions.gnome.org/extension/615/appindicator-support/

### Other DEs

Install one of:
- `libappindicator3-1`
- `libappindicator1`
- `libappindicator`

## Repo structure

### Electron app + electron-builder assets

- **desktop/packages/mullvad-vpn/** (the package name is kept to avoid merge conflicts with upstream;
  the Electron `productName` is `Warren VPN`)
  - **assets/**: graphic assets + stylesheets
  - **src/**
    - **main/index.ts**: main process entry
    - **renderer/app.tsx**: renderer process entry
    - **renderer/routes.tsx**: route configuration
    - **renderer/transitions.ts**: transition rules between views
  - **tasks/**: Gulp tasks for build + dev watch
    - **distribution.js**: `electron-builder` config
  - **test/**: Electron GUI tests
- **dist-assets/**: icons, binaries and files used to produce the distributables
  - **binaries/**: submodule (still upstream Mullvad)
  - **linux/**: scripts + config for deb and rpm
  - **pkg-scripts/**: macOS pkg bundle scripts
  - **windows/**: NSIS installer config + assets

### Build, tests, misc

- **build-windows-modules.sh**: compiles the Windows C++ libs
- **build.sh**: working dir sanity check + installer build

### Warren daemon

The daemon is in Rust, multi-crate. The top-level crate that produces the `warren-daemon` binary is
[`mullvad-daemon`](mullvad-daemon/) (upstream package name kept, binary renamed via
`[[bin]] name = "warren-daemon"`).

Like upstream, the code splits into two families:

- `talpid-*` crates: generic VPN library, *agnostic* of the account backend. The fork adds
  [`talpid-warren-tunnel`](talpid-warren-tunnel/), which plugs the Warren QUIC tunnel into the talpid
  state machine.
- `mullvad-*` crates: app-specific code (settings, management interface, GUI integration). The fork
  adds the `warren_*` modules in `mullvad-daemon/src/` (see the list in
  [`docs/warren-architecture.md`](docs/warren-architecture.md)).

Files worth knowing:

- **Cargo.toml**: workspace root. Lists the 52 member crates, the `warren-*`/`warrenguard-*` path
  deps on the two siblings, and a `[patch.crates-io]` that points `quinn`/`quinn-proto`/`quinn-udp`
  at the published `WarrenBrowse/warren-quinn` git-dep (pinned by tag).
- **mullvad-daemon/**: crate that builds the `warren-daemon` binary.
- **mullvad-cli/**: crate that builds the `warren` binary (CLI frontend).
- **talpid-core/**: core of the VPN implementation, Mullvad/Warren agnostic.
- **talpid-warren-tunnel/**: Warren QUIC tunnel adapter for the talpid state machine (fork-only).

## Vocabulary

- **App**: this whole repo = "Warren VPN App".
  - **Daemon**: headless `warren-daemon` process (Rust), exposes a management interface.
  - **Frontend**: any program that connects to the management interface to drive the daemon.
    - **GUI**: Electron + React app (bundled `Warren VPN` binary).
    - **CLI**: Rust `warren` binary (terminal frontend).
- **Warren tunnel**: the Warren QUIC tunnel (Ed25519 TLS handshake). It is the fork's only tunnel
  backend: there is no longer a toggle to enable/disable it.
- **Warren account**: account/device/subscription operations go through warren-api (remote backend,
  Ed25519-signed). The identity comes from the local BIP39 mnemonic. "Create an account" generates a
  fresh mnemonic (mandatory on-screen backup of the phrase before continuing); "Restore" imports an
  existing phrase; "Log out" wipes the mnemonic from this device (a real logout). There is no public
  key login: you identify yourself with the restore phrase.
- **Mnemonic**: BIP39 12 words stored in the OS secret vault (Keychain / DPAPI / file
  `secrets/warren_mnemonic.txt` 0600 on Linux), source of the Ed25519 `SigningKey` that serves as the
  Warren identity.
- **EndpointId / WarrenPubKey**: Ed25519 pubkey (32 bytes) that identifies a Warren exit in the
  `warren-relays.json`.

## File paths used by the Warren app

### Daemon

All paths are defined in the [`mullvad-paths`](mullvad-paths/) crate and include the `WARREN_*`
aliases (priority) + `MULLVAD_*` (compat fallback).

On Windows, when a process runs as a service, `%LOCALAPPDATA%` resolves to
`C:\Windows\system32\config\systemprofile\AppData\Local`.

#### Settings (env override: `WARREN_SETTINGS_DIR`)

| Platform | Path |
|---|---|
| Linux | `/etc/warren-vpn/` |
| macOS | `/etc/warren-vpn/` |
| Windows | `%LOCALAPPDATA%\Warren VPN\` |

#### Logs (env override: `WARREN_LOG_DIR`)

| Platform | Path |
|---|---|
| Linux | `/var/log/warren-vpn/` + systemd |
| macOS | `/var/log/warren-vpn/` |
| Windows | `C:\ProgramData\Warren VPN\` |

#### Cache (env override: `WARREN_CACHE_DIR`)

| Platform | Path |
|---|---|
| Linux | `/var/cache/warren-vpn/` |
| macOS | `/Library/Caches/warren-vpn/` |
| Windows | `C:\ProgramData\Warren VPN\cache` |

#### RPC socket (env override: `WARREN_RPC_SOCKET_PATH`)

| Platform | Path |
|---|---|
| Linux | `/var/run/warren-vpn` |
| macOS | `/var/run/warren-vpn` |
| Windows | `//./pipe/Warren VPN` |

The `PRODUCT_NAME` rename (from `mullvad-vpn` to `warren-vpn`) is deliberate: it avoids
filesystem/socket collisions with an upstream Mullvad client installed in parallel on the same
machine. See `mullvad-paths/tests/warren_collision_safety.rs`.

#### Warren-only files under `<settings_dir>/`

| File | Role |
|---|---|
| `secrets/warren_mnemonic.txt` | BIP39 12-word mnemonic: only the Linux/plaintext fallback (perms 0600, owner root); on macOS/Windows it lives in the Keychain/DPAPI. A legacy `<settings_dir>/warren_mnemonic.txt` file is migrated then deleted at boot. |

#### Warren-only files under `<cache_dir>/`

| File | Role |
|---|---|
| `warren-relays.json` | Ed25519-signed list of Warren exits (v2 format). Format detailed in [`docs/warren-architecture.md`](docs/warren-architecture.md) |

### Desktop Electron app

| Platform | Path |
|---|---|
| Linux | `$XDG_CONFIG_HOME/Warren VPN/gui_settings.json` |
| macOS | `~/Library/Application Support/Warren VPN/gui_settings.json` |
| Windows | `%LOCALAPPDATA%\Warren VPN\gui_settings.json` |

## Icons

See the [graphics README](graphics/README.md). The Warren icons are not integrated yet; the upstream
assets are temporarily reused.

## Locales and translations

General procedure: [locales README](./desktop/packages/mullvad-vpn/locales/README.md). The
user-facing "Mullvad VPN" strings were replaced with "Warren VPN" in commit `22d84f69a7`; the locale
`.po` files have not been re-translated yet, the existing translations may contain "Mullvad".

# License

This repo is a **GPL-3.0** fork of [`mullvadvpn-app`](https://github.com/mullvad/mullvadvpn-app).

Original copyright: (C) 2026  Mullvad VPN AB
Fork modifications: (C) 2026  Warren contributors

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of
the License, or (at your option) any later version.

For the full license agreement, see [LICENSE.md](LICENSE.md).

**Trademarks**: the names "Mullvad" and "Mullvad VPN" and the associated logo are trademarks of
Mullvad VPN AB **not covered by the GPL**. The Warren fork does not use these trademarks in its
distributed binaries (rebranded `productName` + paths + bin names), see
[`UPSTREAM_BASELINE.md`](UPSTREAM_BASELINE.md) section "Known risks".
