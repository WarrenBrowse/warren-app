# App routing: exclude, per-app country, VPN-only-for

Status: design, being implemented on branch `feat/per-app-exit` (2026-09-25).
This document is the contract the implementation lots follow. When the code and
this file disagree, fix one of them in the same commit.

The split tunneling page becomes **App routing**, with three tabs:

| tab | mode | what the user gets |
|---|---|---|
| **Bypass VPN** | exclude (existing) | selected apps talk to the network as if Warren were off |
| **Country per app** | per-app exit (new) | each selected app leaves the Internet from its own country, every other app keeps the main connection |
| **VPN only for** | include-only (new) | the system is NOT tunneled; only the selected apps are |

"Bypass VPN" and "VPN only for" are mutually exclusive (one split mode at a
time). "Country per app" composes with both.

## 1. Settings model

One settings struct, persisted on every platform that supports any mode
(Windows, macOS, Linux, Android). Today Linux persists nothing, because
exclusion there is launch-based; the new struct is persisted on Linux too.

```
AppRoutingSettings {
    split_mode: Off | Exclude | IncludeOnly,
    excluded_apps: set<AppId>,          // tab 1, kept when the mode changes
    included_apps: set<AppId>,          // tab 3, kept when the mode changes
    app_exits_enabled: bool,            // tab 2 master switch
    app_exits: map<AppId, ExitChoice>,  // tab 2
}
ExitChoice { country: CountryCode, city: Option<CityCode> }
```

- The existing `SplitTunnelSettings { enable_exclusions, apps }` migrates into
  `split_mode = Exclude if enable_exclusions else Off`, `excluded_apps = apps`
  through a settings version bump with a migration test.
- `AppId` is the executable path on Windows and Linux, the `.app` bundle path on
  macOS (a process matches when its executable lives anywhere inside the
  bundle, so Chromium helpers and Electron renderers belong to their app), and
  the package name on Android.
- Precedence, enforced in the daemon and never only in the GUI:
  1. an app in `excluded_apps` while `split_mode == Exclude` is outside the
     tunnel, and its `app_exits` entry is ignored;
  2. while `split_mode == IncludeOnly` and `app_exits_enabled`, an app with an
     `app_exits` entry is treated as included (choosing a country for an app
     in that mode is enough to put it in the VPN; with the countries switched
     off, the entry puts nothing in the VPN);
  3. otherwise an app with an `app_exits` entry leaves through its country, and
     everything else through the main connection.
- The split-tunneling RPCs keep their access class (owner and administrators
  only, `docs/security.md`), including the new ones.

## 2. Per-app country (in-engine, no driver, no entitlement)

Every packet of every tunneled app already reaches the daemon through the Warren
TUN. The daemon classifies each new flow by the process that owns its socket
and sends it through the session of that app's country.

### 2.1 Flow owner lookup

On the first packet of a new 5-tuple the router asks the OS which process owns
the local socket:

| OS | API | measured / expected cost |
|---|---|---|
| macOS | `sysctl net.inet.{tcp,udp}.pcblist_n` (`xinpcb_n`, `xsocket_n.so_last_pid`, `so_e_pid` when non-zero), then `proc_pidpath` | 0.8 ms TCP dump, 0.1 ms UDP, unprivileged (2026-09-25 probe); 0.42 ms for both tables of 366 sockets through `talpid-app-routing` in a release build |
| Windows | `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)`, `GetExtendedUdpTable(UDP_TABLE_OWNER_PID)`, then `QueryFullProcessImageNameW` | to measure |
| Linux | `NETLINK_SOCK_DIAG` exact lookup (inode; UDP takes the pair as a packet reaching the socket carries it, TCP the socket's own, measured on 6.8), inode to pid through an index of `/proc/*/fd` checked against that process's descriptors, then `/proc/<pid>/exe` | to measure |
| Android | `ConnectivityManager.getConnectionOwnerUid` (API 29+) | later lot |

Rules:
- The structs of `pcblist_n` are XNU-private: declare them with the packed
  layout (`#pragma pack(4)` equivalent, records of 104 bytes, offsets in the
  probe notes) and pin the layout with a test that opens a real socket and finds
  its own pid.
- Unconnected UDP sockets have no remote address: match them on the local port.
- A new flow is attributed only from a view of the OS taken after its packet
  arrived: the first new flow of a batch read from the TUN takes a fresh
  snapshot, and every other new flow of the batch reuses it. A hit in an older
  snapshot is never trusted, since the port may have changed hands since. On
  Linux the socket lookup is live, and the inode index is rebuilt at most once
  per batch. The hot path never calls the OS for a packet of a known flow.
- Classification is fully bypassed (zero per-packet cost) while no app has a
  country.
- A pid decision is cached by (pid, process start time, program), never by pid
  alone: a process keeps its pid and start time across `exec`, so the program
  is identified too (the pid version on macOS, the executable's inode on
  Linux; a Windows process never replaces its image).
- A flow whose owner cannot be resolved after the fresh snapshot goes through
  the main connection. That is the documented behavior, and it never leaves the
  tunnel. Two exceptions fail closed instead: a TCP segment that does not open
  a connection (anything but a bare SYN) belongs to a connection under way, so
  an ownerless one is closing and may have been routed, and it is dropped; and
  a UDP port shared by two processes' unconnected sockets cannot be
  attributed, so it counts as unresolved.
- ICMP echo is not attributed (no OS table lists ICMP sockets) and goes through
  the main connection.
- A later fragment follows the decision made for its first fragment; one whose
  first fragment was not seen is dropped. A bare SYN on a 5-tuple still known
  opens a new connection, possibly from another program, and is attributed
  again.

### 2.2 Sessions per country, shared

- Each distinct `ExitChoice` resolves through the relay selector with the main
  connection's own constraints (multihop, obfuscation, IP version, DAITA) and
  only the exit location replaced. Apps whose choices resolve to the same exit
  share one session.
- If a choice resolves to the main connection's exit, those apps simply use the
  main session.
- Each extra session is a full Warren session (the production datapath, same
  supervisor as the main tunnel). It presents v7 anonymous tokens only: **a
  route session never falls back to the wallet-signed v6 login.** When no token
  is available the route is shown as unavailable and its apps are blocked.
- At most **2** route sessions at a time (main + 2 = the 3 session tokens of an
  epoch, `TOKEN_QUOTA_PER_EPOCH`). The daemon refuses a third distinct
  `ExitChoice` (a country, or a city in one: `se` and `se`/`got` are two) with
  `FAILED_PRECONDITION` and the details `app_exit_limit`, and the GUI says why.

### 2.3 Address translation

A route session has its own inner address (assigned by its exit). Uplink: the
source address of a routed flow is rewritten from the main tunnel address to
the route session address, with incremental checksum fixes (IPv4 header,
TCP, UDP, ICMP). Downlink: the destination is rewritten back. Ports are never
changed: a 5-tuple is unique on the host, so the mapping is one to one per
session. A routed IPv6 flow whose route session has no IPv6 is dropped (the
app falls back to IPv4 through happy eyeballs).

### 2.4 Fail closed, per app

- A route session that is connecting, reconnecting or failed drops its apps'
  packets. They never go through the main session, and never outside.
- The main connection keeps its existing state machine and kill switch; route
  sessions live inside the connected state and are torn down with it.
- The firewall allows the route sessions' relay endpoints exactly like the main
  one (`peer_endpoints` is a list).

### 2.5 DNS

System DNS keeps going through the main tunnel resolver, as documented in
`split-tunneling.md`: resolution happens in a system service that cannot be
attributed to an app. The Internet traffic itself leaves from the chosen
country; sites that geolocate by client IP see that country.

### 2.6 What the user sees

For each routed app: its flag, the country, the live state (connecting,
connected, unavailable), and the public IP it appears from.

## 3. VPN only for (include-only)

The default route stays on the physical network. Only the included apps are
tunneled, and they are fail-closed: an included app never reaches the Internet
outside the tunnel, also while the tunnel reconnects. DNS for the whole system
keeps going through the tunnel resolver (reachable through the tunnel's own
subnet route), so an included app's names never go to the ISP.

| OS | mechanism |
|---|---|
| macOS | the existing split-tunnel classifier (eslogger process tracking, pf route-to into the ST utun) with the decision inverted: included processes to the VPN, everything else to the default interface. Needs Full Disk Access, like exclude. |
| Windows | the unmodified, Microsoft-signed Mullvad driver with the address pair swapped (tunnel address registered as "internet", physical as "tunnel") and the included apps registered as split; the tunnel gets its own `0.0.0.0/0` with a higher metric than the physical default instead of the two `/1` halves; winfw gets a policy that permits non-included apps. IPv6: when the tunnel has no IPv6, winfw blocks included apps' IPv6 (by app id) so no IPv6 flow escapes. Included apps cannot reach the LAN. |
| Linux | an `included` cgroup marked in nft; the tunnel table lookup (pref 51) becomes conditional on that mark (warrenguard-route-split gains the parameter); marked traffic that would leave through anything but the tunnel is dropped; launched through `warren-include`, the mirror of `warren-exclude`, same owner-only rule. |
| Android | `VpnService.Builder.addAllowedApplication`, with a guard refusing an empty or fully uninstalled list (Android would otherwise capture everything), and the same allow list on every blackhole plan. |
| iOS | not available. |

The GUI shows a persistent, calm warning while include-only is active: a
banner in the tab ("Only these apps are protected. Everything else on this
device uses your normal connection.") and a short label on the main screen
under the connection state. Turning the mode on asks for one confirmation.

## 4. Platform availability

- **macOS exclude and include-only** need Full Disk Access for
  `/usr/bin/eslogger` (Apple's binary carries the Endpoint Security
  entitlement; Warren does not need it). The compile-time `macos-split-tunnel`
  feature is replaced by a runtime probe that runs before any utun, pf or BPF
  setup and fails closed on an inconclusive answer. On an ad-hoc signed build
  the grant does not survive an update, so the GUI explains that and links the
  Full Disk Access pane. With a Developer ID signature (stable designated
  requirement) the grant persists.
- **Per-app country** needs neither Full Disk Access nor a driver on any
  desktop OS.
- **Windows** ships the driver as-is (x64 and ARM64 binaries are in
  `dist-assets/binaries`).

## 5. Security invariants (each one has a test)

1. No packet of an app routed to a country ever egresses through the main
   session or outside the tunnel while its route session is down.
2. In include-only mode, no packet of an included app leaves outside the
   tunnel, including while the tunnel reconnects and in the error state.
3. A route session never presents the wallet (no v6 fallback).
4. No pid, path, address or exit identity of a routed flow is logged in clear
   (shared no-log rule); counters only.
5. The mode switches and lists are owner-only RPCs.

## 6. Test plan

- Unit: packet parsing, flow table (TCP lifecycle, UDP idle), NAT checksum
  vectors, precedence rules, settings migration, owner lookup against real
  sockets on the host OS.
- Integration: the router between a fake TUN and two fake sessions (routing,
  fail closed per app, downlink translation).
- Real exits: a harness that runs the router with real Warren sessions to two
  beta exits and fetches the public IP through each (runs on macOS without
  touching the host network); the full daemon in a Linux VM (per-app country,
  include-only, exclude); the daemon in the Windows ARM64 VM (include-only
  with the swapped driver, per-app country).

## 7. Desktop GUI

The view keeps the split tunneling route (`RoutePath.splitTunneling`), renamed
**App routing**, with the three tabs above; a link opens a given tab through
the `app-routing-tab` location option. The rules it shows are computed in
`desktop/packages/mullvad-vpn/src/shared/app-routing.ts`, which mirrors the
daemon (precedence, the limit of `MAX_APP_EXITS` distinct exits) so the GUI
explains a refusal before making it, and still maps the daemon's
`app_exit_limit` answer when it comes.

- **Country per app** gives an app its exit in one click: a chip on each row
  opens a searchable country and city picker that only reports the choice and
  never moves the main connection. Choosing a country while the tab switch is
  off turns it on. A country the limit refuses stays listed, disabled, with
  why; at the limit, a country whose city is in use opens on its cities.
- Each app with a country shows its route state from `AppRouteStatus` (pushed
  as `DaemonEvent.app_routes`): connecting, connected with its public IP, or
  the reason it is unavailable. A route waiting for the main connection is not
  shown as a fault.
- **Bypass VPN** and **VPN only for** share one availability answer: on macOS
  the daemon's `SplitTunnelIsSupported` false means a build that cannot run the
  classifier (the GUI says it needs a signed build), then `NeedFullDiskPermissions`
  drives the existing Full Disk Access steps. Turning a mode off is never
  refused. Turning include-only on, or switching between the two modes, goes
  through one confirmation; both lists are kept.
- On Linux both modes are launch based (`warren-exclude`, `warren-include`) and
  per-app countries are path based: a desktop entry is keyed by the program it
  runs, resolved through `PATH` and symlinks. A shell script wrapper resolves to
  the script, which the router never sees running.
- The main screen shows "VPN only for N apps" under the connection state and
  "N apps in other countries" among the feature badges (red when a route cannot
  run), both opening their tab.
