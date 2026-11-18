# Build instructions

This document aims to explain how to build the Warren Android app. The native libraries compile
the sibling repositories (`warrenguard`, `warren-sdk-rs`, `warren-contract`) by path, so the build
runs on a machine with the whole workspace checked out, natively or in the nix devshell.

## Build process

The build process consist of two main steps. First building the native libraries (`warren-jni`)
and then building the Android app/project which will bundle the previously built native libraries.
Building the native libraries requires some specific toolchains and packages to be installed, listed
below.

The native libraries doesn't have to be rebuilt very often, only when including daemon changes or
after cleaning the project, so apart from that it's possible to build the Android app/project using
the Gradle CLI or the Android Studio GUI.

## Build natively

> __*NOTE:*__ This guide is only supported on Linux and may not work on other platforms, if you are
> using macOS please refer to [macOS build instructions](./docs/BuildInstructions.macos.md)

It requires installing multiple Sdk:s and toolchains.

### Setup build environment
These steps explain how to manually setup the build environment on a Linux system.

#### 1. Install `protobuf-compiler`
Install a protobuf compiler (version 3 and up), it can be installed on most major Linux distros via
the package name `protobuf-compiler`. An additional package might also be required depending on
Linux distro:
- `protobuf-devel` on Fedora.
- `libprotobuf-dev` on Debian/Ubuntu.

#### 2. Install `gcc`

#### 3. Install Android toolchain

- Install the JDK

  ```bash
  sudo apt install zip openjdk-21-jdk
  ```

- Install the SDK and NDK

  The SDK should be placed in a separate directory, like for example `~/android` or `/opt/android`.
  This directory should be exported as the `$ANDROID_HOME` environment variable.

  Note: if `sdkmanager` fails to find the SDK root path, pass the option `--sdk_root=$ANDROID_HOME`
  to the command above.

  ```bash
  cd /opt/android     # Or some other directory to place the Android SDK
  export ANDROID_HOME=$PWD

  wget https://dl.google.com/android/repository/commandlinetools-linux-13114758_latest.zip
  mkdir -p cmdline-tools
  unzip commandlinetools-linux-13114758_latest.zip -d cmdline-tools-latest
  mv cmdline-tools-latest/cmdline-tools cmdline-tools/latest && rm -d cmdline-tools-latest
  ./cmdline-tools/latest/bin/sdkmanager "platforms;android-36" "build-tools;36.0.0" "platform-tools" "ndk;27.3.13750724"
  ```

#### 4. Install and configure Rust toolchain

- Get the latest **stable** Rust toolchain via [rustup.rs](https://rustup.rs/).

- Configure Android cross-compilation targets and set up linker and archiver. This can be done by setting the following
environment variables:

  Add to `~/.bashrc` or equivalent:
  ```
  export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.3.13750724"
  export NDK_TOOLCHAIN_DIR="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
  export AR_aarch64_linux_android="$NDK_TOOLCHAIN_DIR/llvm-ar"
  export AR_armv7_linux_androideabi="$NDK_TOOLCHAIN_DIR/llvm-ar"
  export AR_x86_64_linux_android="$NDK_TOOLCHAIN_DIR/llvm-ar"
  export AR_i686_linux_android="$NDK_TOOLCHAIN_DIR/llvm-ar"
  export CC_aarch64_linux_android="$NDK_TOOLCHAIN_DIR/aarch64-linux-android26-clang"
  export CC_armv7_linux_androideabi="$NDK_TOOLCHAIN_DIR/armv7a-linux-androideabi26-clang"
  export CC_x86_64_linux_android="$NDK_TOOLCHAIN_DIR/x86_64-linux-android26-clang"
  export CC_i686_linux_android="$NDK_TOOLCHAIN_DIR/i686-linux-android26-clang"
  export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN_DIR/aarch64-linux-android26-clang"
  export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$NDK_TOOLCHAIN_DIR/armv7a-linux-androideabi26-clang"
  export CARGO_TARGET_I686_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN_DIR/i686-linux-android26-clang"
  export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN_DIR/x86_64-linux-android26-clang"
  ```

- Install Android targets
  ```bash
  ./scripts/setup-rust android
  ```

- (Optional) Run the following to install a git `post-checkout` hook that will automatically
  run the `setup-rust` script when the Rust version specified in the `rust-toolchain.toml` file changes:
  ```bash
  .scripts/setup-rust install-hook
  ```

### Debug build
Run the following command to build a debug build:
```bash
./gradlew assembleProdDebug
```

### Beta and staging flavors (separate installable apps)
The `beta` and `staging` infrastructure flavors build their own product:
`com.warrenbrowse.vpn.beta` / `com.warrenbrowse.vpn.staging`, launcher labels
"Warren VPN Beta" / "Warren VPN Staging", and a `libwarren_jni.so` compiled
with the matching `WARREN_PRODUCT_ENV` (that environment's API host; the
flavor drives the cargo build, and a per-variant guard refuses a mismatched
`.so`). Build one flavor per Gradle invocation:
```bash
./gradlew assembleBetaDebug        # or assembleBetaRelease
./gradlew assembleStagingDebug     # or assembleStagingRelease
```
`scripts/dev/launch-android.sh --beta|--staging|--prod` does the build,
install and launch in one step, and passes the environment to Gradle
explicitly.

The `.so` files of every flavor land in one shared directory and cargo only
rewrites the ABIs it was asked for, so switching flavor (or narrowing the ABI
list with `-Pwarren.app.build.cargo.targets=arm64`) drops that directory
first. An APK then either carries this environment's datapath for an ABI or
carries none, never another environment's.

### Release build
1. Configure a signing key by following [these instructions](#configure-signing-key).
2. Run the following command to build:
   ```bash
   ./gradlew fullRelease
   ```
3. Sign the release artifacts using apksigner with the following command:
   ```bash
   apksigner sign --ks app-keys.jks (WarrenVPN)(version)(.apk|.aab)
   ```

### Release build without a keystore
The signing inputs are the four Gradle properties `warren.keystore.path`,
`warren.keystore.password`, `warren.key.alias` and `warren.key.password`, or
their environment fallbacks `WARREN_KEYSTORE_PATH`, `WARREN_KEYSTORE_PASSWORD`,
`WARREN_KEY_ALIAS` and `WARREN_KEY_PASSWORD`. When any of them is missing,
`assembleBetaRelease` still produces an installable APK: `app/build.gradle.kts`
signs the release build type with the AGP debug keystore
(`~/.android/debug.keystore`) instead of leaving the signing config null, which
would yield an `app-<flavor>-release-unsigned.apk` that no device installs.
A prod release refuses to build instead (`checkProdReleaseSigning`, run before
`preProdReleaseBuild`), and the release workflow fails a `v*` tag that has no
keystore secret.

This is what the release workflow ships for the beta while no upload keystore
exists. The beta is distributed as a direct APK, and Android upgrades an app in
place only when the new APK carries the same signer as the installed one. The CI
runner's debug keystore signed every beta APK published so far, so signing the
optimised build (R8, release-profile Rust) with that key keeps every existing
install upgrading. Until 1.1.28 the workflow fell back to the debug build type
instead, an unoptimised datapath that held a Fairphone 3 at 59 % CPU. The prod
flavor never takes this path: a store install signed with a key that lives on a
build machine could be updated by anyone holding a copy of that file, and the
move to a real key would break its updates. The release keystore signs with
scheme v3 so that a later key rotation (`apksigner rotate`) can carry the
lineage to existing installs.

The key is whatever `~/.android/debug.keystore` the build machine holds, and AGP
generates a new one wherever it finds none. Two consequences:

- A developer's own machine signs with a different debug key. A release build
  from your laptop installs on a clean device but does not upgrade a published
  beta (`INSTALL_FAILED_UPDATE_INCOMPATIBLE`), and uninstalling to get past
  that erases the wallet. Reproduce an upgrade with the published APK only.
- The published signer is pinned in `android/fallback-signer.sha256`: the
  SHA-256 of the signer certificate as `apksigner verify --print-certs` prints
  it, hex, no colons. On the fallback path the release workflow runs
  `ci/verify-android-signer.sh` against the built APK and fails the job on a
  mismatch, so a runner rebuilt on another HOME cannot rotate the key without
  anyone noticing. `ci/stage-release-assets.sh` refuses an unsigned APK and
  stages the R8 mapping beside it as
  `WarrenVPN[-Beta]-<ver>-android-mapping.txt.gz`, the file that symbolicates
  the stack traces in a problem report from that build.

A configured keystore takes precedence over the fallback.

## Build using nix devshell
This is supported on Linux (x86_64) as well as macOS (x86_64 and aarch64).

1. Install the nix package manager by following the [official instructions](https://nixos.org/download/).
2. Enable the experimental `nix-command` and `flake` features by following [these instructions](https://nixos.wiki/wiki/flakes).
3. Launch a devshell (in `<repository>/android`) by running:
   ```bash
   nix develop .#android
   ```
4. Build the app as usual by running for example:
   ```bash
   build
   ```
   or
   ```bash
   ./gradlew assembleProdDebug
   ```

## Configure signing key
Generate a key/keystore named `app-keys.jks` and make sure to write
down the used passwords:
   ```bash
   keytool -genkey -v -keystore app-keys.jks -alias release -keyalg RSA -keysize 4096 -validity 10000
   ```

## Creating a release

The Android app ships with the desktop release, on the same tag and version:
see [docs/RUNBOOK-RELEASE.md](../../docs/RUNBOOK-RELEASE.md).

## Gradle dependency metadata verification lockfile
This lockfile helps ensuring the integrity of the gradle dependencies in the project.

### Update lockfile
When adding or updating dependencies, it's necessary to also update the lockfile. This can be done
in the following way:

1. Run update script:
   ```bash
   ./scripts/lockfile -u
   ```

   If you're on macOS make sure GNU sed is installed. Install with `brew install gnu-sed` and add it to your `PATH` so that it is used instead of the `sed` macOS ships with `PATH="$HOMEBREW_PREFIX/opt/gnu-sed/libexec/gnubin:$PATH"`

2. Check diff before committing.

### Disable during development
This is easiest done by temporarily removing the lockfile:
```bash
rm ./gradle/verification-metadata.xml
```

## Gradle properties
Some gradle properties can be set to simplify development, for the full list see `android/gradle.properties`.
In order to override them, add the properties in `<USER_GRADLE_HOME>/gradle.properties`. See the
[gradle documentation](https://docs.gradle.org/current/userguide/build_environment.html#sec:project_properties)
for more info of the prioritization of properties.

### Override version code and version name
To avoid or override the rust based version generation, the `warren.app.config.override.versionCode` and
`warren.app.config.override.versionName` properties can be set:
```
warren.app.config.override.versionCode=123
warren.app.config.override.versionName=1.2.3
```

### Disable version in-app notifications
To disable in-app notifications related to the app version during development or testing,
the `warren.app.config.inAppVersionNotifications.enable` property can be set:
```
warren.app.config.inAppVersionNotifications.enable=false
```

### Run tests highly affected by rate limiting
To avoid being rate limited we avoid running tests sending requests that are highly rate limited
too often. If you want to run these tests you can override the
`warren.test.e2e.config.runHighlyRateLimitedTests` gradle properties. The default value is `false`.

### Compose compiler reports and stability

Every module with composables shares the Compose compiler configuration of
`gradle/build-logic/src/main/kotlin/utilities/ComposeCompiler.kt`: the stability
configuration file `compose_compiler_config.conf` (the model classes composables
take as parameters that the compiler cannot prove stable), and, when
`warren.app.build.compose.reports=true` is set, the reports and metrics in each
module's `build/compose_reports` and `build/compose_metrics`. There,
`*-classes.txt` names every unstable class and `*-composables.txt` every
composable that is restartable but not skippable. The property is off by
default because the reports are an input of the build cache key.

### Frame accounting on the device

`MainActivity` tracks every frame with JankStats and folds the frames that
missed their deadline into one logcat line per second, in every build and
nowhere else (no telemetry):

```bash
adb logcat -s WarrenJank
```

### Baseline profile

`app/src/main/baseline-prof.txt` is generated, never edited, by
`test/baselineprofile` on Warren's own flow (the cold start to the Connect
screen, the picker, the settings tree):

```bash
./gradlew :app:generateProdReleaseBaselineProfile
```

The task builds the `prodNonMinifiedRelease` app and its generator, installs
both on the connected device, walks the flow, and moves the profile into
`app/src/main/`. The prod flavor is the one to run when the device holds a
beta install worth keeping: the two flavors share every class, and the
generator's install replaces (and its data clear wipes) whichever app carries
the application id it targets. Every release build then fails when the file is
missing or names no Warren class (`checkBaselineProfile` in
`app/build.gradle.kts`), so a regeneration that lands nothing cannot ship a
profile-less APK unnoticed.

The generator clears the package data and walks the privacy disclosure and the
onboarding wizard to a fresh wallet, so run it on a device (API 33+, or rooted
API 28+) that holds no wallet worth keeping. Regenerate it when the startup
path or the main screens change substantially; the `betaNonMinifiedRelease` and
`betaBenchmarkRelease` variants exist for that purpose.

## Reproducible builds

No job verifies today that the published APK can be rebuilt bit for bit. The
inherited verification (an F-Droid recipe and a workflow comparing a container
build with an fdroidserver build) targeted build variants this fork removed and
was deleted with them. A verification for this fork compares two independent
`assembleBetaRelease` builds of the same tag, signatures stripped
(`zip -d <apk> "META-INF/*"`), by checksum.
