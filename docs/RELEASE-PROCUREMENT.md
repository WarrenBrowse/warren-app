# Warren VPN: Beta Release Procurement Guide

Consolidated procurement guide for signing certificates and external accounts required by `.github/workflows/release.yml` to produce signed installers on all 5 target platforms (Linux, macOS, Windows, iOS, Android).

Without these credentials the CI still builds unsigned artifacts (skip-if-no-secrets logic), useful for dry-runs, but the resulting binaries are not distributable to end users.

## Estimated annual cost

| Item | Cost | Recurrence |
|------|------|------------|
| Apple Developer Program (macOS notarization + iOS TestFlight) | 99 USD | annual |
| Windows Authenticode certificate (IV individual or OV org, cloud-signing) | ~200-280 EUR | annual (Sectigo, SSL.com, Certum) |
| Google Play Console developer account | 25 USD | one-time |
| Hetzner production servers (warren-exit-1 + warren-backend-api) | already provisioned | monthly |

Cost considered for the first year, baseline beta launch: **~400-500 EUR**.

## No company yet: what is actually reachable today

State on 2026-08-07: **no signing secret exists on `WarrenBrowse/warren-app`**
(`gh secret list` shows only the engine, core-read and update-publishing ones), so
every desktop installer the beta ships is unsigned. `pkgutil --check-signature` on
the published `WarrenVPN-Beta-1.1.2-macos-universal.pkg` answers `no signature`.
That is not only cosmetic on Windows: Smart App Control refuses to load an
unsigned DLL, so `warren-daemon.exe --register-service` fails and the NSIS
installer stops on "Failed to install Warren VPN Beta service".

The two platforms are NOT symmetric on what an entity buys you.

- **macOS needs no company.** The Apple Developer Program enrolls a natural
  person (Individual, 99 USD/year, no D-U-N-S, approval in about 24-48 h) and an
  Individual account issues **Developer ID Application** and **Developer ID
  Installer** certificates plus notarization, which is the whole macOS story. The
  certificate carries the person's legal name instead of a company name; that is
  the only difference the user sees. Nothing else about `docs/macos-signing.md`
  changes.
- **Windows has no free lane.** Azure Trusted Signing (renamed Azure Artifact
  Signing) is closed to us twice over: Public Trust for *individual* developers
  requires the developer to be located in the US or Canada, and the
  *organization* path requires roughly three years of verifiable operating
  history, which a freshly registered warrenBrowse SRL will not have. Both are
  Microsoft's own documented rules, so the workflow's Trusted Signing path stays
  wired and unusable for now.

So Windows means a classic Authenticode certificate from a CA in the Microsoft
Trusted Root Program. Since 2023-06-01 the private key must live on certified
hardware, so CI signs through the CA's cloud service (SSL.com eSigner, Certum
SimplySign, DigiCert KeyLocker) rather than from a `.pfx`.

| option | who it is issued to | rough price | what it fixes |
|---|---|---|---|
| Sectigo / SSL.com **IV** (Individual Validation) | a natural person, ID check | ~200-280 EUR/year | Smart App Control blocks, "unknown publisher" |
| Certum **Open Source Code Signing** | a natural person, publisher shown as "Open Source Developer" | ~100-190 EUR/year | same, cheaper, needs the project to be verifiably open source |
| Sectigo / SSL.com **OV** | the company, once it exists | ~270-280 EUR/year | same, publisher shows the company |
| any **EV** | the company | ~600-1000 EUR/year | the above plus instant SmartScreen reputation |

Two facts decide this table:

- **Smart App Control accepts any RSA signature from a CA in the Trusted Root
  Program.** EV is not required, which is what makes the cheap individual
  certificate worth buying: it turns a hard install failure into, at worst, a
  dismissable SmartScreen screen. Note the constraint in Microsoft's own page:
  SAC does not evaluate ECC signatures, so the certificate must be RSA.
- **SmartScreen reputation is per certificate and accrues with downloads.** Only
  EV grants it on day one. An IV or OV certificate still shows "Windows protected
  your PC" at first, which the user can click past.

Certum's open-source product is the cheapest lane but it verifies the project is
published under an open licence. warren-app is GPL-3.0 and warrenguard AGPL-3.0,
yet both repositories are private until launch, so eligibility has to be
confirmed with Certum before counting on it.

Since 2026-03-01 no publicly trusted code-signing certificate may be issued for
more than 458 days, so a multi-year purchase means scheduled reissues, not one
long certificate.

Recommended order, given a beta that is live and blocking real testers: enroll
Apple as an Individual first (it is the only fully closing fix and the fastest),
then buy one Windows IV certificate with cloud signing. Waiting for the SRL buys
nothing on Windows, because Trusted Signing stays out of reach for three years
after incorporation either way.

## Linux: no signing required

Linux `.deb`/`.rpm` are unsigned by convention (users verify via SHA-256 checksums published in GitHub Release). The CI auto-builds Linux artifacts without any procurement.

## macOS: Apple Developer Program

1. Subscribe to https://developer.apple.com/programs/. Cost: 99 USD/year. Enroll as
   an **Individual** while no company exists (no D-U-N-S, approval in about 24-48 h,
   Developer ID and notarization included); switch to Organization later if the
   published publisher name has to read warrenBrowse SRL rather than a person.
2. Once enrolled, generate two signing keys and certificates following `docs/macos-signing.md`:
    - "Developer ID Application" `.p12`
    - "Developer ID Installer" `.p12`
3. Set up notarytool credentials:

   ```sh
   xcrun notarytool store-credentials warren-notary \
     --apple-id <email> \
     --team-id <team-id> \
     --password <app-specific-password>
   ```

4. Base64-encode the artifacts and add the following GitHub Secrets to `WarrenBrowse/warren-app`:

   ```sh
   base64 -i macos_signing_application.p12 | pbcopy  # paste into WARREN_CSC_LINK_MACOS
   ```

5. Required secrets:
    - `WARREN_CSC_LINK_MACOS`: base64 of `.p12` Developer ID Application
    - `WARREN_CSC_KEY_PASSWORD_MACOS`: passphrase of the `.p12`
    - `WARREN_NOTARIZE_KEYCHAIN`: keychain path (default `~/Library/Keychains/login.keychain-db`)
    - `WARREN_NOTARIZE_KEYCHAIN_PROFILE`: notarytool profile name (`warren-notary` above)

## Windows: Authenticode OV (or EV) certificate

> **Read "No company yet" above first.** Since 2023-06-01 CAs no longer deliver OV/EV
> certificates as a downloadable `.pfx` (keys must live on certified hardware), so the release
> workflow was migrated to **Azure Trusted Signing**
> (`WARREN_WIN_SIGN_BACKEND=trusted-signing`, see `signing-accounts-agent-runbook.md` § 3).
> That path is currently unreachable for us (US/Canada only for individuals, three years of
> operating history for organizations), so the realistic route is a CA certificate signed
> through that CA's cloud service. The `WARREN_CSC_LINK_WIN` path below assumes a downloadable
> `.pfx` and only applies to a pre-existing one.

1. Purchase a code-signing certificate from a recognized CA: **IV** (Individual Validation) while
   there is no company, **OV** (Organization Validation) once there is one. Recommended vendors:
    - SSL.com (~280 EUR/year, fast OV validation)
    - DigiCert (~430 USD/year)
    - Sectigo (~270 EUR/year)
2. Complete the OV validation process (typically 2-3 days). The CA verifies the organization (warrenBrowse SRL or holcommOn SAS) via D-U-N-S number or equivalent EU registry.
3. Export the certificate as `.pfx` from the CA's portal. Apply a strong passphrase.
4. Base64-encode and add secrets:

   ```sh
   base64 -i warren-codesign.pfx | pbcopy  # paste into WARREN_CSC_LINK_WIN
   ```

5. Required secrets:
    - `WARREN_CSC_LINK_WIN`: base64 of the `.pfx`
    - `WARREN_CSC_KEY_PASSWORD_WIN`: passphrase

> Note: EV certificates (~600-1000 EUR/year) provide instant SmartScreen reputation but require a hardware token (USB key) which is incompatible with cloud CI. Skip unless distributing outside GitHub Releases.

## iOS: TestFlight + App Store Connect

1. Apple Developer Program enrollment (same as macOS, can share the 99 USD/year fee).
2. In Xcode, create an "iOS Distribution" certificate and matching provisioning profile for `com.warrenbrowse.vpn.ios`:
    - Bundle ID: `com.warrenbrowse.vpn.ios`
    - App Group: `group.com.warrenbrowse.vpn`
    - Capabilities: NetworkExtension, Personal VPN
3. Export the certificate as `.p12` with passphrase.
4. Download the `.mobileprovision` from https://developer.apple.com/account/resources/profiles/list.
5. Create an App Store Connect API key:
    - https://appstoreconnect.apple.com/access/api → Keys tab → "+"
    - Role: App Manager
    - Download the `.p8` key
6. Base64-encode and add secrets:

   ```sh
   base64 -i WarrenVPN_ios_distribution.p12 | pbcopy
   base64 -i WarrenVPN.mobileprovision | pbcopy
   base64 -i AuthKey_<id>.p8 | pbcopy
   ```

7. Required secrets:
    - `IOS_DISTRIBUTION_CERT_BASE64`: base64 of `.p12`
    - `IOS_DISTRIBUTION_PASSWORD`: passphrase
    - `IOS_PROVISIONING_PROFILE_BASE64`: base64 of `.mobileprovision`
    - `APPSTORECONNECT_API_KEY_BASE64`: base64 of `.p8`
    - `APPSTORECONNECT_API_KEY_ID`: key id (e.g. `ABCD123XYZ`)
    - `APPSTORECONNECT_API_ISSUER_ID`: issuer id (UUID shown next to the key)
    - `APPLE_DEVELOPER_TEAM_ID`: 10-char team id

## Android: Upload keystore + Play Console

1. Subscribe to https://play.google.com/console (25 USD one-time, individual or organization).
2. Generate an upload keystore (RSA 4096, 25-year validity):

   ```sh
   keytool -genkey -v \
     -keystore warren-upload.keystore \
     -alias warren-upload \
     -keyalg RSA -keysize 4096 -validity 9125 \
     -storetype JKS
   ```

   Backup the `.keystore` file in **at least 2 encrypted offline locations**. Losing it means losing the ability to push updates to `com.warrenbrowse.vpn` on Play Store, forever.

3. In Play Console, create the app `com.warrenbrowse.vpn`:
    - Setup → App integrity → enable "Use Play App Signing" (Google holds the App Signing key, we keep the Upload key locally).
    - Setup → Testers → create email list (e.g. `warren-internal@warrenbrowse.com`) for internal-test track.
4. Create a service account in Google Cloud Console:
    - https://console.cloud.google.com/iam-admin/serviceaccounts → new service account
    - Role: "Service Account User"
    - Generate a JSON key, download it.
5. In Play Console, link this service account:
    - Settings → API access → link the service account
    - Grant role: "Release manager"
6. Base64-encode and add secrets:

   ```sh
   base64 -i warren-upload.keystore | pbcopy
   base64 -i google-play-service-account.json | pbcopy
   ```

7. Required secrets:
    - `ANDROID_KEYSTORE_BASE64`: base64 of `.keystore`
    - `ANDROID_KEYSTORE_PASSWORD`: keystore passphrase
    - `ANDROID_KEY_ALIAS`: `warren-upload`
    - `ANDROID_KEY_PASSWORD`: key passphrase (often same as keystore)
    - `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON_BASE64`: base64 of service account JSON

## Warren-core read-only token

Already required for path-dep checkout in CI:

- `WARREN_CORE_RO_TOKEN`: GitHub PAT with `repo:read` scope on `WarrenBrowse/warren-core`. Generate at https://github.com/settings/tokens.

## Auto-update: signing key + manifest host

Powers in-app updates and the forced-update screen. Full setup and rationale in
`docs/AUTO-UPDATE.md`; this is the procurement summary.

1. Generate a dedicated, offline ed25519 key (NOT the relay/admin key):
   ```sh
   cargo run -p mullvad-release -- generate-key   # prints Secret key + Public key
   ```
   Put the **Public key** in `mullvad-update/warren-trusted-metadata-signing-pubkeys` (committed).
2. One-time prep of the update host (Hetzner/Caddy, Let's Encrypt): create
   `/srv/warren-updates/desktop` owned by `warren-deploy`, add a CI deploy SSH key, and
   `docker compose up -d caddy`. See `docs/AUTO-UPDATE.md`.
3. Required secrets:
    - `WARREN_UPDATE_SIGNING_KEY`: the ed25519 **secret** (hex)
    - `WARREN_UPDATES_SSH_KEY`: CI deploy private key for `warren-deploy@<host>`
    - `WARREN_UPDATES_SSH_USER`: `warren-deploy`
    - `WARREN_UPDATES_SSH_HOST`: `api.warrenbrowse.com` (or the VPS IP)
    - `WARREN_UPDATES_SSH_PATH`: `/srv/warren-updates/desktop`
4. Optional repo **variable** (not a secret): `WARREN_UPDATE_MIN_VERSION` to
   hard-block clients below a version.

Without `WARREN_UPDATE_SIGNING_KEY` the manifests are not published (the job is
a no-op), so the app simply never detects updates.

## Verification after procurement

After all secrets are wired, run a dry-run via the manual trigger:

```sh
gh workflow run release.yml --ref main --field tag=v0.1.0-beta.1
```

Then check the run logs at https://github.com/WarrenBrowse/warren-app/actions. Every signing step that was previously `::warning::` should now run the signed-build branch.

If a sign step still skips, double-check the secret name spelling (GitHub is case-sensitive).

## Total checklist before tagging beta

- [ ] Apple Developer Program active (warrenBrowse SRL or holcommOn SAS)
- [ ] macOS `.p12` Developer ID Application + Installer + notarytool credentials
- [ ] Windows OV `.pfx` certificate
- [ ] iOS Distribution certificate + provisioning profile + App Store Connect API key
- [ ] Android upload keystore + Google Play service account
- [ ] Auto-update: dedicated ed25519 signing key (pubkey committed, secret in CI) + update-host SSH secrets (`docs/AUTO-UPDATE.md`)
- [ ] All required secrets configured on `WarrenBrowse/warren-app`
- [ ] Dry-run via `gh workflow run release.yml` succeeds on all 5 platforms
- [ ] Internal-test groups created in TestFlight + Play Console with at least 1 tester email

Once all checkboxes are green, follow `docs/RUNBOOK-RELEASE.md` to tag and publish.
