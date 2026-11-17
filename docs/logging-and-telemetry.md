# Logging and telemetry

The primary focus of the Warren VPN app is of course the users' privacy and anonymity.
For the purpose of debugging issues and in order to alert users who might be at risk, some
logging and minimal telemetry is being performed.


## Logging

For debugging and support purposes, the app's system service and GUI writes logs on the *local
device*. These logs are readable by all users on the system, but never automatically sent
anywhere by the app. Logs leave the device only if the user explicitly generates a problem
report and shares it themselves, see below.

The paths to the logs can be found in the main [README](../README.md).

The app must not log the wallet mnemonic, the Ed25519 private key, the SS58 wallet address, or
any tunnel key material.

On Windows, a crashdump named `DAEMON.DMP` is being generated when `warren-daemon.exe` crashes.
It is never sent anywhere, but stored locally in the same directory as the other logs
if the user/a developer would like to investigate the crash.

### Problem reports

Generating a problem report is opt-in and manual. The app never uploads any logs
or crash dumps anywhere by itself. A user generates a report explicitly:

* Desktop: Settings, Report a problem, or the `warren-problem-report` CLI tool,
  then attaches the redacted file to a thread on the community forum (the
  forum's paperclip flow, `docs/warren-forum-login.md`).
* Android: Settings, Report a problem. The form mirrors the forum's bug report;
  "View the logs" shows the exact redacted file, which the user can also hand
  to any app through the system share sheet, and Send posts the topic and the
  file to the support team through the wallet-signed `POST /v1/forum/report`
  of the connect broker (no browser involved). The report is collected at the
  moment the user asks for it. A report collected for Send also carries the
  live probes of that moment: the connect host through the VpnService-protected
  socket (and through the plain client only if the protected one failed), the
  resolver, the API, and the clock offset against the broker (`probe-*`,
  `clock-offset*` keys). The values are classes and durations, never
  addresses. A report collected for "View the logs" runs no probe (the keys
  read `not-run`): nothing reaches any host for a report the user only reads.

The logs collected for problem reports are redacted before the user shares them,
and the user always has the option to see exactly what information is included.
The following is redacted:

* Any 16 digit number, as a defensive measure. The Warren identity is a BIP39
  wallet (Ed25519 / SS58), not an account number, and wallet material is never
  logged to begin with.
* Home directory - In order to avoid including the current user's username in
  the logs.
* IPs and MAC addresses.
* V4 UUIDs. This includes account and device IDs, and network interface GUIDs on Windows.

On desktop and Android the report form sends the report once the user taps
Send, and the user can also save or share the redacted file instead; nothing
leaves the device without that tap. The tap on
Send is also what runs the probes above: they reach the connect host and the
API, from the device's own address on the protected leg, and are taken right
before the wallet-signed POST to the same connect host.


## Version check

The app learns of new versions, and of whether the running one is still
supported, by downloading the signed update manifest of its platform from the
update channel of its build. The request is a plain HTTPS GET of a static file:
it carries no wallet address, account or device identifier, no app version and
no system version (the client sets no such header and no user agent). The
manifest is verified against the update signing key before it is used. See
[`AUTO-UPDATE.md`](AUTO-UPDATE.md).

The device list of an account, which the user reads in the app, holds for each
device its platform, app version, registration day and last active day: the
server learns those when the device registers and calls the API.

