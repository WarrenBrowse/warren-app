# App routing: exclude, per-app country, VPN-only-for

Status on `main` (2026-09-26), which carries the four lots integrated:
per-app country runs in the datapath on desktop (sections 2.2 to 2.6, real
exits measured from macOS and Windows); include-only runs on macOS, Linux and
Windows (section 3, the Windows run in section 3.2); the desktop GUI covers all
three tabs (section 7); section 3.3 is how the modes and the countries combine.
On Android, exclude and include-only run (section 3.4); per-app country is the
next Android step (section 3.5).
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
     off, the entry puts nothing in the VPN). On Linux the tunnel takes no
     list, since an app joins the included cgroup when it is opened through
     `warren-include`, so there this holds for the app opened that way
     (section 3.3);
  3. otherwise an app with an `app_exits` entry leaves through its country, and
     everything else through the main connection.
- The split-tunneling RPCs keep their access class (owner and administrators
  only, `docs/security.md`), including the new ones.
- On Android the settings live in `WarrenLocalSettingsRepository`
  (SharedPreferences `split_tunneling_mode`, `split_tunneling_excluded_apps`,
  `split_tunneling_included_apps`). The older `split_tunneling_enabled` switch
  is read once, when no mode is stored yet, and becomes `Exclude` or `Off`.

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
| Windows | `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)`, `GetExtendedUdpTable(UDP_TABLE_OWNER_PID)`, then `QueryFullProcessImageNameW` | 0.19 ms for the four tables of 113 sockets (x64 debug build under emulation on Windows 11 ARM64, 2026-09-26) |
| Linux | `NETLINK_SOCK_DIAG` exact lookup (inode; UDP takes the pair as a packet reaching the socket carries it, TCP the socket's own, measured on 6.8), inode to pid through an index of the `/proc/<pid>/fd` of the processes running a routed program, checked against that process's descriptors, then `/proc/<pid>/exe`. Those processes are followed through the proc connector (fork, exec and exit events) | per new flow, release build, Debian 13 VM with 433 processes and 1,500 to 3,000 TCP sockets (2026-09-26): 3 µs while no routed program runs; 0.5 to 0.7 ms (p99 1.1 ms) while a routed program holding 600 to 800 sockets runs, since its descriptors are read again for each new flow; 4.5 to 5.2 ms (p99 5.7 to 6.4 ms) when every process is searched, as before the search was narrowed and still for a TCP segment under way |
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
  per refresh. The hot path never calls the OS for a packet of a known flow.
- On Linux a new flow is searched for only among the processes running a
  routed program: reading every process's descriptors costs milliseconds per
  new flow on a busy host, under the lock the main pump takes for every
  packet. A live socket none of them holds belongs to an app without a
  country, and the flow goes through the main connection, as one whose owner
  is never found does. A TCP segment of a connection under way is still
  searched for among every process, so it goes through main only when its
  holder is found and runs no routed program; otherwise it is dropped, since
  the connection may have been routed. The router names the routed programs
  with each policy, and the resolver then reads every process's program and
  follows the proc connector's fork, exec and exit events, which the kernel
  queues before the process they name can open a socket. Every process's
  program is read again when events may be missing: a full queue, a gap in a
  CPU's event numbers (the kernel drops an event it cannot allocate without
  reporting it), a broken socket, no events at all (a kernel without
  `CONFIG_PROC_EVENTS`, a network namespace other than the host's, an
  unprivileged process on a kernel before 6.6, retried every 30 s), and in
  any case at least once a second, for what no event reports (a program
  swapped in without an exec, as CRIU does). A read of every program costs
  0.3 to 0.6 ms for 430 processes.
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
  The planner and the tunnel each cap again, since a settings file edited by
  hand may hold more: the apps of an exit past the cap are blocked.

Implementation (`mullvad-daemon/src/warren_app_routes.rs`,
`talpid-warren-tunnel/src/app_routes/`):

- The daemon's parameters generator resolves the exits in force every time
  one of its inputs changes (app routing settings, verified directory, main
  circuit, draining exits) and publishes an `AppRoutesPlan` on a watch
  channel. Every tunnel follows it live: a settings change starts or stops
  route sessions and never reconnects the main one. An unchanged plan is not
  republished, and the circuit a choice resolved to is kept while it stays
  valid, so a directory refresh does not move a route.
- A city choice matches the directory node whose city slug is the relay-list
  city code. A route has as many hops as the main connection, and a two-hop
  route keeps the main connection's entry constraint.
- A route session runs the engine's supervisor and pumps with
  `SessionAdmission::TokensOnly`, a random signing key (the wallet never
  enters it), one connection, and none of the hooks that feed process-wide
  state (dial-refusal cooldown, session placement, reconnect counter, entry
  RTT store, drain reactor). Its tokens come from the daemon's token source
  (see Tokens below). After an end a retry may fix (no token, refused,
  failed) it waits 60 s and dials again. A drain its exit announces is recorded like the main one's,
  and the new plan moves the route to another exit.
- The plan's policy is installed before the main pumps carry their first
  packet, and a new plan's before the firewall is asked for the new relays:
  a planned app's packets are dropped until its route is connected, never
  sent through the main session meanwhile. Route sessions are started once
  the main tunnel is up, and stopped (and awaited, so the TUN device is
  released) before the main pumps at teardown.
- An exit the main circuit already matches is carried by the main session
  only while the main session is on the exit the plan assumed (the tunnel
  follows the exit of the live session, across migrations); otherwise its
  apps are dropped until the next plan. While a custom exit is on, no exit is
  left to the main session.
- A route session shares one process-wide state with the main session on
  purpose: the engine's memory of whether this network lets QUIC through.

Tokens (`mullvad-daemon/src/warren_token_provider.rs`):

- The wallet's batch is blinded from its seed, so every client of the wallet
  holds the same three tokens an epoch, and an exit leases each serial to one
  live session in the whole fleet. A session is handed the whole current-epoch
  batch and never consumes it. The exit spends the first token of a setup
  that verifies: a route session (tokens-only admission) sends the lead token
  alone, the main session (default admission) sends the whole stack, and when
  the exit refuses the lead the engine redials leading with the next one. A
  reconnect costs no token.
- The tunnel opens a provider of its own for the main session and for each
  route session (`SessionTokenSource`). A provider holds the serial its
  session leads with until the session's supervisor is dropped: no other
  session of the daemon leads with it, and the session leads with it again
  when it redials, which the exit renews in place.
- The hold follows the lead, not the serial the exit admitted: the engine does
  not say which token of a walked stack it was admitted on, so after a refusal
  the hold sits on the refused serial. A serial another session holds
  therefore goes to the tail of a stack and never out of it, since it may be
  the one serial the exit would renew for this session.
- The main session keeps the engine's default admission: with no token this
  epoch it logs in with the wallet, and when the exit refuses every token of
  the batch the session ends on the exit's rejection (every serial of the
  wallet is held elsewhere, which is the wallet's device limit).
- The wallet's batches are minted in the background, when the first tunnel
  starts and every 10 minutes after; a failed round is retried after 15 s,
  then 30 s, doubling up to the 10 minutes. The first round fails at times
  (seen twice in the Linux VM right after the daemon started and connected,
  `all API hosts are unreachable`, cause not established), and with the
  fixed period route sessions then went 10 minutes without a token; with the
  retry they connected 60 s after the connect, at their own next attempt.

### 2.3 Address translation

A route session has its own inner address (assigned by its exit). Uplink: the
source address of a routed flow is rewritten from the main tunnel address to
the route session address, with incremental checksum fixes (IPv4 header,
TCP, UDP, ICMP). Downlink: the destination is rewritten back. Ports are never
changed: a 5-tuple is unique on the host, so the mapping is one to one per
session. A routed IPv6 flow whose route session has no IPv6 is dropped (the
app falls back to IPv4 through happy eyeballs).

Only IP headers are translated: a protocol that writes the local address into
its payload (WebRTC host candidates without mDNS, SIP, FTP `PORT`) carries the
main session's inner address through the route's exit, which ties the two
sessions together at that exit.

### 2.4 Fail closed, per app

- A route session that is connecting, reconnecting or failed drops its apps'
  packets. They never go through the main session, and never outside.
- The main connection keeps its existing state machine and kill switch; route
  sessions live inside the connected state and are torn down with it.
- The firewall allows the route sessions' relay endpoints exactly like the main
  one (`peer_endpoints` is a list). The tunnel's `PeerFence` names them next to
  the main session's relay, whatever the main session migrates to, through
  `TunnelEvent::PeerEndpoints`; a new route's relay is named, and the policy
  applied, before its session dials, and a removed route's relay is unnamed
  only once its session is gone. macOS pf, Linux nft and Windows winfw each
  install one allow rule per named endpoint (the daemon's own sockets only),
  so no firewall code changed. The route session's carrier escapes the tunnel
  the way the main one does (`SocketBypass`), and on a macOS network where the
  bind is cached as black-holing, through the relay's `/32` route.
- The router sits between the TUN device and every session as a
  `PacketDevice` wrapper: the main pumps read `RoutedTun`, which takes a
  routed app's packets out of the main path, and each route session's pumps
  read and write a `RouteTun`. While no app has an exit, the main path costs
  one relaxed atomic load per packet.

### 2.5 DNS

System DNS keeps going through the main tunnel resolver, as documented in
`split-tunneling.md`: resolution happens in a system service that cannot be
attributed to an app. The Internet traffic itself leaves from the chosen
country; sites that geolocate by client IP see that country.

### 2.6 What the user sees

For each routed app: its flag, the country, the live state (connecting,
connected, unavailable), and the public IP it appears from.

The daemon combines the resolution of each exit with what the tunnel reports
about its route sessions, and publishes `DaemonEvent.app_routes` when that
changes: tunnel not connected is `unavailable (tunnel down)`; an exit the main
connection already matches is `connected` with the main exit's address; a
route session is `connecting` until it reports, `connected` with its exit's
address, or `unavailable` (`no token`; `limit` when the exit refused every
token, the usual cause being serials held by live sessions; `no relay` when
nothing matches or the session failed). The public IP is the address the exit
node is listed on: each exit egresses from it (measured on four beta exits,
section 6).

## 3. VPN only for (include-only)

The default route stays on the physical network. Only the included apps are
tunneled, and they are fail-closed: an included app never reaches the Internet
outside the tunnel, also while the tunnel reconnects. DNS for the whole system
keeps going through the tunnel resolver (routed into the tunnel by each
platform, see below), so an included app's names never go to the ISP.

| OS | mechanism |
|---|---|
| macOS | the existing split-tunnel classifier (eslogger process tracking, pf route-to into the ST utun) with the decision inverted: included processes to the VPN, everything else to the default interface. A packet is attributed to the pktap effective pid when there is one, so WebKit's network process working for Safari counts as Safari. A process the monitor does not know yet is dropped, and an included packet without the tunnel address is dropped. System DNS to the tunnel resolver never reaches the classifier (a pf rule passes it on the tunnel first). The error and blocked states block everything, the rest of the system included. Needs Full Disk Access and a signed build, like exclude. |
| Windows | the unmodified, Microsoft-signed Mullvad driver with the address pair swapped (tunnel address registered as "internet", physical as "tunnel") and the included apps registered as split; the tunnel gets its own `0.0.0.0/0` with a higher metric than the physical default instead of the two `/1` halves, and host routes to its resolvers; winfw's connected policy permits IPv4 outside the tunnel for non-included apps, and a hard block holds every app the driver splits to the tunnel interface and loopback in every state. IPv6 outside the tunnel stays blocked for every app. Included apps cannot reach the LAN. Details below. |
| Linux | an `included` cgroup (`warren-inclusions`, cgroup2) marked in nft; the tunnel table lookup (pref 51) becomes conditional on that mark (`TunLookupScope::Marked`) and the exclusion bypass (pref 49) is not installed; marked traffic that would leave through anything but the tunnel is dropped; launched through `warren-include`, the mirror of `warren-exclude`, same owner-only rule. Details below. |
| Android | `VpnService.Builder.addAllowedApplication`, never mixed with `addDisallowedApplication` in one builder (Android refuses it), with a guard for an empty or fully uninstalled list (Android would otherwise capture everything), and the same allow list on every blackhole plan. Details in section 3.4. |
| iOS | not available. |

The GUI shows a persistent, calm warning while include-only is active: a
banner in the tab ("Only these apps are protected. Everything else on this
device uses your normal connection.") and a short label on the main screen
under the connection state. Turning the mode on asks for one confirmation.

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

**Status: enabled** (`talpid_core::split_tunnel::INCLUDE_ONLY_READY`),
validated in the Windows 11 ARM64 VM against beta exits on 2026-09-26 (below).
With the driver not loaded, include-only is refused and a persisted one falls
back to the full tunnel, with an empty split set (the included apps as the
driver's split set would be excluded).

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
  The connecting, error and lockdown policies add no permit.
- winfw adds no IPv6 permit: the driver guards one physical IPv6 address and
  an interface usually holds several (temporary addresses).
- A mode change while connected applies the blocked policy before the driver
  lets go of the included apps, and the driver's refusal of a mode leaves the
  tunnel state machine on the old one.

Included apps cannot reach the LAN.

**The hold.** The driver installs `PermitNonTunnel` (`firewall/filters.cpp`,
weight `ST_HIGH`, in winfw's baseline sublayer), whose callout soft-permits
every split app from any local address but its "tunnel" one, in every state
where it holds addresses (connected, connecting, and the error and lockdown
states). A TCP connection is redirected onto the tunnel address whatever it
binds, but a UDP socket an included app binds to a second interface's IPv4
address, or to an IPv6 address other than the registered one, would leave
outside the tunnel past `BlockAll`. winfw closes it without touching the
driver: `rules/includeonly/blockoutsidetunnel.cpp` adds, to every policy, a
hard (`definitive`) block of the held apps at `ALE_AUTH_CONNECT` and
`ALE_AUTH_RECV_ACCEPT`, IPv4 and IPv6, for anything that is not loopback and
not on the tunnel interface (all of it when there is no tunnel interface), in
a sublayer of its own, which outweighs a soft permit in any other sublayer.
The held apps (`split_tunnel/include_hold.rs`) are the listed ones, from
before the driver is handed a new list until it confirms taking it, and the
executables of every process the driver reports splitting, so an included
app's child of another executable is held too, from the driver's event
(`WinFw_SetIncludedApps`, device paths accepted). A path that does not resolve
is left out rather than turning the filter into a block of every app.

**Sublayers shared with the driver.** The driver adds its filters to winfw's
baseline and DNS sublayers by Mullvad's fixed keys (`firewall/identifiers.h`),
while winfw salts its keys per product environment so that one environment's
purge never removes another's kill switch. Those two keys are therefore never
salted (`windows/winfw/src/winfw/sharedsublayers.cpp`, README "Sublayers
shared with the split tunnel driver"): they belong to no provider, carry an
inert claim filter of the environment using them, are deleted only when no
filter is left in them, and are adopted only when nothing but the adopter's own
filters and the driver's is in them. An environment that finds another live
policy there uses private salted keys instead, and the daemon then refuses to
engage the split tunnel (`Unavailable`), so two kill switches never mix. Before
this, exclusion was as broken on beta as include-only: the daemon refused a
split mode with "The sublayer does not exist" (reproduced in the VM on the
previous build).

**The system resolver.** `PermitNonTunnelIpv4` has no condition, so the
system resolver's DNS over HTTPS or TLS to a resolver configured on the
physical adapter would pass it (port 53 stays confined to the tunnel
resolvers) and carry the names an included app looks up from the physical
address. The include-only connected policy therefore also hard-blocks the
Dnscache service SID on ports 443 and 853 off the tunnel interface
(`rules/includeonly/blocksystemresolver.cpp`, an `ALE_USER_ID` condition on a
security descriptor granting that SID). Reproduced in the VM before the block,
with the adapter's DNS set to 1.1.1.1 and DoH auto-upgrade on: 20 packets to
1.1.1.1:443 on the physical adapter while resolving three names; none with it,
names still resolving through the tunnel resolver.

Residuals: a process the driver splits by inheritance is held from the
driver's event, once the state machine handles it, so a child of another
executable that binds a second address before then is not held yet; while it
runs, every other instance of its executable is held too (a hold is by app
id). A hold update that fails is retried (three attempts, then at the next
change) but does not block anything more meanwhile. A build that predates the
shared sublayers cannot run next to this one (winfw README).

**Validation, 2026-09-26**, Windows 11 ARM64 VM (the CI runner VM), x64 debug
daemon under emulation, ARM64 driver, beta, main exit DE (167.233.127.54);
"outside" is the address the host egresses from, since the guest's egress is
the host's. `curl.exe` (`C:\Windows\System32`) and copies of
`curl.exe`, `cmd.exe` and `powershell.exe` under other paths were the apps.

| check | observed |
|---|---|
| exclusion | `curl-ex.exe` excluded: the outside address; `curl.exe`, another copy and PowerShell: 167.233.127.54; the previous build refused the mode ("The sublayer does not exist") |
| include-only egress | included `curl.exe`: 167.233.127.54; a non-included copy, the excluded copy and PowerShell: the outside address |
| children | a non-included `curl` started by an included `cmd.exe`: 167.233.127.54 |
| sockets | a long-lived included connection's local address is the tunnel address (10.66.0.229), a non-included one's the physical (10.0.2.15) |
| routes | `Warren` holds `0.0.0.0/0` at metric 9000 and `10.66.0.1/32` (the resolver); no `0.0.0.0/1` or `128.0.0.0/1`; the physical default at metric 0 |
| second address, TCP | included `curl --interface 10.0.2.16` (a second address on the NIC): 167.233.127.54, redirected onto the tunnel by the driver |
| second address, UDP | an included app's NTP request from a socket bound to 10.0.2.16: no answer, 0 packets on the physical adapter (`pktmon`); the same from a non-included app: answered, leaving from 10.0.2.16 |
| second address, child | `powershell.exe` (not listed) started by an included `cmd.exe`, NTP from 10.0.2.16: no answer, 0 packets on the physical adapter; with the split-process report disabled, the same child's request left the physical adapter from 10.0.2.16 and was answered |
| encrypted DNS | adapter DNS 1.1.1.1 with DoH: 0 packets to 1.1.1.1:443 on the physical adapter under include-only (20 before the resolver block), names resolving |
| IPv6 | included TCP to an IPv6 destination: refused at once; included UDP bound to a second or the SLAAC IPv6 address: 0 packets on the physical adapter |
| exit cut (Windows Firewall block of the exit's address) | included `curl` by address and by name, its child, and its UDP: nothing; 0 packets to the destination and 0 DNS packets on the physical adapter; after the rule is removed the tunnel reconnects and the included app is back on the exit |
| mode switches under traffic | an included `curl` every 300 ms across three runs switching include-only, off, exclude and back: 613 answers all from 167.233.127.54, 17 failures during the reconnects, 0 packets on the physical adapter |
| restart | the daemon restarted with include-only persisted connects and routes the included app as before |
| WFP | the hold's 4 filters in their own sublayer; 45 filters in the shared baseline sublayer, 8 of them the driver's |
| per-app country, include-only | `curl-fr.exe` (not listed as included) with `fr`: 135.136.60.142, `curl.exe`: 167.233.127.54, a non-included copy: the outside address |
| per-app country, route cut | FR exit blocked: `curl-fr.exe` gets nothing, 0 packets on the physical adapter, `curl.exe` stays on 167.233.127.54; the route reconnects 60 s after the block is lifted |
| per-app country, full tunnel | `curl-fr.exe`: 135.136.60.142, the rest: 167.233.127.54 |

`talpid-core/src/firewall/windows/winfw_tests.rs` runs winfw against the real
filtering engine (elevated, ignored by default): the sublayers the driver uses,
a live foreign policy kept to itself, a sweep leaving the live pair alone,
teardown next to a foreign filter, the claim between policies, the hold by
path and by device path, an unresolved path holding nothing back, and the
resolver block under include-only only. Each was seen failing against a build
with its behavior removed.

### 3.3 Include-only and exclusion with per-app countries

The split mode decides which packets reach the Warren TUN device; the router
(section 2) then sends each tunneled flow to its app's session. Nothing else
couples them, and the daemon computes both from one `AppRoutingSettings`
(`effective_app_exits`, `effective_included_apps` in `mullvad-types`), so they
cannot disagree about an app.

- **Exclude and a country.** An excluded app's packets never reach the TUN, and
  its exit is out of the plan (`effective_app_exits` drops it), so no route
  session is opened for it alone. Excluded wins.
- **Include-only and a country, macOS.** The daemon hands the classifier the
  included apps and every app with a country in force. An included packet goes
  into the tunnel utun with the tunnel address as its source, which is also the
  socket's local address, so the owner lookup finds the app and the router
  sends it to its country.
- **Include-only and a country, Linux.** The included cgroup's traffic is marked
  into the tunnel table and masqueraded to the tunnel address (section 3.1),
  so the TUN shows the translated pair while the socket holds the physical
  address. The Linux resolver asks conntrack for the connection's original
  tuple first (`NETLINK_NETFILTER`, `IPCTNL_MSG_CT_GET` by the reply tuple,
  `talpid-app-routing/src/owner/conntrack.rs`), then looks the socket up by
  that tuple. The conntrack entry is confirmed in postrouting, before the
  packet can be read from the TUN. The kernel matches the tuple against either
  direction, so the original tuple is used only when the reply direction
  matched: a connection the far end opened is looked up as seen, as is one
  conntrack does not know. Without conntrack over netlink, or without the
  privilege (the daemon has it), the resolver stops asking and looks every
  flow up as seen. Tested in a network namespace of its own with a SNAT rule
  standing in for the masquerade, root and nft, ignored by default:
  `finds_a_connection_masqueraded_on_its_way_to_the_tunnel` and
  `finds_the_accepted_socket_of_a_connection_the_far_end_opened`, run in a
  privileged Linux container on 2026-09-26. Downlink, the router restores the main tunnel address
  and conntrack the physical one.
- **Include-only and a country, Windows.** The swapped driver binds an included
  socket to the tunnel address, which is what the owner lookup matches; an app
  with a country is included (measured in section 3.2).
- **Android.** No per-app country yet (section 3.5), so the split mode alone
  decides.
- **Mode switches while route sessions run.** On Linux a change to or from
  include-only, and on Windows a mode change, while connecting or connected
  reconnects the tunnel: the route
  sessions are stopped and awaited with the old tunnel, and the new one starts
  them from the plan in force, which the generator republishes once the
  settings are saved (a route change never reconnects the main session by
  itself). On macOS the classifier takes the new split apps in place and the
  route sessions keep running; the plan follows the saved settings live, so an
  app that becomes excluded loses its route and a session no app uses any
  more is stopped. On Linux, switching between off and exclude changes
  nothing in the tunnel (exclusion is chosen at launch), so the main session
  and the route sessions keep running and the plan follows the settings as on
  macOS. Re-applying the firewall after a split change keeps the route
  sessions' relays allowed, since the connected state keeps the relay list the
  tunnel last reported.

### 3.4 Android

Implementation: `lib/model/.../AppRouting.kt` (the resolution),
`app/.../service/WarrenTunInterfacePlan.kt` (`tunAppRouting`, the plans),
`WarrenQuinnAdapter.kt` (the drop policy), `WarrenTunnelPlatform.kt` (the
builder calls).

- The allow list is the included packages installed on the device, plus Warren
  itself. Warren has to be on it: the in-tunnel egress probe and the NAT-PMP
  client are sockets of the app aimed at the tunnel gateway, and outside the
  VPN they would reach the physical network, where the probe convicts a
  healthy exit. Everything else about Warren's own traffic is as in a full
  tunnel: the relay and token-mint sockets are protected, and the other API
  calls go through the tunnel.
- Guard: a builder given no allowed package, or only packages it cannot find,
  captures every app. So an include-only list with no installed package never
  reaches the builder as an allow list: the tunnel is a full tunnel, which
  keeps the chosen apps protected, and the screen says so ("None of the apps
  you chose is on this device, so every app uses the VPN until you choose
  one."). The connect screen label counts only installed apps, and is absent
  in that case.
- Every blackhole plan carries the same allow list: the included apps stay
  captured while the tunnel is down, and every other app stays online. In
  exclude mode the blackhole still captures every app, excluded ones
  included, as before.
- In include-only a drop stays blocked whatever the lockdown setting: a
  flapping tunnel parks behind the blackhole instead of releasing traffic, and
  an expired account blocks instead of releasing, because the blackhole strands
  no other app and releasing would let an included app out in clear.
- A list change reconnects a connected tunnel, and replaces a blackhole that is
  up (new one established first, old one closed after), so an app added while
  blocked is held at once.
- The system setting "Block connections without VPN" blocks every app outside
  the VPN, so with it on only the included apps have Internet; the tab says so.
- DNS: an included app resolves through the tunnel like any tunneled app. An
  app outside the list uses the physical network's resolver, as an excluded
  app does (`split-tunneling.md`).

### 3.5 Android per-app country: the next step

Not implemented, and the tab is not shown on Android. The route sessions and
the router are the desktop ones (sections 2.2 to 2.4) ported into
`warren-jni`; what Android lacks is the flow owner. Every packet of a tunneled
app already reaches the engine through the TUN, and
`ConnectivityManager.getConnectionOwnerUid` (API 29+, callable by the active
VPN app) answers the uid owning a TCP or UDP 5-tuple seen there, which
`PackageManager.getPackagesForUid` names. The lookup is a Binder call, so it
follows the desktop rules: once per new flow, never per packet, and an
unresolved owner goes through the main connection.

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
  touching the host network):
  `WARREN_MNEMONIC="$(cat ~/.warren/app-routing-test-wallet.mnemonic)" cargo
  test -p mullvad-daemon --lib real_exit -- --ignored --nocapture`
  (`mullvad-daemon/src/warren_app_routes/real_exit.rs`). It mints the current
  epoch's batch with the daemon's manager and hands it out through the
  daemon's token source. Both sessions are admitted on tokens: the main one
  under its default admission but with a random key in place of the wallet's,
  so an exit that admits it can only have admitted a token, and the route
  session under tokens-only admission. The wallet needs a subscription and
  two free serials this epoch; a wallet whose epoch was issued to a client
  blinding at random is refused its derived batch. Measured 2026-09-26 on beta
  (RO main and DE route, then FI main and FR route, both with a wallet-admitted
  main session): each app appeared from its own exit's listed address, the
  routed app failed once its route was stopped while the unrouted one kept
  working, none of the routed app's packets reached the main session, and a
  tokens-only route session with no token reported `no token` and never
  connected. Measured again 2026-09-26 with both sessions on tokens (RO main
  135.136.59.234, DE route 167.233.127.54): the same results, and the route
  session led with a serial other than the main session's. Run again on the
  integrated branch (modes, GUI and datapath, warrenguard `88f4ed0`) the same
  day, with the same results on the same exits.
  The full daemon in a Debian 13 VM (kernel 6.12, aarch64, release build,
  beta, 2026-09-26, `warren-exclude` and `warren-include` setuid as packaged),
  main session on RO 135.136.59.234: a copy of `curl` with DE egressed from
  167.233.127.54, one with FR from 135.136.60.142 and every other program
  from the main exit, the three sessions admitted on the wallet's three
  tokens at once. With the DE relay blocked by nft inside the VM the DE app
  got nothing (its route `connecting`, curl timing out) while the main exit
  kept answering, and it recovered once unblocked. Include-only: an app opened
  through `warren-include` with DE egressed from DE, another from RO, and a
  program not included from the VM's own address. Exclusion won over a
  country: the excluded app egressed from the VM's own address, and the same
  program started normally from RO. Adding or changing a country never
  reconnected the main session; switching to or from include-only did, and
  the routes came back. A capture on the physical interface over all of it
  held no packet to the IP echo service, no DNS and no tunnel source
  address. The same checks passed again with 300 more processes and 800 more
  sockets on the VM. FI, NL and SG refused all three tokens as route exits
  while a serial was free (FR admitted it seconds later): a matter for those
  exits, not looked into from the client. The daemon in the Windows ARM64 VM
  (include-only with the swapped driver, per-app country): section 3.2.

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
  runs, resolved through `PATH`, symlinks and shell wrappers whose last line
  is `exec [-a NAME] PROGRAM ... "$@"` with a literal program
  (`desktop/packages/mullvad-vpn/src/main/linux-app-routing.ts`). A Flatpak or
  Snap app, or a script whose program cannot be read, is listed with the
  reason and offers no country; the file picker reaches the real program. In
  include-only mode the tab says a country applies to the app opened from VPN
  only for.
- While include-only is on, the connection card's line under the state reads
  "Only selected apps are protected" instead of "You are protected", above the
  "VPN only for N apps" label (no count on Linux). "N apps in other countries"
  sits among the feature badges (red when a route cannot run). Both open their
  tab.
- The mocked Playwright specs `app-routing.spec.ts`,
  `app-routing-windows.spec.ts` and `app-routing-linux.spec.ts` render the
  Windows and Linux views on any host through `WARREN_E2E_PLATFORM`, which the
  preload reads only under the end-to-end harness (`CI=e2e`).
- `app-routing-locales.spec.ts` opens the view in other catalogs through
  `WARREN_E2E_LOCALE` (read by the mocked main under the same harness, and the
  only way to reach ar, fa and uk, which the language picker does not list),
  fails when a tab label is clipped or widens its tab, and screenshots each
  state per locale. `APP_ROUTING_LOCALES=all` renders every catalog; run it
  after changing any App routing copy.
