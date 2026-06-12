# Warren VPN: Beta Release Procurement Guide

Consolidated procurement guide for signing certificates and external accounts required by `.github/workflows/release.yml` to produce signed installers on all 5 target platforms (Linux, macOS, Windows, iOS, Android).

Without these credentials the CI still builds unsigned artifacts (skip-if-no-secrets logic), useful for dry-runs, but the resulting binaries are not distributable to end users.

## Estimated annual cost

| Item | Cost | Recurrence |
|------|------|------------|
| Apple Developer Program (macOS notarization + iOS TestFlight) | 99 USD | annual |
| Windows OV code-signing certificate | ~280 EUR | annual (SSL.com, DigiCert, Sectigo) |
| Google Play Console developer account | 25 USD | one-time |
| Hetzner production servers (warren-exit-1 + warren-backend-api) | already provisioned | monthly |

Cost considered for the first year, baseline beta launch: **~400-500 EUR**.

## Linux: no signing required

Linux `.deb`/`.rpm` are unsigned by convention (users verify via SHA-256 checksums published in GitHub Release). The CI auto-builds Linux artifacts without any procurement.

## macOS: Apple Developer Program

1. Subscribe to https://developer.apple.com/programs/ as an organization (warrenBrowse SRL or holcommOn SAS). Cost: 99 USD/year.
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

1. Purchase an OV (Organization Validation) code-signing certificate from a recognized CA. Recommended vendors:
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
- [ ] All 18 secrets configured on `WarrenBrowse/warren-app`
- [ ] Dry-run via `gh workflow run release.yml` succeeds on all 5 platforms
- [ ] Internal-test groups created in TestFlight + Play Console with at least 1 tester email

Once all checkboxes are green, follow `docs/RUNBOOK-RELEASE.md` to tag and publish.
