# Split tunneling

Split tunneling decides, app by app, whether traffic goes through the VPN. In the app it is the
**App routing** page, with three modes. The design, the datapath and the tests behind them are in
[app routing](app-routing.md).

## The three modes

| mode (tab) | what the user gets |
|-|-|
| **Bypass VPN** (exclude) | The chosen apps communicate with the network as if Warren VPN were disconnected. Every other app uses the VPN. |
| **VPN only for** (include-only) | Only the chosen apps use the VPN, and they never reach the Internet outside it, also while the tunnel reconnects. The rest of the device uses the normal connection. System DNS keeps going through the tunnel resolver. |
| **Country per app** | Each chosen app leaves the Internet from a country of its own, through a second Warren session; every other tunneled app keeps the main connection. At most 2 countries at a time besides the main connection. |

Bypass VPN and VPN only for exclude each other: one split mode is on at a time, and switching keeps
both lists. Country per app composes with either:

* an app in Bypass VPN bypasses the VPN, and its country is ignored;
* in VPN only for, an app with a country is in the VPN and leaves from its country. On Linux,
  where an app joins the VPN when it is opened through `warren-include`, that holds for the app
  opened from the VPN only for tab;
* an app whose country's session is connecting or down gets no traffic at all until it is up. It
  never falls back to the main connection or to the normal one.

While VPN only for is on, the main screen says "Only selected apps are protected" under the
connection state, with the number of apps in the VPN (on Linux, without a number, since the apps
are chosen when they are opened).

### Where each mode is available

| mode | Windows | macOS | Linux | Android | iOS |
|-|-|-|-|-|-|
| Bypass VPN | yes | macOS 13 or later, signed build, Full Disk Access | yes, apps opened through `warren-exclude` | yes | no |
| VPN only for | yes | macOS 13 or later, signed build, Full Disk Access | cgroup v2 with nftables socket matching; apps opened through `warren-include` | being implemented | no |
| Country per app | yes* | yes, no Full Disk Access needed | yes*, for apps whose program can be named (below) | not yet | no |

*: implemented; the datapath has been run against real exits on macOS only so far.

On macOS the daemon answers `split_tunnel_is_supported` false for an unsigned build or a macOS older
than 13, and `need_full_disk_permissions` for the grant; the App routing page says which one is
missing and links the Full Disk Access pane when that is it.

On Linux a per-app country names the program the kernel runs. A desktop entry is followed through
`PATH`, symlinks and a shell wrapper whose last line execs a fixed program with `"$@"`. A Flatpak or
Snap app, or one started by a script whose program cannot be read, cannot take a country, and its
row says so; picking the real program with **Find another app** works for the script case.

## Vocabulary

* **Split tunneling** - The name of the feature.
* **Excluded app** - An app that only communicates outside of the VPN tunnel (Bypass VPN).
* **Included app** - An app that communicates inside the VPN tunnel. Outside VPN only for, this is
  every app that is not excluded; in VPN only for, only the chosen apps and those with a country.
* **To exclude** - The act of enabling split tunneling for a specific app, excluding its traffic
  from the VPN tunnel.
* **To include** - Putting an app's traffic in the VPN tunnel: removing it from Bypass VPN, or
  adding it to VPN only for.

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
