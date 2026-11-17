# Warren VPN: release runbook

How a release is cut, what the pipeline does with it, and how to withdraw one.
Which channel is live and what is dormant is deployment state: the
`warren-release-channels` skill of the workspace has it. How the in-app update
works (signing, hosting, the forced-update floor): `docs/AUTO-UPDATE.md`.

## Channels and tags

The shape of the tag picks the channel, and nothing else does
(`ci/resolve-release-channel.sh`):

| tag | channel | artifacts | update host |
| --- | --- | --- | --- |
| `beta-vX.Y.Z` | beta | `WarrenVPN-Beta-*` | `api.beta.warrenbrowse.com` |
| `vX.Y.Z` | prod | `WarrenVPN-*` | `api.warrenbrowse.com` |

A run builds and publishes one channel. The two version series are independent,
and the app and the headless client (`daemon-v*`, `daemon-beta-v*`) share one
version space. A prod tag is refused while any signing identity is missing
(`ci/require-signing-identities.sh`).

## Before the tag

1. `main` is green: Warren checks, Android checks, iOS checks.
2. Bump the version with `scripts/release/bump-version.sh <X.Y.Z>` and commit
   the files it lists.
3. Write the version's notes in `CHANGELOG.md`, `CHANGELOG.fr.md` and
   `CHANGELOG.ro.md`, under `## [X.Y.Z] - <date>`. The release refuses a version
   missing from any of the three (`ci/require-release-notes.sh`): the GitHub
   release body, the in-app what's-new and the signed manifests all show them.
4. Optional: `bash scripts/release/verify-beta.sh` runs locally what the release
   would catch.
5. Optional: to force clients below a version to update, set the channel's
   floor before tagging: `scripts/release/set-update-min-version.sh --beta
   <version>` (without `--beta` for prod).
6. Push to `main` and wait for its checks.

## Tag and push

```sh
git tag -l 'beta-v[0-9]*' | sort -V | tail -1   # never a bare `git tag -l | tail`
git ls-remote --tags origin 'beta-v*'
git tag -a beta-vX.Y.Z -m "Warren VPN Beta X.Y.Z"
git push origin beta-vX.Y.Z
```

`tag-guard` then refuses, before anything is built:

- a ref that is not a release tag;
- a tagged commit that `origin/main` does not carry (`ci/require-tag-on-main.sh`);
- a version that is not strictly above every tag of its series
  (`ci/check-release-version.sh`);
- missing release notes, and on prod a missing signing identity.

A failed run is re-run from the Actions tab, or with `gh workflow run
release.yml --ref beta-vX.Y.Z`: the workflow only starts from the tag itself.

## What the pipeline does

`.github/workflows/release.yml`, in order:

1. `tests`: the whole Warren test scope (`warren-tests.yml`, the same suite as
   the push gate). Nothing is built if it fails.
2. Builds, each uploading to a draft release and recording the hashes of what
   it uploads:
   - `build-macos`: universal `.pkg`, on the self-hosted macOS pool;
   - `build-linux`, `build-linux-arm64`: `.deb`, `.rpm`, `.pacman`, on
     `ubuntu-24.04` and `ubuntu-24.04-arm` inside the Debian 11 build image;
   - `build-linux-sysvinit`, `build-nixos`: repackage the amd64 `.deb` and
     install-test it;
   - `build-windows`: the NSIS installer, on `windows-2025`;
   - `build-android`: the APK (and the AAB on prod), on the self-hosted macOS
     pool;
   - `build-ios`: dormant until the repository variable `WARREN_ENABLE_IOS` is
     `true`.
   Signing is per platform and only happens when its secrets exist
   (`docs/RELEASE-PROCUREMENT.md`).
3. `release-headless` (`release-daemon.yml`): the warren-cli release of the same
   version.
4. `publish-release`, once every build and the headless release succeeded:
   refuses a draft that does not hold exactly the bytes the builds recorded
   (`ci/release-asset-sums.sh`), writes `SHA256SUMS` from those hashes, sets the
   notes and publishes. A beta is a pre-release and never becomes `latest`.
5. `publish-update-metadata` (`update-metadata.yml`): generates the update
   manifests, signs them in a job that holds only the update key, uploads
   installers and manifests to the update host through its receiver, attaches
   the torrents and `SHA256SUMS.sshsig` to the release, and asks the tracker
   whether the release is seeded (a warning, never a failure). The download
   page reads `downloads.json` from the update host: nothing to edit by hand.

A failed Release run triggers `refresh-update-metadata.yml`, which re-signs the
latest published release of each live channel, so a run that died after
publishing never leaves the served manifests behind. The same workflow runs on
the first of every month to renew the manifests' expiry.

## Withdrawing a release

Clients take the highest version listed in the signed manifest, and the
history is carried from one manifest to the next, so withdrawing a version is
re-signing the release below it without it:

```sh
gh workflow run refresh-update-metadata.yml --repo WarrenBrowse/warren-app \
  -f tag=beta-v<previous good version> -f drop_version=<bad version>
```

Several versions are comma-separated. Then fix on `main` and cut the next
version; the withdrawn version stays out of every later manifest. Leave the
GitHub release and its tag in place: the tag guard refuses to reuse a version
anyway.

## Versioning

- Desktop and Android share the version (`bump-version.sh`); iOS keeps its
  calendar version in `ios/Configurations/Version.xcconfig`.
- The beta Android `versionCode` is computed from the tag
  (`major * 10000 + minor * 100 + patch`), so it only grows along the beta
  line.
