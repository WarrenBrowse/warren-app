# Warren VPN Android app

This directory contains the Android project as well as related files and information. The Android
project relies on building the [warren-jni](../warren-jni/) Rust library (the Warren engine,
consumed from the sibling `warrenguard`, `warren-sdk-rs` and `warren-contract` repositories) as a
shared library bundled into the assembled APK.

The app will be available for download on
[our website](https://warrenbrowse.com/download) and
[GitHub Releases](https://github.com/WarrenBrowse/warren-app/releases).

## Quick start

### Browsing the source code

The content in this directory (`<repository-root>/android`) follows a standard Android project
structure and can therefore be opened in Android Studio or any other IDE or editor of your choice.

### Building the app

The easiest and recommended way to build the Android project is to use the containerized build
(which requires `podman`):
```
../building/containerized-build.sh android
```
See the [build instructions](./docs/BuildInstructions.md) for further information.

## Linting and formatting

### Kotlin formatting
`ktfmt` is used for kotlin formatting.

See the [official documentation](https://github.com/facebook/ktfmt) for how to use it as default
formatter in Android Studio. Ensure to set `kotlinLangStyle` as "Code Style" and to set the project
to rely on the `EditorConfig` (`.editorconfig` file).

Also, see the [`ktfmt` gradle plugin documentation](https://github.com/cortinico/ktfmt-gradle) for
how to use it as a gradle task.

### XML formatting
In order to format XML files, the script `scripts/tidy.sh` is used. As the script name implies, it's basically a helper script to run the tool called `tidy`. It needs to be installed unless the
container image is used.

Command to format:
```
scripts/tidy.sh format
```

Command to format and check for any changes:
```
scripts/tidy.sh formatAndCheckDiff
```

#### macOS
Since macOS is using a different version of `sed` running the tidy script (`scripts/tidy.sh`) will
lead to the creation of a large number of files ending with `-e`. The recommended fix for this
issue is to install the gnu version of `sed`. This can be done by running:
`brew install gnu-sed` and then set `gnu-sed` as your default `sed`.

### Android Gradle Plugin lint tool

The Android Gradle Plugin lint tool for linting of resources and code. The tool is configured to be
strict. See each `build.gradle.kts`for more information.

## Translations and localization

Android strings are written directly in every locale: a key added to
`lib/ui/resource/src/main/res/values/strings.xml` is added to the 23 `values-<locale>/strings.xml`
files in the same change. `StringResourceCoverageTest` (in `:app`) fails on a missing or stale key,
a dash, a placeholder the English string does not have, or a plural quantity a language needs.
The [locale README][desktop-locales-readme] lists the terms that stay untranslated.

[desktop-locales-readme]: ../desktop/packages/mullvad-vpn/locales/README.md

## Icons and assets

For a general overview of icons and graphics read [the graphics readme](../graphics/README.md).
