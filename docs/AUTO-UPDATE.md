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
notification. The beta channel has its own floor, `WARREN_UPDATE_MIN_VERSION_BETA`,
read only by a `beta-v*` release: the two version series are independent, so
a prod floor never reaches a beta manifest and the reverse. Manage both with
the helper:

```sh
scripts/release/set-update-min-version.sh            # show current value
scripts/release/set-update-min-version.sh 1.2.0      # block clients below 1.2.0
scripts/release/set-update-min-version.sh --unset    # back to optional updates
scripts/release/set-update-min-version.sh --beta 1.0.0   # the beta channel's floor
```

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

- Source of truth: `warren-core/infra/docker/Caddyfile` (the `{$WARREN_API_DOMAIN}`
  vhost gains `handle_path /updates/*` → `file_server` rooted at `/srv/updates`)
  and `docker-compose.yml` (mounts host `/srv/warren-updates` →
  `/srv/updates:ro`).
- **Live prod reality:** the API host (`warren-backend-api`, Hetzner hel1,
  `204.168.244.76`) runs the stack with **docker compose**, but from a
  hand-managed directory `/srv/warren/` (NOT the repo, NOT a git clone; files
  are deployed by `update-prod.sh` with `.bak.<ts>` backups before each change).
  The live compose is `/srv/warren/compose.prod.yml` and the live config is
  `/srv/warren/Caddyfile`. SSH access is **`root@204.168.244.76`** (there is no
  `warren` user; the docs under `warren-core` that show `warren@<ip>` are
  outdated). So applying this change in prod means editing those two files in
  `/srv/warren/` and recreating only the caddy container.
- Override at runtime with the `WARREN_UPDATE_URL` / `WARREN_METADATA_URL` env
  vars (staging mirrors, local testing).

### One-time host setup

Run on the API host (`ssh root@204.168.244.76`). A dedicated unprivileged
`warren-deploy` user owns the manifest dir so CI never logs in as root.

```sh
# 1. Dedicated deploy user + the served directory.
adduser --system --group --home /srv/warren-updates --shell /usr/sbin/nologin warren-deploy
mkdir -p /srv/warren-updates/desktop
chown -R warren-deploy:warren-deploy /srv/warren-updates

# 2. Authorize a dedicated CI deploy key (generate it offline, NOT a personal key):
#    ssh-keygen -t ed25519 -f warren-updates-deploy -C ci-updates-deploy -N ""
mkdir -p /srv/warren-updates/.ssh
echo 'ssh-ed25519 AAAA...ci-updates-deploy' > /srv/warren-updates/.ssh/authorized_keys
chown -R warren-deploy:warren-deploy /srv/warren-updates/.ssh
chmod 700 /srv/warren-updates/.ssh && chmod 600 /srv/warren-updates/.ssh/authorized_keys
#    -> the private key goes into the WARREN_UPDATES_SSH_KEY secret.

# 3. Add the Caddy route + the mount, then recreate only caddy (~seconds blip).
cd /srv/warren
cp Caddyfile Caddyfile.bak.$(date +%s)
cp compose.prod.yml compose.prod.yml.bak.$(date +%s)
#    - add `handle_path /updates/*` to the {$WARREN_API_DOMAIN} vhost
#      (wrap the existing `reverse_proxy warren-api:8080` in `handle { }`)
#    - add `- /srv/warren-updates:/srv/updates:ro` to the caddy service volumes
docker run --rm -v /srv/warren/Caddyfile:/etc/caddy/Caddyfile:ro caddy:2.8-alpine \
    caddy validate --config /etc/caddy/Caddyfile      # sanity-check before applying
docker compose -f compose.prod.yml up -d caddy

# 4. Verify.
curl -fsS https://api.warrenbrowse.com/healthz                       # API still up
echo '{"ok":true}' > /srv/warren-updates/desktop/test.json
curl -fsS https://api.warrenbrowse.com/updates/desktop/test.json     # -> {"ok":true}
rm /srv/warren-updates/desktop/test.json
```

Rollback if anything is off: restore the `.bak.<ts>` files and
`docker compose -f compose.prod.yml up -d caddy` again.

## CI: publishing signed manifests

The `publish-update-metadata` job in `.github/workflows/release.yml` runs after
`publish-release` (which assembles the GitHub Release from the build jobs, then
publishes it non-draft once they are all green). It:

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
| `WARREN_UPDATES_SSH_USER` | `warren-deploy` |
| `WARREN_UPDATES_SSH_HOST` | `204.168.244.76` (the API VPS IP; `api.warrenbrowse.com` resolves to it, the IP avoids any future DNS/proxy surprise) |
| `WARREN_UPDATES_SSH_PATH` | `/srv/warren-updates/desktop` |
| `WARREN_UPDATES_SSH_KEY` | the CI deploy private key (the `warren-deploy` key) |

GitHub repo **variable**
(https://github.com/WarrenBrowse/warren-app/settings/variables/actions):

| Variable | Value |
| --- | --- |
| `WARREN_UPDATE_MIN_VERSION` | e.g. `1.2.0`: prod clients below this are hard-blocked. Leave unset for no forced update. |
| `WARREN_UPDATE_MIN_VERSION_BETA` | The same floor for the beta channel, in the beta version series (`1.0.0` since 2026-09-03: the 0.0.x betas have no forum-login handler). |

### Ordering gotcha

The installer URLs inside a manifest point at the GitHub Release assets, which
only resolve once the release is **non-draft**: manifests going live against a
draft release make clients see "update available" and then 404 on the download.
This is why `publish-release` publishes the release and verifies `isDraft` is
false BEFORE `publish-update-metadata` runs. Keep that ordering: never publish
manifests for a release that is still a draft.

## Per-platform capability

| Platform | Detection | In-app install | Forced-update gate |
| --- | --- | --- | --- |
| macOS | yes | yes (`.pkg` via `open`) | yes (blocking screen) |
| Windows | yes | yes (`.exe /inapp`) | yes (blocking screen) |
| Linux | yes | no (download page) | yes (blocking screen, manual update) |
| Android | planned | planned (direct APK) / store-managed (Play) | planned |
| iOS | planned | not possible (App Store only) | planned (version-check + store redirect) |

## Bringing Android and iOS into the same flow

The **server side is reused as-is** and is **already wired**: the CI emits and
signs `android.json` and `ios.json` next to the desktop manifests (same job,
same ed25519 key, same Caddy `/updates/` host). They use the same installer-less
`Response` shape as the Linux manifest, i.e. the latest version + changelog +
`minimum_supported_version` (mobile never self-downloads an installer; the store
performs the update). What remains is **client** work, and mobile stores
constrain what is possible.

### What already exists in the Warren mobile apps

**Android** is ~90% wired (inherited from Mullvad, intact) but the data source is
stubbed:

- Model `VersionInfo(currentVersion, isSupported)`:
  `android/lib/model/.../VersionInfo.kt`.
- Notification + UI: `InAppNotification.UnsupportedVersion`, the banner
  (`lib/ui/component/.../NotificationData.kt`) with an "open store" action, and
  `VersionNotificationUseCase` (emits when `!isSupported`). Gated by
  `ENABLE_IN_APP_VERSION_NOTIFICATIONS` (currently `true`).
- Store routing already done: `ResolveAppListingUseCaseImpl` →
  `market://details?id=...` when installed from a store, else
  `https://warrenbrowse.com/download`.
- **Stub to replace:** `AppVersionInfoRepository` hardcodes `isSupported = true`
  (its own comment: "poll warren-api `/v1/version` is planned").

**iOS** only has a *soft* path:

- `WarrenREST/ApiHandlers/AppVersionService.swift` polls the **iTunes Lookup API**
  every 24h and shows a "Update available" banner
  (`NewAppVersionInAppNotificationProvider.swift`) that opens the App Store.
- No "unsupported / forced" concept exists.
- Caveat: the App Store id in `ApplicationCoordinator.swift`
  (`itms-apps://...id1488466513`) is **Mullvad's**, not Warren's; iOS is disabled
  in CI (no Apple Developer account, no Warren App Store listing yet).

### Android wiring (the testable one)

Point `AppVersionInfoRepository` at `https://api.warrenbrowse.com/updates/desktop/android.json`
(LE-pinned host), verify the ed25519 signature against the embedded update
pubkey (`0f684b…`), and compute `isSupported = current >= minimum_supported_version`.
That immediately lights up the existing banner + store deep-link. For the
**forced** case add a non-dismissible Compose gate (mirroring the desktop
`BlockingUpdateGate`) shown when `!isSupported`, with a single "Update" action
to the store. ed25519 verification in Kotlin via Tink/BouncyCastle, or a small
shared Rust verifier over JNI.

### iOS wiring (blocked on an Apple account)

Add a signed version-check against `ios.json` (verify, then
`isSupported = current >= minimum_supported_version`), a new "unsupported"
notification provider, and a non-dismissible SwiftUI gate for the forced case
whose only action opens the App Store. No in-app install (Apple policy). Blocked
until Warren has an Apple Developer account + its own App Store listing (replace
the Mullvad `id1488466513`).

### Next steps

- [x] CI emits + signs `android.json` / `ios.json` (server side).
- [x] Android client wired (commit `ee8c0e6633`): `mullvad-update` gained the
      public `is_current_version_supported` + `MetaRepositoryPlatform::Android`;
      `warren-jni` exports `fetchVersionInfo` (one fetch + ed25519-verify,
      then the min-version rule, fail-open, and the newest stable release,
      fail-closed); `AppVersionInfoRepository` calls it off the main thread; a forced-update Compose gate (`UnsupportedVersionScreen`) replaces
      the UI in `WarrenApp` when unsupported. **Not yet build-tested** (android
      cross-compile needs the NDK; Kotlin needs a gradle build).
- [ ] Build + device-test the Android wiring; confirm the gate + banner with a
      manifest whose `minimum_supported_version` is above the installed version.
- [ ] iOS (when it ships): signed version-check + "unsupported" provider +
      forced-update SwiftUI gate + App Store redirect with Warren's app id
      (currently hardcoded to Mullvad's `id1488466513`).
- [ ] Optional: a direct-APK installer entry in `android.json` for sideloaded
      Android builds (full in-app update via `PackageInstaller`).

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
  publish it. CI publishes it automatically on a green build, so a draft here
  means the release run failed or the manifests were published out of band.
- *Verification fails after a key change:* the shipped client trusts the old
  pubkey; sign with the matching key, or ship the new trust file first
  (rotation overlap).
