# Making a Warren VPN release

When making a real Warren VPN release there are a couple of steps to follow.
`<VERSION>` here will denote the version of the app you are going to release.
For example `2026.5.0` or `2026.5.0-beta1`.

The release pipeline runs in GitHub Actions on `WarrenBrowse/warren-app`
(`.github/workflows/release.yml`) and is triggered by a tag push matching
`v*.*.*`. Builds happen on macos-14, ubuntu-22.04 and windows-2022 runners,
signing keys are sourced from `WARREN_*` GitHub Secrets, and the resulting
DMG/PKG/.deb/.rpm/MSI/EXE artifacts are uploaded to a draft GitHub Release.

## Pre-flight

1. Follow the [Install toolchains and dependencies](BuildInstructions.md#install-toolchains-and-dependencies) steps
   if you have not already completed them.

2. Make sure the `CHANGELOG.md` is up to date and reflects all the changes
   present in this release. Change the `[Unreleased]` header into
   `[<VERSION>] - <DATE>` and add a new `[Unreleased]` header at the top.
   Push this, get it reviewed and merged to `main`.

3. Make sure `.warrenguard-version`, `.warren-sdk-version` and
   `.warren-contract-version` pin the sibling HEADs you want to ship, and that
   `Cargo.lock` still pins the `warren-quinn` fork (`build.sh` and CI fail
   loudly otherwise).

## Tag the release

Run `./prepare-release.sh [--desktop] [--android] <VERSION>`. This will:

  1. Verify the working tree is clean and the version format is valid
  2. Update `desktop/packages/mullvad-vpn/package.json` with the new version
     and commit it
  3. Create a signed git tag `v<VERSION>` on the current commit

Push the commit and the tag to `origin/main`:

```bash
git push origin main
git push origin v<VERSION>
```

The tag push triggers `.github/workflows/release.yml`, which builds and
publishes a *draft* GitHub Release. Inspect the draft release on
`https://github.com/WarrenBrowse/warren-app/releases` and publish it when
you have verified the artifacts.

## GitHub Secrets configuration

The release pipeline requires the following secrets to be configured at
`WarrenBrowse/warren-app -> Settings -> Secrets and variables -> Actions`:

### Required for any build (even unsigned)

* **`WARREN_CORE_RO_TOKEN`** - PAT with read access to
  `WarrenBrowse/warren-core`. Used by every build job to checkout the
  warren-core sibling repository (Warren's path-deps live there).

### Required for signed macOS builds

* **`WARREN_CSC_LINK_MACOS`** - The macOS Developer ID Application
  certificate (`.p12`) base64-encoded. Must contain both the "Developer ID
  Application" and the "Developer ID Installer" certificates with private
  keys. Generate the base64 with:

  ```bash
  base64 -i Warren-Developer-ID.p12 | pbcopy
  ```

  Paste into the secret value field.

* **`WARREN_CSC_KEY_PASSWORD_MACOS`** - The password protecting the
  `.p12` file.

* **`WARREN_NOTARIZE_KEYCHAIN`** - Path to a keychain stored on the runner
  that holds the Apple notarytool profile. Typically created with
  `xcrun notarytool store-credentials <profile> --keychain <keychain>`.

* **`WARREN_NOTARIZE_KEYCHAIN_PROFILE`** - The notarytool profile name.

### Required for signed Windows builds

* **`WARREN_CSC_LINK_WIN`** - The Authenticode code-signing certificate
  (`.pfx`) base64-encoded.

* **`WARREN_CSC_KEY_PASSWORD_WIN`** - The password protecting the `.pfx`.

  The release pipeline imports the `.pfx` into the runner's
  `Cert:\CurrentUser\My` store and resolves the thumbprint at runtime, so
  no separate `WARREN_CERT_HASH_WIN` secret is necessary.

## Local builds

For developer/manual builds (not CI), `build.sh` honors both upstream
Mullvad env vars (`CSC_LINK`, `CSC_KEY_PASSWORD`, `CERT_HASH`,
`NOTARIZE_KEYCHAIN`, `NOTARIZE_KEYCHAIN_PROFILE`) and their Warren-prefixed
equivalents (`WARREN_CSC_LINK_MACOS`, `WARREN_CSC_KEY_PASSWORD_MACOS`,
`WARREN_CERT_HASH`, `WARREN_NOTARIZE_KEYCHAIN`,
`WARREN_NOTARIZE_KEYCHAIN_PROFILE`). The Warren variant takes precedence
when both are set.

Set them in your shell with `HISTCONTROL=ignorespace` so they do not leak
into bash history:

```bash
export HISTCONTROL=ignorespace
 export WARREN_CSC_LINK_MACOS=/path/to/Warren-Developer-ID.p12
 export WARREN_CSC_KEY_PASSWORD_MACOS='my secret'
 export WARREN_NOTARIZE_KEYCHAIN=/Users/<you>/Library/Keychains/notarytool.keychain-db
 export WARREN_NOTARIZE_KEYCHAIN_PROFILE=warren-notary
```

Then run `./build.sh --optimize --sign --notarize --universal` (macOS) or
the equivalent on Linux/Windows. The script will produce signed artifacts
under `dist/` matching `WarrenVPN-<VERSION>*`.

## Apple notarytool credentials

To create a notarytool profile:

  1. Generate an app-specific password on Apple's Apple ID management
     portal (https://appleid.apple.com -> Sign-In and Security -> App
     specific passwords). Do *not* use your real Apple ID password.

  2. Run:

     ```bash
     xcrun notarytool store-credentials warren-notary \
         --keychain /Users/<you>/Library/Keychains/notarytool.keychain-db
     ```

     Leave the first prompt empty (no team ID prefix), then fill in the
     Apple ID, the app-specific password generated in step 1, and your
     team ID (e.g., `A12B34C56D`).

  3. Set `WARREN_NOTARIZE_KEYCHAIN` and `WARREN_NOTARIZE_KEYCHAIN_PROFILE`
     to the values used in step 2.

See https://github.com/electron/electron-notarize for additional
guidance on Apple notarization.

## Certificate storage and rotation

* Production `.p12` and `.pfx` files MUST live outside the working tree
  and MUST NOT be committed. The repository `.gitignore` excludes
  `*.p12`, `*.pfx`, `*.cer`, and `.notarytool-creds.json`.

* Rotate signing certificates at least every 24 months or before any
  Apple/Microsoft expiration deadline. After rotation, update the
  corresponding `WARREN_CSC_*` GitHub Secret and re-run the latest
  release pipeline to confirm new signatures verify.

* If a signing certificate is suspected of being compromised, revoke it
  immediately via the respective vendor portal (Apple Developer or your
  Windows CA), generate replacements, and re-issue affected releases
  with the new certificates.
