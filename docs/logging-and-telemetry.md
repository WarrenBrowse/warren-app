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
  moment the user asks for it and carries, besides the platform facts, the
  live probes of the moment: the connect host through the VpnService-protected
  socket and through the plain client, the resolver, the API, and the clock
  offset against the broker (`probe-*`, `clock-offset*` keys). The values are
  classes and durations, never addresses.

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

On desktop the report is not transmitted by the app: the user shares the
redacted file themselves. On Android the user chooses between the in-app send
and the share sheet; nothing leaves the device without that tap.


## Telemetry (version check)

<!--
This section of the docs is an *explanation*, and below it comes a *reference*. Please try
to follow the documentation guidelines on this in https://github.com/mullvad/coding-guidelines/
-->

The app reports a very minimal amount of telemetry to the Warren API. And it does not in any way
tie it to identifiable information. See reference below for exact telemetry data.

The app calls an API designed to tell the app if there are any upgrades available and
if the currently running version is still supported. The main purpose
is to inform the users about new app versions, and alert the user if there are known
vulnerabilities or bugs in the version they are currently running. All of this is first and
foremost to improve the user experience and keep the user safe.

This API request does not contain the wallet address or any account or device identifier. It only
contains which version of the app is currently running and which operating system version it's
running on.
The API server aggregates this information and only keeps counters on number of used app versions
and operating systems. These statistics are recorded for the purpose of letting Warren
understand the impact of discovered bugs and issues, and to prioritize features.

### Reference

The following is the telemetry included in the version check API call. These are sent as
http headers and are only submitted once per 24 hours:

* `M-App-Version`: Contains the version of the Warren VPN app. For example `2026.1`.
* `M-Platform-Version`: Contains the operating system name and version. Only the most important
  parts of the OS version number is included. It will never include patch versions or build numbers.
  Examples: `Windows 11`, `Linux Ubuntu 24.04`, `macOS 26.0`, `Android 16`.
