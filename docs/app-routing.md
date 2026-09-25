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
keeps going through the tunnel resolver (routed into the tunnel by each
platform, see below), so an included app's names never go to the ISP.

| OS | mechanism |
|---|---|
| macOS | the existing split-tunnel classifier (eslogger process tracking, pf route-to into the ST utun) with the decision inverted: included processes to the VPN, everything else to the default interface. A packet is attributed to the pktap effective pid when there is one, so WebKit's network process working for Safari counts as Safari. A process the monitor does not know yet is dropped, and an included packet without the tunnel address is dropped. System DNS to the tunnel resolver never reaches the classifier (a pf rule passes it on the tunnel first). The error and blocked states block everything, the rest of the system included. Needs Full Disk Access and a signed build, like exclude. |
| Windows | the unmodified, Microsoft-signed Mullvad driver with the address pair swapped (tunnel address registered as "internet", physical as "tunnel") and the included apps registered as split; the tunnel gets its own `0.0.0.0/0` with a higher metric than the physical default instead of the two `/1` halves, and host routes to its resolvers; winfw's connected policy permits IPv4 outside the tunnel for non-included apps. IPv6 outside the tunnel stays blocked for every app. Included apps cannot reach the LAN. Details and the beta blocker below. |
| Linux | an `included` cgroup (`warren-inclusions`, cgroup2) marked in nft; the tunnel table lookup (pref 51) becomes conditional on that mark (`TunLookupScope::Marked`) and the exclusion bypass (pref 49) is not installed; marked traffic that would leave through anything but the tunnel is dropped; launched through `warren-include`, the mirror of `warren-exclude`, same owner-only rule. Details below. |
| Android | `VpnService.Builder.addAllowedApplication`, with a guard refusing an empty or fully uninstalled list (Android would otherwise capture everything), and the same allow list on every blackhole plan. |
| iOS | not available. |

The GUI shows a persistent, calm warning while include-only is active: a
banner in the tab ("Only these apps are protected. The rest of your device
uses your normal connection.") and a short label on the main screen under the
connection state. Turning the mode on asks for one confirmation.

### 3.1 Linux

The daemon creates the cgroup at start, after checking that nftables can match
cgroup2 sockets; `warren-include` only joins it (it never creates one, so an app
can never land in a cgroup the firewall does not know) and refuses to run the
program when it is missing. When include-only cannot select anything (no
cgroup2 socket matching), a persisted include-only falls back to the full
tunnel, never to an untunneled host. `warren split-tunnel add <pid>` puts a
running process in it while include-only is on, but only one that holds no
Internet socket: a socket keeps the cgroup it was created in, so an open
connection would carry on outside the tunnel. A process that has one is put
back and refused, with the advice to start it through `warren-include`. Exclusion and
include-only never shape the same state: switching modes reapplies the
firewall and reconnects the tunnel so the routes follow; cgroup membership is
kept, so switching back restores it, and a process left in the cgroup of the
mode not in force is simply routed like any other.

The nftables policy (`talpid-core/src/firewall/linux.rs`, `include_only_head`
and `include_only_tail`):

- the mangle chain marks the included cgroup's sockets (`socket cgroupv2`)
  and every later packet of their connections (conntrack mark), which the
  route chain reroutes into table 100;
- while connected, traffic to the tunnel resolvers, on any port, is marked the
  same way: the system resolver belongs to no app, and the tunnel address is a
  `/32` with no subnet route, so without the mark it would leave on the
  physical network;
- an included socket connects before its first packet is marked, with the
  physical source address, so included traffic leaving through the tunnel is
  masqueraded; replies are re-marked in prerouting for the reverse-path check
  (`src_valid_mark=1`);
- the output hook reports the device a packet was routed to before the mark,
  so it cannot tell where a rerouted packet leaves. Included traffic is
  therefore accepted there while connected, and a postrouting filter drops any
  included packet leaving by anything but loopback and the tunnel. Before the
  tunnel is connected (connecting, error, lockdown) included traffic is
  rejected at once in the output hook;
- included DNS reaches only the tunnel resolvers, as in the full tunnel;
- a packet arriving outside the tunnel for an included socket, a listener
  included, is dropped: an included app's open port never answers the
  physical network, which would tie its exit address to the real one;
- everything else is accepted both ways in every state, blocked ones
  included, except probes for the tunnel address.

Validated in a Debian 13 VM (kernel 6.12) against a beta exit: an included
`curl` egresses from the exit address while the rest of the VM keeps its own;
with the daemon cut from the network the included app times out, then is
refused at once in the connecting state, while the rest of the VM keeps
working; no packet to the included app's destination, no DNS and no tunnel
source address appeared on the physical interface; name lookups by included
and other apps alike produced no packet on it either. A listening included app
was unreachable from a peer on the physical side while a normal one answered.

Limits: included apps cannot reach the LAN (their traffic, LAN destinations
included, goes to the tunnel). A local DNS forwarder that is not included
(dnscrypt-proxy, a DNS-over-TLS stub) sends the names it resolves for included
apps to its own upstream outside the tunnel; only the system resolver pointed
at the tunnel resolver is covered. In the disconnected state without lockdown
nothing is tunneled, as with the full tunnel.

### 3.2 Windows

Nothing in the driver changes: `talpid-core/src/split_tunnel/driver_addresses.rs`
hands it the address pair swapped (the physical address as its "tunnel"
address, the VPN address as its "internet" one) and the included apps as the
split ones. Confirmed against the driver source
(`dist-assets/binaries/win-split-tunnel/src`): for split apps the bind redirect
(`callouts.cpp`, non-TCP) rewrites a bind to `ANY` or to the "tunnel" address
into the "internet" one, the connect redirect (TCP) rewrites the source of a
connection that uses the "tunnel" address or goes to a non-local destination,
and `BlockTunnel` (`filters.cpp`, weight `MAX`, in winfw's baseline sublayer)
hard-blocks their flows on the "tunnel" address; apps that are not split are
left to winfw.

- Routes (`talpid-warren-tunnel/src/default_route_split/windows.rs`): no `/1`
  halves. The tunnel gets an on-link `0.0.0.0/0` (and `::/0` when it has IPv6)
  at route metric 9000, so the physical default stays the best route and only
  sockets bound to the tunnel address take it, plus a host route to each tunnel
  resolver, since the system resolver binds nothing. A DNS change while
  connected reconnects, so the host routes follow.
- Firewall: the connected policy adds `PermitNonTunnelIpv4`
  (`windows/winfw/src/winfw/rules/baseline/permitnontunnelipv4.cpp`, weight
  `Medium`, above `BlockAll`, below the driver's filters), which lets IPv4 go
  outside the tunnel for every app the driver does not hold back. DNS keeps the
  full tunnel's rules (port 53 only to the tunnel resolvers over the tunnel).
  Connecting, error and lockdown keep blocking everything: before the driver
  has its addresses nothing tells an included app from another.
- IPv6 outside the tunnel stays blocked for every app. The driver guards one
  physical IPv6 address and an interface usually holds several (temporary
  addresses), and a permit by app id would miss an included app's children,
  which the driver splits by inheritance, so apps outside the VPN use IPv4
  only while include-only is on. This replaces the per-app IPv6 block the
  first design named: it is stricter and cannot miss a process.
- With the driver not loaded, include-only is refused, and a persisted one
  falls back to the full tunnel.

Residual limits: an included app that binds a UDP socket explicitly to the
address of an interface other than the physical default one is neither rebound
nor blocked by the driver; included apps cannot reach the LAN.

**Blocker for beta and staging.** The driver adds its filters to winfw's
baseline and DNS sublayers by their hardcoded Mullvad GUIDs
(`firewall/identifiers.h:132,140`), while winfw salts every GUID per product
environment (`mullvadguids.cpp`, `WarrenEnvGuid`; the salt is 0 on prod only).
On beta and staging the driver finds no such sublayer, cannot engage, and the
tunnel goes to the error state as soon as a split mode is on: exclusion is as
broken there as include-only. Fixing it means keeping those two sublayers
unsalted (shared by environments installed side by side, with each side
tolerating the other's sublayer on add and on delete), which needs a Windows
test run before it lands.

What the Windows VM run must check, on a build whose sublayers the driver can
reach (prod, or beta once the blocker is fixed):

1. `warren-beta app-routing mode include-only` then `include add` for a
   browser: connect, and compare the browser's public address (exit) with
   `curl.exe https://api.ipify.org` in a terminal (ISP address).
2. `Get-NetRoute -InterfaceAlias <tunnel>`: a `0.0.0.0/0` at metric 9000 and a
   `/32` per resolver, no `/1` halves; `route print` shows the physical default
   with the lower effective metric.
3. `Get-NetUDPEndpoint`/`Get-NetTCPConnection -OwningProcess <browser pid>`:
   local address is the tunnel address.
4. Cut the exit (block its address in Windows Defender Firewall, or pull the
   network): the included app gets nothing, the terminal keeps working once the
   network is back, and a packet capture on the physical adapter (`pktmon`)
   shows no packet of the included app and no DNS query.
5. IPv6 on a dual-stack network: the included app never reaches an IPv6
   destination outside the tunnel; other apps fall back to IPv4.
6. The browser's child processes (renderers, the network service) egress from
   the exit too.
7. Switch include-only off and on while connected: the routes, the WFP filters
   (`netsh wfp show filters`) and the driver state follow the mode, with no
   window where an included app leaves on the physical address.
8. Exclusion still works: `mode exclude`, `exclude add` for an app, it leaves
   from the ISP address while the rest uses the exit.


## 4. Platform availability

- **macOS exclude and include-only** need Full Disk Access for the daemon that
  spawns `/usr/bin/eslogger` (Apple's binary carries the Endpoint Security
  entitlement; Warren does not need it). A runtime check replaces the old
  compile-time `macos-split-tunnel` feature: it runs before any utun, pf or BPF
  setup, refuses a daemon whose own signature is not anchored at Apple (an
  ad-hoc build loses the grant at every update), and counts an inconclusive
  Full Disk Access probe as missing. `split_tunnel_is_supported` answers whether
  the build and the OS can run it; `need_full_disk_permissions` answers the
  grant. The GUI reads them as: supported and granted, split tunneling works;
  supported and not granted, it links the Full Disk Access pane; not supported
  on macOS 13 or later, it needs a signed build; older macOS, not available.
  The signature, the identifier and what to verify on the first Developer ID
  build: `docs/macos-signing.md`.
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
