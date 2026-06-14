# Warren VPN: Auto-update and forced-update system

How Warren detects new versions, lets users update in-app, and can hard-block
clients that are too old. This covers the desktop app today and the design for
bringing Android and iOS into the same flow.

For the per-release operational steps, see `RUNBOOK-RELEASE.md`. This document
is the architecture and the why.

## Design decision

Warren is a fork of Mullvad, which already ships a complete, signed update
pipeline (the `mullvad-update` crate: signed metadata, staged rollout, in-app
download + verify + launch). The fork had **disabled** it ("Warren uses GitHub
Releases", updater never spawned). We **re-enabled and re-pointed** that
infrastructure instead of bolting on `electron-updater`, because:

- It updates the privileged daemon, not just the Electron GUI bundle.
- It already verifies the installer's signature and checksum.
- It already has a forced-update primitive (the `supported` flag).
- It avoids a second, redundant update mechanism fighting the daemon-driven one.

`electron-updater` was rejected: it only updates the GUI, has no daemon
awareness, and no forced-update concept.

## How it works (desktop)

```
api.warrenbrowse.com/updates/desktop/{macos,windows,linux}.json   signed (ed25519)
        │   metadata_version (anti-rollback), metadata_expiry,
        │   minimum_supported_version, releases[]{ installers[arch]{url, sha256, size} }
        ▼  HTTPS, TLS pinned to ISRG Root X1 (Let's Encrypt)
warren-daemon  ── version updater (mullvad-daemon/src/version/check.rs) ──┐
        │   verify signature + anti-rollback, detect arch, build VersionCache
        ▼  gRPC AppVersionInfo { supported, suggested_upgrade }
GUI (Electron renderer)
        ├─ supported = true,  upgrade available → soft notification + /settings/app-upgrade
        └─ supported = false (running < minimum)  → BlockingUpdateGate replaces the whole UI
                 └─ "Update" runs the in-app download/verify/install; "Quit" exits.
                    The VPN tunnel is left untouched (the user stays protected).
```

### Components

| Concern | Location |
| --- | --- |
| Daemon version poller | `mullvad-daemon/src/version/check.rs` (`spawn_version_updater`, `check_once`, `current_version_is_supported`) |
| Daemon version router | `mullvad-daemon/src/version/router.rs` (`spawn_version_router`) |
| Signed metadata format | `mullvad-update/src/format/response.rs` (`Response`, incl. `minimum_supported_version`) |
| Fetch + verify | `mullvad-update/src/client/api.rs` (`HttpVersionInfoProvider`) |
| Trusted signing pubkey(s) | `mullvad-update/warren-trusted-metadata-signing-pubkeys` |
| Default endpoint | `mullvad-update/src/defaults.rs` (`WARREN_RELEASES_URL`) |
| In-app download/install | `mullvad-update` downloader + `desktop/.../views/app-upgrade/` |
| Soft notification | `desktop/.../shared/notifications/{update-available,unsupported-version}.ts` |
| Forced-update gate | `desktop/.../components/BlockingUpdateGate.tsx` + `views/blocked-update/BlockedUpdateView.tsx` |

### Detection

The daemon polls every 6 hours (5 min retry on error) and on demand when a
frontend asks. It fetches `{base}/{platform}.json`, verifies the ed25519
signature against the trusted pubkey(s), enforces the `metadata_version`
anti-rollback floor, picks the installer for the running CPU architecture, and
emits an `AppVersionInfo` to the GUI. Detection runs on macOS, Windows and
Linux; the in-app *install* step is gated to macOS/Windows (`in_app_upgrade`
cfg), while Linux is sent to the download page.

### Forced update (the `minimum_supported_version` lever)

The manifest carries an optional `minimum_supported_version`. The daemon
computes:

```
supported = current_version >= minimum_supported_version      (if the field is set)
          = current_version is listed in releases[]           (fallback, upstream behaviour)
          = true                                               (always, for -dev builds)
```

`supported` rides the existing `AppVersionInfo.supported` gRPC field (no proto
change). In the renderer, `BlockingUpdateGate` replaces the entire UI with
`BlockedUpdateView` when `connectedToDaemon && consistent && !supported`. The
gate is non-escapable; the only actions are "Update" (runs the existing in-app
upgrade flow, or the download page on Linux) and "Quit". The VPN is left
running.

To force an update, set the repo variable `WARREN_UPDATE_MIN_VERSION` (see
below) so the next published manifest declares the new floor. It is graduated:
a normal release with no floor bump only shows the soft "update available"
notification.

## Signing key

The update key can ship a privileged installer to every user, so it is the
**highest-privilege key in the system**. It is a **dedicated, offline-generated
ed25519 key** used for nothing else. Do NOT reuse the relay/admin signing key
(it is online and its lineage was burned once:
`admin/admin-signing.key.BURNED-committed-do-not-use`).

- Generate offline: `cargo run -p mullvad-release -- generate-key` (prints
  "Secret key:" and "Public key:").
- The **public** key (hex) goes in `mullvad-update/warren-trusted-metadata-signing-pubkeys`
  (committed). Manifests signed by any other key fail verification, fail-closed.
- The **secret** key is stored offline and as the CI secret
  `WARREN_UPDATE_SIGNING_KEY`.
- **Rotation:** add the new pubkey on its own line (both trusted during the
  overlap), ship a release so clients pick up the new trust file, then drop the
  retired line.

## Hosting

Manifests are served from the **Hetzner/Caddy host** (the same VPS as
`api.warrenbrowse.com`) at `https://api.warrenbrowse.com/updates/desktop/`.

This host is used **on purpose**: Caddy issues a **Let's Encrypt** cert
(chains to **ISRG Root X1**), which is exactly what the client pins
(`mullvad-update/src/defaults.rs` → `PINNED_CERTIFICATE`, with system roots
disabled). The marketing site `warrenbrowse.com` is Cloudflare Pages and serves
a Cloudflare cert; serving manifests from there would fail the TLS pin. (The
manifests are ed25519-signed, so TLS is only defense-in-depth, but a failed pin
means no updates at all.)

- Caddy route: `warren-core/infra/docker/Caddyfile`, the `{$WARREN_API_DOMAIN}`
  vhost, `handle_path /updates/*` → `file_server` rooted at `/srv/updates`.
- Volume: `warren-core/infra/docker/docker-compose.yml` mounts host
  `/srv/warren-updates` → container `/srv/updates:ro`.
- Override at runtime with the `WARREN_UPDATE_URL` / `WARREN_METADATA_URL` env
  vars (staging mirrors, local testing).

### One-time VPS setup

```sh
# On the Hetzner API host:
ssh warren@api.warrenbrowse.com
sudo mkdir -p /srv/warren-updates/desktop
sudo chown -R warren:warren /srv/warren-updates

# A dedicated CI deploy key (do not reuse a personal key):
ssh-keygen -t ed25519 -f warren-updates-deploy -C "ci-updates-deploy" -N ""
ssh-copy-id -i warren-updates-deploy.pub warren@api.warrenbrowse.com
# -> private key content goes into the WARREN_UPDATES_SSH_KEY secret.

# Redeploy Caddy so the /updates route + the mount take effect:
cd <warren-core infra/docker on the host> && docker compose up -d caddy

# Verify:
echo '{"ok":true}' | sudo -u warren tee /srv/warren-updates/desktop/test.json
curl https://api.warrenbrowse.com/updates/desktop/test.json   # -> {"ok":true}
```

## CI: publishing signed manifests

The `publish-update-metadata` job in `.github/workflows/release.yml` runs after
`publish-release` (which builds installers and creates the **draft** GitHub
Release). It:

1. Downloads the installers from the release (`gh release download`).
2. Builds the signer: `cargo build -p mullvad-update --bin mullvad-version-metadata --features sign,client`.
3. Generates per-platform unsigned metadata with `ci/build-version-metadata.py`:
   computes size + SHA-256 of each installer, fetches the previously published
   manifest to bump `metadata_version` monotonically (anti-rollback) and to keep
   older releases listed, and injects `minimum_supported_version` from the repo
   variable. Mapping: macOS `.pkg` is universal (listed for both arches),
   Windows `.exe` is per-arch, Linux is installer-less.
4. Signs each `{platform}.json` with `WARREN_UPDATE_SIGNING_KEY`.
5. `scp`s the signed JSON to the host (and always uploads them as a run
   artifact for audit / manual fallback).

The whole job is a graceful no-op when `WARREN_UPDATE_SIGNING_KEY` is unset
(same opt-in philosophy as the build signing).

### Required configuration

GitHub repo **secrets**
(https://github.com/WarrenBrowse/warren-app/settings/secrets/actions):

| Secret | Value |
| --- | --- |
| `WARREN_UPDATE_SIGNING_KEY` | the dedicated ed25519 secret (hex) |
| `WARREN_UPDATES_SSH_USER` | `warren` |
| `WARREN_UPDATES_SSH_HOST` | `api.warrenbrowse.com` (or the VPS IP) |
| `WARREN_UPDATES_SSH_PATH` | `/srv/warren-updates/desktop` |
| `WARREN_UPDATES_SSH_KEY` | the CI deploy private key |

GitHub repo **variable**
(https://github.com/WarrenBrowse/warren-app/settings/variables/actions):

| Variable | Value |
| --- | --- |
| `WARREN_UPDATE_MIN_VERSION` | e.g. `1.2.0` — clients below this are hard-blocked. Leave unset for no forced update. |

### Ordering gotcha

The installer URLs inside a manifest point at the GitHub Release assets, which
only resolve once the **draft** release is **published**. Publish the GitHub
Release before (or together with) the manifests going live, otherwise clients
see "update available" but the download 404s.

## Per-platform capability

| Platform | Detection | In-app install | Forced-update gate |
| --- | --- | --- | --- |
| macOS | yes | yes (`.pkg` via `open`) | yes (blocking screen) |
| Windows | yes | yes (`.exe /inapp`) | yes (blocking screen) |
| Linux | yes | no (download page) | yes (blocking screen, manual update) |
| Android | planned | planned (direct APK) / store-managed (Play) | planned |
| iOS | planned | not possible (App Store only) | planned (version-check + store redirect) |

## Bringing Android and iOS into the same flow

The **server side is fully reusable**: the same CI job, the same ed25519 signing
key, the same Caddy `/updates/` host, and the same "signed JSON manifest"
principle. Only the **client** differs, and mobile stores constrain what is
possible.

### Shared mobile manifest

Emit `android.json` and `ios.json` next to the desktop manifests, signed with
the **same key**. The desktop `Response` schema is arch-keyed for desktop
installers and is a poor fit for phones, so mobile uses a **simpler shape**
(to be finalised with the mobile clients), e.g.:

```jsonc
{
  "metadata_version": 7,
  "metadata_expiry": "2026-12-14T00:00:00Z",
  "minimum_supported_version": "1.2.0",
  "latest": {
    "version": "1.3.0",
    "url": "https://github.com/.../WarrenVPN-1.3.0.apk",   // android direct-APK only
    "sha256": "…",                                          // android direct-APK only
    "store_url": "https://apps.apple.com/app/idXXXXXXX"     // ios / play deep link
  }
}
```

`ci/build-version-metadata.py` gains `android` / `ios` outputs; the signing and
upload steps already loop over platforms.

### Android

Two distribution channels, two behaviours:

- **Play Store (AAB):** Google performs the actual update. The manifest is used
  only as a **min-version gate** — block clients below
  `minimum_supported_version` with a blocking Compose screen that deep-links to
  the Play listing.
- **Direct APK (sideload / direct download):** full in-app update, mirroring
  desktop. Fetch `android.json`, verify the ed25519 signature, compare versions,
  download the APK from the GitHub Release, verify its SHA-256, and launch the
  package installer (`ACTION_INSTALL_PACKAGE` / `PackageInstaller`, needs
  `REQUEST_INSTALL_PACKAGES`). Forced update = the same blocking Compose screen.

Client work is Kotlin in the Android app. ed25519 verification can use Tink /
BouncyCastle, or reuse a small shared Rust verifier over JNI; either way the
**same public key** is embedded. Note Android is currently a parity work in
progress.

### iOS

iOS apps can only be updated through the **App Store** (or TestFlight); Apple
does not permit self-hosted installers. So the manifest is used for
**version-check + forced-update gating only**:

- Fetch `ios.json`, verify the signature.
- If `current < minimum_supported_version`, show a non-dismissible SwiftUI
  screen whose only action opens the App Store (`itms-apps://` / the
  `store_url`).
- No in-app download or install.

iOS is currently **disabled** in CI (no Apple Developer account yet; see
`release.yml`), so this is forward-looking.

### Next steps (not yet implemented)

- [ ] Finalise the mobile manifest schema with the Android/iOS clients.
- [ ] Extend `ci/build-version-metadata.py` to emit signed `android.json` /
      `ios.json`.
- [ ] Android: in-app updater + forced-update Compose screen (direct-APK) and/or
      Play min-version gate.
- [ ] iOS: signed version-check + forced-update SwiftUI screen + App Store
      redirect (once iOS ships).
- [ ] Embed the update pubkey in the Android and iOS apps.

## Operational notes

- **Anti-rollback:** `metadata_version` must increase on every publish. The
  generator derives it from the previously published manifest; never hand-edit a
  manifest to a lower counter.
- **Expiry:** manifests carry `metadata_expiry` (default 6 months). A long-lived
  client that cannot reach the host past expiry stops trusting stale metadata;
  publishing any release refreshes it.
- **Rollout:** the format supports staged rollout per release; the client's
  position is its persisted `rollout_threshold_seed`. With no seed yet, only
  fully rolled-out releases are surfaced.
- **Fail-closed:** a bad signature, an expired manifest, or a rolled-back
  counter is rejected and logged; the app keeps running on the current version.
- **Pin fragility:** the client pins ISRG Root X1. Keep the update host on
  Let's Encrypt. If the CA chain ever changes, update the pinned cert in
  `mullvad-api/le_root_cert.pem` and ship a release before the host switches.

## Troubleshooting

- *No update ever detected:* check the daemon log for "Failed to check for app
  updates"; confirm `https://api.warrenbrowse.com/updates/desktop/<platform>.json`
  is reachable and serves a valid signature, and that the running version's CPU
  arch has an installer entry.
- *TLS / pin failure:* confirm the host serves a Let's Encrypt (ISRG Root X1)
  cert: `openssl s_client -connect api.warrenbrowse.com:443 -showcerts`.
- *Update available but download 404s:* the GitHub Release is still a draft;
  publish it.
- *Verification fails after a key change:* the shipped client trusts the old
  pubkey; sign with the matching key, or ship the new trust file first
  (rotation overlap).
