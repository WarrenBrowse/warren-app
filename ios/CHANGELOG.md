# iOS changelog
All changes to the software that can be noticed from the users' perspective should have an entry in
this file. Except very minor things that will not affect functionality, such as log message changes
and minor GUI adjustments.

The iOS app has not been released yet. The history of the Mullvad VPN app this one derives from is
in the upstream repository.

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

## UNRELEASED
### Added
- Add support for connecting over IPv6
- Attach the app's redacted logs to a forum bug report, from the forum's "attach your
  logs" link or a session code typed under Settings, behind a consent prompt with a
  log preview.

### Fixed
- Start the tunnel when iOS starts it without the app (on-demand rule, Settings, after a
  reboot) instead of showing it connected while dropping all traffic.
- Let the packet tunnel read the wallet key it signs with: it reads a seed-only item the app
  shares with it, readable after the first unlock, while the recovery phrase stays private to
  the app.
- End the previous tunnel session when the tunnel stops or changes location.
