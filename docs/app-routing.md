# App routing: exclude, per-app country, VPN-only-for

Status: being implemented on branch `feat/per-app-exit` (2026-09-25): per-app
country runs in the datapath on desktop (sections 2.2 to 2.6).
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
keeps going through the tunnel resolver (reachable through the tunnel's own
subnet route), so an included app's names never go to the ISP.

| OS | mechanism |
|---|---|
| macOS | the existing split-tunnel classifier (eslogger process tracking, pf route-to into the ST utun) with the decision inverted: included processes to the VPN, everything else to the default interface. Needs Full Disk Access, like exclude. |
| Windows | the unmodified, Microsoft-signed Mullvad driver with the address pair swapped (tunnel address registered as "internet", physical as "tunnel") and the included apps registered as split; the tunnel gets its own `0.0.0.0/0` with a higher metric than the physical default instead of the two `/1` halves; winfw gets a policy that permits non-included apps. IPv6: when the tunnel has no IPv6, winfw blocks included apps' IPv6 (by app id) so no IPv6 flow escapes. Included apps cannot reach the LAN. |
| Linux | an `included` cgroup (`warren-inclusions`, cgroup2) marked in nft; the tunnel table lookup (pref 51) becomes conditional on that mark (`TunLookupScope::Marked`) and the exclusion bypass (pref 49) is not installed; marked traffic that would leave through anything but the tunnel is dropped; launched through `warren-include`, the mirror of `warren-exclude`, same owner-only rule. Details below. |
| Android | `VpnService.Builder.addAllowedApplication`, with a guard refusing an empty or fully uninstalled list (Android would otherwise capture everything), and the same allow list on every blackhole plan. |
| iOS | not available. |

### 3.1 Linux

The daemon creates the cgroup at start; `warren-include` only joins it (it never
creates one, so an app can never land in a cgroup the firewall does not know)
and refuses to run the program when it is missing. `warren split-tunnel add
<pid>` puts a running process in it while include-only is on. Exclusion and
include-only never shape the same state: switching modes reapplies the
firewall and reconnects the tunnel so the routes follow; cgroup membership is
kept, so switching back restores it, and a process left in the cgroup of the
mode not in force is simply routed like any other.

The nftables policy (`talpid-core/src/firewall/linux.rs`, `include_only_head`
and `include_only_tail`):

- the mangle chain marks the included cgroup's sockets (`socket cgroupv2`)
  and every later packet of their connections (conntrack mark), which the
  route chain reroutes into table 100;
- while connected, traffic to the tunnel resolvers is marked the same way:
  the system resolver belongs to no app, and the tunnel address is a `/32`
  with no subnet route, so without the mark it would leave on the physical
  network;
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
- everything else is accepted both ways in every state, blocked ones
  included, except probes for the tunnel address.

Validated in a Debian 13 VM (kernel 6.12) against a beta exit: an included
`curl` egresses from the exit address while the rest of the VM keeps its own;
with the daemon cut from the network the included app times out, then is
refused at once in the connecting state, while the rest of the VM keeps
working; no packet to the included app's destination, no DNS and no tunnel
source address appeared on the physical interface. Included apps cannot reach
the LAN: their traffic, LAN destinations included, goes to the tunnel.

The GUI shows a persistent, calm warning while include-only is active: a
banner in the tab ("Only these apps are protected. The rest of your device
uses your normal connection.") and a short label on the main screen under the
connection state. Turning the mode on asks for one confirmation.

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
  session led with a serial other than the main session's.
  The full daemon in a Linux VM (per-app country,
  include-only, exclude); the daemon in the Windows ARM64 VM (include-only
  with the swapped driver, per-app country).
