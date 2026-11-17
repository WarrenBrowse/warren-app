# Android changelog
All changes to the software that can be noticed from the users' perspective should have an entry in
this file. Except very minor things that will not affect functionality, such as log message changes
and minor GUI adjustments.

### Format

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/).

Entries should have the imperative form, just like commit messages. Start each entry with words like
add, fix, increase, force etc.. Not added, fixed, increased, forced etc.

Line wrap the file at 100 chars.                                              That is over here -> |

### Categories each change fall into

* **Added**: for new features.
* **Changed**: for changes in existing functionality.
* **Deprecated**: for soon-to-be removed features.
* **Removed**: for now removed features.
* **Fixed**: for any bug fixes.
* **Security**: in case of vulnerabilities.

## [1.1.31] - 2026-09-21
### Added
- Add a "Get subscription" button in wallet settings that opens the checkout page.
- Add custom location lists to the exit picker (create a list, add/remove exits, delete).
- Pin each exit's public key on first use and refuse to connect if it changes; add a
  "Reset pinned exit keys" action in tunnel settings to accept a legitimate key rotation.
- Add Romanian as a new language in the app.
- Attach the app's logs to a forum bug report from the forum's "attach your logs" page, the link
  the desktop apps already handle: a consent prompt names the topic, shows the exact redacted
  report on request, and sends only on approval. A session code from that page typed under
  "Sign in to the forum with a code" opens the same prompt instead of reading as an expired
  sign-in.

### Changed
- Ship an optimised build even when no upload keystore is configured. Every beta APK so far was the
  debug variant, with the tunnel engine compiled unoptimised, which cost CPU and heat on slower
  phones; the release variant is now built and signed with the same key, so it installs over the
  previous beta without uninstalling.
- Optimize LWO performance. The throughput is around 3 times higher in our benchmarks.
- Replace the non-functional multi-hop toggle with a read-only indicator: multi-hop is
  available on the desktop app and the Android tunnel connects single-hop for now.
- Include native logs in problem reports so they are more useful for diagnostics.
- Translate the wallet, onboarding, and tunnel settings screens into every supported
  language; previously these Warren screens were only shown in English.

### Fixed
- Show the Warren wordmark in the app header instead of the Mullvad one.
- Fix the connect card going back to "Connected" after the system revoked the VPN mid-dial. The
  bounded revoke teardown hands its work to the adapter scope and returns, so a native status edge
  already in flight could publish "Connected" over the session it had just torn down.

## Mullvad upstream history

The release history inherited from Mullvad VPN is in
[CHANGELOG-upstream.md](CHANGELOG-upstream.md).
