These are instructions on how to build the app on desktop platforms. See the
[readme](./README.md#building-the-app) for help building on other platforms.

# Warren prerequisites: three sibling repositories

**Read this before any `cargo build` or `./build.sh`.**

The Warren fork consumes crates by `path` from three sibling repositories,
checked out next to `warren-app` (the fork has **no dependency on
`warren-core`**):

- [`warrenguard`](https://github.com/WarrenBrowse/warrenguard): the data-plane
  engine (`warrenguard-transport`, `-config`, `-wire`, `-multihop`, `-relay`,
  ...). Pinned in `.warrenguard-version`.
- [`warren-sdk-rs`](https://github.com/WarrenBrowse/warren-sdk-rs): the client
  SDK (`warren-api`, `warren-identity`). Pinned in `.warren-sdk-version`.
- [`warren-contract`](https://github.com/WarrenBrowse/warren-contract): the
  client/server contract (`warren-contract`, `warren-discovery-core`). Pinned in
  `.warren-contract-version`.

Expected layout:

```
<workspace>/
├── warren-app/        this repository (Mullvad fork)
├── warrenguard/       data-plane engine (warrenguard-* crates)
├── warren-sdk-rs/     client SDK (warren-api, warren-identity)
└── warren-contract/   contract (warren-contract, warren-discovery-core)
```

All four repositories are public. Clone them over HTTPS and check each sibling
out at the commit this tree pins:

```bash
cd /path/to/workspace
for repo in warren-app warrenguard warren-sdk-rs warren-contract; do
  git clone "https://github.com/WarrenBrowse/$repo.git"
done
cd warren-app
git submodule update --init
git -C ../warrenguard checkout --detach "$(cat .warrenguard-version)"
git -C ../warren-sdk-rs checkout --detach "$(cat .warren-sdk-version)"
git -C ../warren-contract checkout --detach "$(cat .warren-contract-version)"
```

A sibling at another commit than its pin may not compile against this tree. CI
checks them out at the pins. After moving a pin, regenerate `Cargo.lock` with
`scripts/dev/regen-lockfile.sh`, never a bare `cargo update`.

When a sibling is missing or at another path, `cargo metadata` and `./build.sh`
fail with:

```
error: failed to load manifest for dependency `warrenguard-transport`
Caused by: failed to read `../warrenguard/crates/warrenguard-transport/Cargo.toml`
Caused by: No such file or directory (os error 2)
```

The path dependencies are declared in the crate manifests (`talpid-core`,
`talpid-warren-tunnel`, `mullvad-daemon`, `mullvad-types`, `warren-jni`,
`warren-ios`) and in the `[patch."https://github.com/WarrenBrowse/warrenguard.git"]`
section of the root `Cargo.toml`. The quinn fork is the published git
dependency `WarrenBrowse/warren-quinn` (pinned by tag), wired through
`[patch.crates-io]`.

# Product environments (prod | staging | beta)

One build-time selector, the `WARREN_PRODUCT_ENV` env var (unset = `prod`),
switches the whole product configuration: API host, desktop update channel
(`{api}/updates/desktop`), install/settings/cache/log paths, RPC socket,
service names and packaging identity. The single source of truth is the
`warren-product-env` crate (`warren-product-env/src/lib.rs`); the desktop
JS mirror is `desktop/packages/mullvad-vpn/src/shared/constants/product-env.ts`
and the packaging table lives in
`desktop/packages/mullvad-vpn/tasks/distribution.cjs`. A beta build is a
separately installable app (`Warren VPN Beta`, `com.warrenbrowse.vpn.beta`)
that coexists with a prod install without sharing any state.

- Desktop: `./build.sh --beta [...]`, or `WARREN_PRODUCT_ENV=beta ./build.sh`
  (the script exports it to cargo, vite and electron-builder; it validates
  the value and defaults to prod).
- Android: build the matching flavor, e.g. `./gradlew assembleBetaDebug` from
  `android/`. The flavor drives `WARREN_PRODUCT_ENV` for the cargo build and
  a per-variant guard refuses an APK whose `libwarren_jni.so` was compiled
  for another environment. One flavor per Gradle invocation.
- iOS: the Xcode project has a configuration for prod (Debug/Release) and one
  for staging; beta is built by overriding `API_HOST_NAME`/`API_ENDPOINT` on
  the `xcodebuild` line, which is what `scripts/dev/launch-ios.sh --beta`
  does. The three share one bundle id, so they replace each other.
- Plain `cargo build`/`cargo test` stay prod; prefix with
  `WARREN_PRODUCT_ENV=beta` to compile/test the beta configuration.

The dev launchers take the environment as `--prod`, `--beta` or `--staging`
(falling back to `WARREN_PRODUCT_ENV`) and print their help instead of
building anything when neither is given: `scripts/dev/warren-dev.sh`,
`scripts/dev/launch-android.sh`, `scripts/dev/launch-ios.sh`,
`scripts/dev/macos-vpn-smoke.sh`, `scripts/dev/macos-vpn-reconnect-smoke.sh`
and `scripts/dev/windows/build-{daemon,app}.sh`. The shared selection helper
is `scripts/utils/product-env.sh`, whose host and path tables the
`warren-product-env` crate pins with a drift test.

Runtime overrides of the API and update URLs (`WARREN_API_URL`,
`WARREN_UPDATE_URL`, the API URL setting) apply on top of the compiled
defaults in development builds only (the `api-override` feature, which
`build.sh` turns on without `--optimize`); a release build ignores them.

# Install toolchains and dependencies

These instructions are probably not complete. If you find something more that needs installing
on your platform please submit an issue or a pull request.

## All platforms

- Get the latest **stable** Rust toolchain via [rustup.rs](https://rustup.rs/).
  - Install default targets and components needed for your platform:
    ```bash
    ./scripts/setup-rust android|ios|windows|linux|macos
     ```
  - (Optional) Run the following to install a git `post-checkout` hook that will automatically
    run the `setup-rust` script when the Rust version specified in the `rust-toolchain.toml` file changes:
    ```bash
    ./scripts/setup-rust install-hook
    ```

- You need Node.js and npm. You can find the exact versions in the `volta` section of
  `desktop/package.json`. The toolchain is managed by volta. This is optional if using `--daemon-only`.

  - Linux

    Follow instructions on [volta.sh](https://volta.sh/)

  - macOS

    ```bash
    brew install volta && volta setup
    ````

  - Windows

    Install the `msi` hosted here: https://github.com/volta-cli/volta

- Install a protobuf compiler (version 3.15 and up), it can be installed on most major Linux distros
  via the package name `protobuf-compiler`, `protobuf` on macOS via Homebrew, and on Windows
  binaries are available on their GitHub [page](https://github.com/protocolbuffers/protobuf/releases)
  and they have to be put in `%PATH`. An additional package might also be required depending on
  Linux distro:
  - `protobuf-devel` on Fedora.
  - `libprotobuf-dev` and `protobuf-compiler` on Debian/Ubuntu.

- **`bash` must be installed and available in PATH on all platforms**. This is required for building
  the desktop app:
- Bash version 4.0 or later is required for all platforms and must be added to your PATH environment variable.
  - Linux: Bash is typically installed by default, otherwise refer to your distribution for instructions on how to install it.
  - macOS: The default installed version (3.2.5) is not supported. Install a newer version via Homebrew: `brew install bash`
  - Windows: Install [Git for Windows] which includes Git Bash and other required unix utilities.

- Install `podman`: https://podman.io/
  - Only needed to regenerate the gRPC bindings of the Electron app after a change to
    `mullvad-management-interface/proto/`; the generated bindings are committed. The generator
    still runs in Mullvad's published image
    (`desktop/packages/management-interface/scripts/container-run-generate-bindings.sh`).


[Git for Windows]: https://git-scm.com/download/win

## Linux

### Debian/Ubuntu

```bash
# For building the daemon
sudo apt install gcc libdbus-1-dev
# For building the installer
sudo apt install rpm
```

#### Warren fork: extra Linux dependencies

On Debian 12 or Ubuntu 22.04 and later, in addition to the packages above:

```bash
sudo apt install \
  protobuf-compiler libprotobuf-dev \
  build-essential clang pkg-config libssl-dev cmake git
```

- `libprotobuf-dev` provides the standard `.proto` files
  (`google/protobuf/empty.proto`, `timestamp.proto`, ...) that
  `mullvad-management-interface` imports.
- `libmnl` and `libnftnl` need no package: the build links the static copies in
  the `dist-assets/binaries` submodule (`.cargo/config.toml`).
- `git` is optional: the daemon's build script reads the commit date from it for
  its startup log line, and builds without it from a source tarball.

The release builds run natively in the Debian 11 job image
(`ghcr.io/warrenbrowse/warren-ci-linux`, see `release.yml`), whose glibc 2.31 is
the floor of every Linux binary shipped. There is no supported cross build from
macOS: build on a Linux host or in that image.

### Fedora/RHEL

```bash
# For building the daemon
sudo dnf install gcc dbus-devel
# For building the installer
sudo dnf install rpm-build
```

## Using nix devshell

This is supported on Linux (x86_64) as well as macOS (x86_64 and aarch64).
[Install nix](./android/docs/BuildInstructions.md#Build-using-nix-devshell) if you haven't already.

   ```bash
   nix develop
   ```

#### direnv

Provided in the repository root is a [direnv](https://direnv.net/) for automatically sourcing the devshell.
Allow it by executing `direnv allow .` (in `<repository>`) once.

### Cross-compiling for ARM64

By default, the app will build for the host platform. It is also possible to cross-compile the app
for ARM64 on x64.

#### Debian

```bash
# As root
dpkg --add-architecture arm64 && \
    apt update && \
    apt install libdbus-1-dev:arm64 gcc-aarch64-linux-gnu
```

```bash
rustup target add aarch64-unknown-linux-gnu
```

To make sure the right linker and libraries are used, add the following to `~/.cargo/config.toml`:

```
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"

[target.aarch64-unknown-linux-gnu.dbus]
rustc-link-search = ["/usr/aarch64-linux-gnu/lib"]
rustc-link-lib = ["dbus-1"]
```

## Windows

The host has to have the following installed:

- Microsoft's _Build Tools for Visual Studio 2022_ (a regular installation of Visual Studio 2022
  Community or Pro edition works as well).

- Windows 10 (or Windows 11) SDK.

- `bash` and base Unix utilities must be installed and available in PATH (see the requirement in
  the [All platforms](#all-platforms) section above).

- `msbuild.exe` available in `%PATH%`. If you installed Visual Studio Community edition, the
  binary can be found under:

  ```
  C:\Program Files\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\<arch>
  ```

  Where `<arch>` refers to the host architecture, either `amd64` or `arm64`.

  The environment can also be set up in bash by sourcing `vcvars.sh`: `. ./scripts/vcvars.sh`. Note
  that that script assumes that you're running VS 2022 Community.

- The `x86` target is required for building some NSIS plugins:

  ```bash
  rustup target add i686-pc-windows-msvc
  ```

### Cross-compiling for ARM64

By default, the app will build for the host platform. It is also possible to cross-compile the app
for ARM64 on x64. This requires:

- The ARM64 MSVC tools added to Visual Studio.

- `clang` (either directly from llvm.org or as part of Visual Studio) on the `PATH`.

- The `AArch64` target added to Rust:

```bash
rustup target add aarch64-pc-windows-msvc
```

### Compiling *on* Windows Arm

In addition to the above requirements:

- `clang` is required (can be found in the Visual Studio installer) in `PATH`.

  `INCLUDE` also needs to include the correct headers for clang. This can be found by running
  `vcvarsall.bat arm64` and typing `set INCLUDE`.

  The environment can also be set up in bash by sourcing `vcvars.sh`: `. ./scripts/vcvars.sh`. Note
  that that script assumes that you're running VS 2022 Community.

## macOS

The host has to have the following installed:

- A recent version of `bash`. The default version in macOS (3.2.57) isn't supported.

# Building and packaging the app

The simplest way to build the entire app and generate an installer is to just run the build script.
`--optimize` can be added to enable compiler optimizations. This will take longer to build but will
produce a smaller installer and installed binaries:
```bash
./build.sh [--optimize]
```
This should produce an installer exe, pkg or rpm+deb file in the `dist/` directory.

Building this requires at least 1GB of memory.

## Notes on options

- `--daemon-only` - This will build daemon only Linux packages (e.g. `warren-vpn-daemon`). You will need to install additional build tools: `cargo install cargo-deb cargo-generate-rpm`.

## Notes on targeting ARM64

### macOS

By default, `build.sh` produces a pkg for your current architecture only. To build a universal
app that works on both Intel and Apple Silicon macs, build with `--universal`.

### Linux

To cross-compile for ARM64 rather than the current architecture, set the `TARGETS` environment
variable to `aarch64-unknown-linux-gnu`:

```bash
TARGETS="aarch64-unknown-linux-gnu" ./build.sh
```

### Windows

To cross-compile for ARM64 from another host architecture, set the `TARGETS` environment
variable to `aarch64-pc-windows-msvc`:

```bash
TARGETS="aarch64-pc-windows-msvc" ./build.sh
```

## Notes on building on ARM64 Linux hosts

You may have to specify `USE_SYSTEM_FPM=true` to generate the deb/rpm packages.

## Notes on building for RISC-V

These instructions are for building, packaging and installing the Warren VPN daemon and CLI interface for use with RISC-V systems. It includes support for RVA22 and RVA23 profiles; for example, on Framework Laptop 13 with [DeepComputing RISC-V Mainboards](https://frame.work/marketplace/mainboards?search=risc-v) running Ubuntu.

Build and package:
```bash
./build.sh --daemon-only --optimize
```

Install on Debian/Ubuntu:
```bash
sudo dpkg -i dist/warren-vpn-daemon_<version>_riscv64.deb
```

Install on Fedora:
```bash
sudo dnf install dist/warren-vpn-daemon_<version>_riscv64.rpm
```

### Cross-compling for RISC-V from other hosts such as Debian/Ubuntu AMD64

Install dependencies:
```bash
# As root
dpkg --add-architecture riscv64 && \
    apt update && \
    apt install libdbus-1-dev:riscv64 gcc-riscv64-linux-gnu
```

Add target to Rust toolchain:
```bash
rustup target add riscv64gc-unknown-linux-gnu
```

Update your local Cargo configuration to use the correct linker and libraries, add the
following `~/.cargo/config.toml`:
```
[target.riscv64gc-unknown-linux-gnu]
linker = "riscv64-linux-gnu-gcc"

[target.riscv64gc-unknown-linux-gnu.dbus]
rustc-link-search = ["/usr/riscv64-linux-gnu/lib"]
rustc-link-lib = ["dbus-1"]
```

Build and package:
```bash
TARGETS="riscv64gc-unknown-linux-gnu" ./build.sh --daemon-only --optimize
```

# Building and running warren-daemon

This section is for building the system service individually.

> **Note (Warren fork)**: the Cargo package is still named `mullvad-daemon`, but
> the binary it produces is `warren-daemon`. Likewise `WARREN_RESOURCE_DIR` is
> the variable read first, with `MULLVAD_RESOURCE_DIR` still accepted as a
> fallback.

1. On Windows, build the C++ libraries:
    ```bash
    ./build-windows-modules.sh
    ```
    And copy the `wintun.dll` next to the daemon binary:
    ```bash
    cp dist-assets/binaries/x86_64-pc-windows-msvc/wintun/wintun.dll target/debug/
    ```

1. Build the system daemon plus the other Rust tools and programs:
    ```bash
    cargo build
    ```

1. On Windows, the daemon must be run as the SYSTEM user. You can use
    [PsExec](https://docs.microsoft.com/en-us/sysinternals/downloads/psexec) to launch
    an elevated bash instance before starting the daemon in it:
    ```
    psexec64 -i -s bash.exe
    ```

1. Run the daemon with verbose logging (from the root directory of the project):
    ```bash
    sudo WARREN_RESOURCE_DIR="./dist-assets" ./target/debug/warren-daemon -vv
    ```
    Leave out `sudo` on Windows. The daemon must run as root since it modifies the firewall and sets
    up virtual network interfaces etc.

# Building and running the desktop app

This section is for building the desktop app individually.

1. Go to the `desktop` directory
   ```bash
   cd desktop
   ```

1. Install all the JavaScript dependencies by running:
    ```bash
    npm run install
    ```

1. Start the Electron app in development mode by running:
    ```bash
    npm run -w mullvad-vpn develop
    ```

If you change any javascript file while the development mode is running it will automatically
transpile and reload the file so that the changes are visible almost immediately.

Please note that the Electron app needs a running daemon to connect to in order to work. See
[Building and running warren-daemon](#building-and-running-warren-daemon) for instructions
on how to do that before starting the Electron app.
