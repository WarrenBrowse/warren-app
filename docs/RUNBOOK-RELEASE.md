# Warren VPN: Release Runbook

How to tag, push, and publish a Warren VPN release.

For how the in-app auto-update and forced-update system works (signing, hosting,
the `minimum_supported_version` lever, Android/iOS plan), see
`docs/AUTO-UPDATE.md`.

## Pre-tag checklist

Run the verification script:

```sh
bash scripts/release/verify-beta.sh
```

All sections must report PASS. If any FAIL is shown, fix the underlying issue and re-run.

For platform-specific skips (e.g. running on Linux but only Linux+Windows+Android matter for now):

```sh
bash scripts/release/verify-beta.sh --skip-ios --skip-android
```

Once green, check that all required secrets are configured on
https://github.com/WarrenBrowse/warren-app/settings/secrets/actions, following `docs/RELEASE-PROCUREMENT.md`. Re-run a dry-run via `gh workflow run` if in doubt:

```sh
gh workflow run release.yml --ref main --field tag=v0.1.0-beta.1
```

## Update CHANGELOG.md

Replace the `[Unreleased]` section's heading with the new version and add the date:

```diff
- ## [Unreleased]
+ ## [0.1.0-beta.1] - 2026-05-22
```

Then create a new empty `[Unreleased]` block on top, with the standard `### Added/Changed/Fixed/Security/Removed` headings, for future changes.

Commit:

```sh
git add CHANGELOG.md
git commit -m "chore: prepare v0.1.0-beta.1"
git push origin main
```

## Tag and push

```sh
git tag -a v0.1.0-beta.1 -m "Warren VPN beta 1"
git push origin v0.1.0-beta.1
```

This triggers `.github/workflows/release.yml`. The workflow:

1. Builds installers in parallel on `macos-14`, `ubuntu-22.04`, `windows-2022`, `macos-15` (iOS), `ubuntu-22.04` (Android).
2. Signs each artifact when the corresponding secret is present (skip-if-no-secrets logic).
3. Uploads the signed iOS `.ipa` to TestFlight via `xcrun altool`.
4. Uploads the signed Android `.aab` to Google Play internal-test track via `r0adkll/upload-google-play`.
5. Aggregates all artifacts and publishes a **draft** GitHub Release with SHA-256 checksums.
6. `publish-update-metadata`: generates the per-platform signed update manifests, signs them with `WARREN_UPDATE_SIGNING_KEY`, and `scp`s them to the update host. No-op when that secret is unset. See `docs/AUTO-UPDATE.md`.

Monitor the run live at https://github.com/WarrenBrowse/warren-app/actions.

Expected wall-clock duration: ~25-40 minutes.

## Post-build

1. Open the draft Release at https://github.com/WarrenBrowse/warren-app/releases.
2. Verify the artifact list contains:
    - macOS: `WarrenVPN-*.dmg`, `WarrenVPN-*.pkg`
    - Linux: `WarrenVPN-*.deb`, `WarrenVPN-*.rpm`, `warren-vpn-daemon_*.deb`, `warren-vpn-daemon_*.rpm`
    - Windows: `WarrenVPN-*.exe`, `WarrenVPN-*.msi`
    - iOS: `WarrenVPN.ipa` (artifact only; TestFlight upload happens automatically)
    - Android: `app-prod-release.aab`, `app-prod-release.apk`
    - `SHA256SUMS`
3. Edit the release body with the appropriate CHANGELOG section.
4. Mark as "Pre-release" (since this is a beta).
5. Click **Publish release** when ready.

## TestFlight + Play Store

After the CI run completes:

- iOS internal testers receive the TestFlight invite by email within ~10 minutes (assuming App Store Connect processing succeeded).
- Android internal testers receive the Play Store update within ~15 minutes via the configured email list.

If uploads fail (e.g. version code conflict, missing privacy nutrition labels), the error appears in the corresponding workflow log. Fix and re-tag with `v0.1.0-beta.2`.

## Update warrenbrowse-site (download page only)

This is the human-facing download page, served from Cloudflare Pages. It is
**separate** from the auto-update manifests (those are signed JSON served from
`api.warrenbrowse.com/updates/`, published automatically by CI; see
`docs/AUTO-UPDATE.md`). Do not confuse the two.

Once the release is public:

1. Update `warrenbrowse-site/src/pages/download.astro` and `warrenbrowse-site/src/pages/fr/download.astro` with the new release URLs and SHA-256 checksums.
2. Commit and push to deploy via Cloudflare Pages.

## Auto-update manifests

The `publish-update-metadata` job publishes the signed update manifests
automatically. Two things to remember at release time:

1. **Publish the draft GitHub Release** (above) so the installer asset URLs in
   the manifests resolve; until then in-app downloads 404.
2. To **force** an update for this release, set the repo variable
   `WARREN_UPDATE_MIN_VERSION` before tagging (clients below it are hard-blocked
   by the forced-update screen). Leave it unset for a normal, optional update.

First-time setup of the signing key, secrets, and the update host is documented
in `docs/AUTO-UPDATE.md`.

## Rollback procedure

If a critical bug is found after publishing:

1. **Do not delete the GitHub Release** (binaries cached by users will still work). Instead, mark it as "Pre-release" or "Draft" to hide it from the latest-release endpoint.
2. **Do not delete the tag** (rewrites git history visible to other devs and breaks the tag-build link).
3. Fix the bug on `main`. Verify with `scripts/release/verify-beta.sh`.
4. Bump the version (e.g. `v0.1.0-beta.2`) and re-tag.
5. The old beta.1 stays in release history; users see beta.2 as the new latest.

For TestFlight specifically, Apple lets you "Expire" a build to prevent further installs from existing testers. Same for Play Console internal-test.

## Versioning convention

- Beta releases: `v0.1.0-beta.<n>` (incremental)
- Release candidates: `v0.1.0-rc.<n>`
- Production: `v0.1.0`, `v0.1.1`, `v0.2.0`, etc.
- For mobile, `versionCode` (Android) and `CFBundleVersion` (iOS) MUST be strictly monotonic. The CI computes them from `git rev-list --count HEAD` to guarantee monotonicity across re-tags.

## When to ship

A new release is appropriate when **at least one** of the following applies:

- A user-visible feature lands (post `### Added` in CHANGELOG).
- A security vulnerability is fixed (post `### Security`, ship within 24h ideally).
- A critical bug is fixed (post `### Fixed`).
- Mullvad upstream sync brings important changes (post all relevant categories).

Avoid shipping for internal refactors that don't affect users. Bundle multiple changes into one release to limit user-facing version churn.
