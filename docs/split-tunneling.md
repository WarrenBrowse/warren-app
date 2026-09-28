# Split tunneling

Split tunneling decides, app by app, whether traffic goes through the VPN. In the app it is the
**App routing** page: one list of rules, where each app takes one route, and one choice for every
app without a rule. The design, the datapath and the tests behind it are in
[app routing](app-routing.md).

## Routes and the default

| route of an app | what the user gets |
|-|-|
| **Through the VPN** | The app uses the main connection, like every app when the VPN is the default. |
| **Through the VPN, another country** | The app leaves the Internet from a country of its own, through a route session of its own; every other tunneled app keeps the main connection. As many countries run at once as the server admits, and a country past that waits for a free route. |
| **Outside the VPN** | The app communicates with the network as if Warren VPN were disconnected. |

"Other apps go" sets what the apps without a rule do:

* **Through the VPN** (the default): every app uses the VPN except the apps sent outside it
  (exclude, formerly Bypass VPN);
* **Outside the VPN** (include-only, formerly VPN only for): only the apps given the VPN or a
  country use it, and they never reach the Internet outside it, also while the tunnel reconnects.
  The rest of the device uses the normal connection. System DNS keeps going through the tunnel
  resolver.

Nothing has a switch: exclusion turns on with its first app and off with its last, countries turn
on with the first one, and the default is the one explicit choice. Switching the default keeps the
countries and drops the other rules. Each app has one route, so combinations the older tabs allowed
without showing them (an app both bypassing and with a country) no longer occur; the daemon still
applies its precedence to settings written before:

* an excluded app bypasses the VPN, and its country is ignored;
* with Outside the VPN as the default, an app with a country is in the VPN and leaves from its
  country. On Linux, where an app joins the VPN when it is opened through `warren-include`, that
  holds for the app opened with **Open through the VPN**;
* an app whose country's session is connecting or down gets no traffic at all until it is up. It
  never falls back to the main connection or to the normal one.

While Outside the VPN is the default, the main screen says "Only selected apps are protected" under
the connection state, with the number of apps in the VPN (on Linux, without a number, since the
apps are chosen when they are opened).

### Where each route is available

| route | Windows | macOS | Linux | Android | iOS |
|-|-|-|-|-|-|
| Outside the VPN, for an app | yes | macOS 13 or later, signed build, Full Disk Access | yes, apps opened with **Open outside the VPN** (`warren-exclude`) | yes | no |
| Outside the VPN, as the default | yes | macOS 13 or later, signed build, Full Disk Access | cgroup v2 with nftables socket matching; apps opened with **Open through the VPN** (`warren-include`) | yes | no |
| Through the VPN, another country | yes* | yes, no Full Disk Access needed | yes*, for apps whose program can be named (below) | yes, Android 10 or newer | no |

*: implemented; the datapath has been run against real exits on macOS, on Linux (a Debian 13 VM)
and on Windows (a Windows 11 ARM64 VM).

On macOS the daemon answers `split_tunnel_is_supported` false for an unsigned build or a macOS older
than 13, and `need_full_disk_permissions` for the grant; the App routing page says which one is
missing and links the Full Disk Access pane when that is it.

On Linux a per-app country names the program the kernel runs. A desktop entry is followed through
`PATH`, symlinks and a shell wrapper whose last line execs a fixed program with `"$@"`. A Flatpak or
Snap app, or one started by a script whose program cannot be read, cannot take a country, and its
route screen says so; picking the real program with **Find another app** works for the script case.

## Vocabulary

* **Split tunneling** - The name of the feature.
* **Excluded app** - An app that only communicates outside of the VPN tunnel (its route is
  Outside the VPN while the VPN is the default).
* **Included app** - An app that communicates inside the VPN tunnel. With the VPN as the default,
  this is every app that is not excluded; with Outside the VPN as the default, only the apps given
  the VPN or a country.
* **To exclude** - The act of enabling split tunneling for a specific app, excluding its traffic
  from the VPN tunnel.
* **To include** - Putting an app's traffic in the VPN tunnel: removing its Outside the VPN rule,
  or giving it the VPN or a country while Outside the VPN is the default.

## DNS

DNS is a bit problematic to exclude properly. Ideally DNS requests from excluded apps would
always go outside the tunnel, because that's what they would have done if Warren was disconnected
or not running. But this is very hard/impossible to achieve on some platforms.
One reason for this is that on some operating systems, programs call into a system service
for name resolution. This system service will then perform the actual DNS lookup.
Since all DNS requests then originate from the same process/system service, it becomes hard
to know which ones are for excluded apps and which ones are not. Because the DNS service is not
excluded, DNS lookups **will fail** in the connecting, disconnecting, and error
[states](architecture.md) whenever they must be sent through a tunnel.

Some definitions of terms used later to describe behavior:

* **In tunnel** - DNS requests are sent in the VPN tunnel. Firewall rules ensure they
    are not allowed outside the tunnel for non-excluded apps*.
* **Outside tunnel** - DNS requests are sent outside the VPN tunnel. Firewall rules ensure
    they cannot go inside the tunnel*.
* **LAN** - Same as **Outside tunnel** with the addition that the firewall rules ensure
    the destination can only be in private non-routable IP ranges*.

* **Default DNS** - Custom DNS is disabled. The app uses the VPN relay server (default gateway)
    as the DNS resolver.
* **Private custom DNS** - Custom DNS is enabled and the resolver IP is in a private IP range.
* **Public custom DNS** - Custom DNS is enabled and the resolver IP is not in a private IP range.
* **System DNS** - Means the DNS configured in the operating system (or given by DHCP).

*: On platforms where we have custom firewall integration. This is currently on desktop operating
  systems, and not mobile.

### Desktop platforms (Windows, Linux, and macOS)

| In-app DNS setting | Normal & Excluded app                          |
|-|------------------------------------------------|
| **Default DNS** | In tunnel (to relay)                           |
| **Private custom DNS** (e.g. 10.0.1.1) | LAN (to 10.0.1.1)<br/>**macOS**: Not supported |
| **Public custom DNS** (e.g. 8.8.8.8) | In tunnel (to 8.8.8.8)                         |

In other words: Normal and excluded processes behave the same. This is because DNS is typically
handled by a service, e.g. DNS cache on Windows or systemd-resolved's resolver on Linux, which is
not an excluded process.

For the sake of simplicity and consistency, requests to public custom DNS resolvers are also sent
inside the tunnel when using a plain old static `resolv.conf`, even though it is technically
possible to exclude public custom DNS in that case.

### Android

| In-app DNS setting | Normal app | Excluded app |
|-|-|-|
| **Default DNS** | In tunnel (to relay) | Outside tunnel (to system DNS) |
| **Private custom DNS** (e.g. 10.0.1.1) | LAN* (to 10.0.1.1) | Outside tunnel (to system DNS) |
| **Public custom DNS** (e.g. 8.8.8.8) | In tunnel (to 8.8.8.8) | Outside tunnel (to system DNS) |

*: The "Local network sharing" option must be enabled to actually allow access to these IPs.
Otherwise DNS won't work.

In other words: Excluded apps behave as if there was no VPN tunnel running at all.

With "VPN only for" (include-only, `app-routing.md` section 3.4), a chosen app behaves as a
normal app above, and every other app behaves as an excluded app.

## Other limitations

Several limitations exist that relate to interprocess communication. An app is excluded if its path
is excluded or if its parent process is excluded. This can be problematic at times. For example,
opening a browser often typically tells the existing browser instance to open a new window, which
means the "excluded" status is not inherited.

On Linux, especially, where split tunneling isn't path-based at all, this means that the new browser
window will be forked off from a process that isn't excluded.

This model also implies other potentially unexpected behavior. For example, clicking a link in an
excluded app may (if there's no existing browser instance) open a browser window that _is_
unexpectedly excluded, simply because the parent is excluded.

The limitations due to IPC are perhaps especially noticeable on macOS, since WebKit relies on other
processes to render web pages. This means that many browsers, including Safari, cannot be excluded
from the VPN.

## Who may exclude or include

Excluding an app takes its traffic out of the tunnel and past the kill switch the wallet's owner
chose for the whole machine, so only the owner and administrators may do it. The daemon's
split-tunneling RPCs follow the rule every other network setting does (see
[the security document](security.md#who-may-use-the-management-interface)). On Linux,
`warren-exclude` is setuid root and never talks to the daemon, so it applies the rule itself: it
runs the program outside the tunnel only for root or for the account recorded in the daemon's
`wallet-owner.json`, read from the compiled settings directory and never from a path the caller's
environment could choose. `warren-include`, which runs a program inside the tunnel while "VPN only
for these apps" is on, follows the same rule; see [app routing](app-routing.md#31-linux).
