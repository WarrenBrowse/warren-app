# How to generate keys and certificates for signing the macOS application and installer

## Generate private keys and CSRs

Generate *two* RSA keys and corresponding certificate signing requests (CSR).
One for "Developer ID Application" and one for "Developer ID Installer".
Replace C (country), CN (Common Name), O (Organization) and emailAddress as you see fit.

Yes, the keys must be 2048 bits. Apple won't allow 4096 for this.

```
openssl req -new -newkey rsa:2048 \
    -outform pem -keyform pem \
    -keyout private_key_application.pem \
    -out cert_signing_request_application \
    -subj "/C=SE/CN=Developer ID Application/O=Warren VPN/emailAddress=app@warrenbrowse.com"

openssl req -new -newkey rsa:2048 \
    -outform pem -keyform pem \
    -keyout private_key_installer.pem \
    -out cert_signing_request_installer \
    -subj "/C=SE/CN=Developer ID Installer/O=Warren VPN/emailAddress=app@warrenbrowse.com"
```

## Upload Certificate Signing Requests (CSR) to Apple

Do the following twice. Once for "Developer ID Application" and once for "Developer ID Installer",
and upload the respective CSR from the previous step.

1. Go to https://developer.apple.com/account/resources/certificates/list
1. Click the plus button (+) in the heading to create a new certificate
1. Select "Developer ID Application"/"Developer ID Installer" option from the given list, press "Continue"
1. Select `G2 Sub-CA (Xcode 11.4.1 or later)` under "Profile Type"
1. Select the previously created `cert_signing_request_application`/`cert_signing_request_installer` file for upload
1. Download the provided `developerID_application.cer`/`developerID_installer.cer`

## Download Apple IDG2CA intermediate certificate

The intermediate certificate that Apple used to sign the request above must be included in the pkcs12 file. So
download it. Apple might change which intermediate certificate they sign with. You can find it by looking
at the "Issuer" field from `openssl x509 -in developerID_application.pem -text`. (Follow the convert-to-pem
instructions below). All Apple's certificates are listed at https://www.apple.com/certificateauthority/.

```
curl -O https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer
```

## Convert certificates from CER to PEM format

Convert all the DER (`.cer`) files obtained from Apple into PEM format that OpenSSL can handle better:
```
openssl x509 -inform der -outform pem -in developerID_application.cer -out developerID_application.pem
openssl x509 -inform der -outform pem -in developerID_installer.cer -out developerID_installer.pem
openssl x509 -inform der -outform pem -in DeveloperIDG2CA.cer -out DeveloperIDG2CA.pem
rm developerID_application.cer developerID_installer.cer DeveloperIDG2CA.cer
```

## Create PKCS12 containers with the private keys and certificates

Two PKCS12 containers are needed. One for application signing and one for installer signing.
Each PKCS12 (`.p12`) file will contain:
  * The private key
  * The certificate issued by Apple
  * The intermediate signing certificate downloaded from Apple (same in both `.p12` files)

When issuing this command you will be asked to provide the passphrase of the private key you
are trying to bundle, as well as the passphrase you want the PKCS12 container to have. These
should be the same password.

For the application signing key/cert:
```
openssl pkcs12 -export \
  -inkey private_key_application.pem \
  -in developerID_application.pem \
  -certfile DeveloperIDG2CA.pem \
  -out macos_signing_application.p12 \
```

Repeat the above command but replace "application" with "installer".

Only the `.p12` files will be used. So the remaining files can be removed if everything works
as intended.

## Using the signing keys/certificates

* Upload the `.p12` files to the macOS build server (unless it was created there).
* In the shell where the app is built, point to the `.p12` files so electron-builder can find them:
  ```
  export CSC_LINK=/path/to/macos_signing_application.p12
  export CSC_INSTALLER_LINK=/path/to/macos_signing_installer.p12
  ```
* The passphrase for the `.p12` files must be assigned to `CSC_KEY_PASSWORD` and
  `CSC_INSTALLER_KEY_PASSWORD` respectively. But our `buildserver-build.sh` script will
  automatically ask for those, so no need to export them manually.



## The daemon's signature, and why split tunneling depends on it

macOS split tunneling (exclude and "VPN only for these apps") watches process
launches through `/usr/bin/eslogger`. Apple's binary carries the Endpoint
Security entitlement, so Warren needs none, but the daemon that spawns it must
hold Full Disk Access. That grant is recorded against the daemon's designated
requirement:

- with a Developer ID signature the requirement is the signing identifier and
  the team, so the grant survives every update;
- with an ad-hoc signature (every local build, every unsigned CI build) it is a
  hash of the binary, so the grant is lost at the next build and the split
  tunnel stops working without a word.

So the daemon refuses to run the split tunnel unless its own signature is
anchored at Apple (`anchor apple generic`, checked with `SecCodeCheckValidity`
on the running code), before any utun, pf or BPF setup. `split_tunnel_is_supported`
answers false on such a build, and turning a split mode on fails with "the
split tunnel needs a signed build". For local testing only, set
`WARREN_ALLOW_UNSIGNED_SPLIT_TUNNEL=1` in the daemon's environment (the launchd
plist): the grant then has to be given again after every rebuild.

The Full Disk Access probe spawns eslogger and starts short-lived processes
until it reports an event (granted) or a permission error (denied). Anything
else, silence included, counts as not granted: the old probe took silence for a
grant and let the split tunnel half-initialise.

What the build does for the signature:

- `mullvad-daemon/build.rs` embeds an Info.plist in the daemon binary
  (`__TEXT,__info_plist`). `codesign` takes the identifier from its
  `CFBundleIdentifier`, so whichever tool signs the daemon names it
  `com.warrenbrowse.vpn.daemon`, `com.warrenbrowse.vpn.beta.daemon` or
  `com.warrenbrowse.vpn.staging.daemon`, from `WARREN_PRODUCT_ENV`. Checked
  locally with an ad-hoc signature: `codesign -dv` reports
  `Identifier=com.warrenbrowse.vpn.daemon` for a prod build and
  `Identifier=com.warrenbrowse.vpn.beta.daemon` for a beta one.
- `tasks/distribution.cjs` signs the app through `signMacApp`, which gives
  `Contents/Resources/warren-daemon` the entitlements in
  `dist-assets/macos/warren-daemon.entitlements`, an empty dictionary: the
  hardened runtime with no exception. Every other binary keeps the inherited
  Electron entitlements (JIT and unsigned executable memory), which the
  Chromium helpers need and the daemon does not.

That the hardened runtime lets the daemon spawn eslogger and open utun,
`/dev/bpf` and `/dev/pf` with no entitlement is an assumption from Apple's list
of hardened runtime exceptions (none of them covers these), not something
observed: no Developer ID build has run it yet.

### What to verify on the first Developer ID build

Run each on the installed `.pkg`, on the channel you signed (beta first):

1. `codesign -dv --verbose=4 "/Applications/Warren VPN Beta.app/Contents/Resources/warren-daemon"`:
   `Identifier=com.warrenbrowse.vpn.beta.daemon`, the team identifier set,
   `flags=0x10000(runtime)`, and `codesign -d --entitlements - <same path>`
   prints an empty dictionary.
2. The daemon starts and connects (the hardened runtime blocks nothing it
   needs): `warren-beta status` reaches Connected.
3. In the GUI, the split tunneling page shows up (the daemon says the build is
   supported) and asks for Full Disk Access. Grant it in System Settings, note which entry the list shows
   (the app or `warren-daemon`), and check that the page stops asking without
   a daemon restart.
4. Exclusion: exclude a browser, connect, and compare its public address with
   another app's (`curl https://api.ipify.org` in Terminal stays on the exit).
5. Include-only: `warren-beta app-routing mode include-only`, include one app,
   and check that it alone leaves from the exit while Terminal's `curl` leaves
   from the ISP address; then block the exit (disconnect the network briefly)
   and check the included app gets no connectivity while the rest keeps it.
   Safari: include Safari and check its pages leave from the exit (its traffic
   is sent by WebKit's network process on its behalf, which the split tunnel
   attributes to Safari through the pktap effective pid).
6. DNS: `sudo tcpdump -ni en0 port 53` during steps 4 and 5 shows nothing.
7. Update to the next signed build over the top (same channel): the Full Disk
   Access grant is still there, the split tunnel still works, no new prompt.
8. An unsigned local build on the same Mac reports split tunneling
   unavailable and never touches the routes or pf.
