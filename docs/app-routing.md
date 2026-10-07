# App routing: exclude, per-app country, VPN-only-for

Status on `main` (2026-09-28): per-app country runs in the datapath on
desktop (sections 2.2 to 2.6, real exits measured from macOS and Windows) and
on Android (section 3.5, beta exits measured from an emulator, routes admitted
by anchor); include-only runs on macOS, Linux, Windows and Android (section 3,
the Windows run in section 3.2, Android in 3.4); the desktop GUI is one list of
rules (section 7) and the Android screen follows it (section 3.5); section 3.3
is how the modes and the countries combine. Route sessions are admitted by the
main session's anchor where the server offers it, with no token each and no
artificial limit on the number of countries (section 2.2, warren-core doc 107).
This document is the contract the implementation lots follow. When the code and
this file disagree, fix one of them in the same commit.

The split tunneling page is **App routing**: one list of rules, each app with a
single route, and one choice for every app without a rule.

| route of an app | mode behind it | what the app gets |
|---|---|---|
| **Through the VPN** | the main connection | the main exit, like any app when the VPN is the default |
| **Through the VPN, another country** | per-app exit | its own exit country, every other app keeping the main connection |
| **Outside the VPN** | exclude, or no rule under include-only | the network as if Warren were off |

Any route through the VPN takes an option of its own, **Never without the
VPN** (section 8): the app is blocked whenever the VPN does not carry it,
the disconnected state and a stopped daemon included.

"Other apps go" chooses the default: **Through the VPN** (split mode `Off` or
`Exclude`) or **Outside the VPN** (split mode `IncludeOnly`, the former "VPN
only for"). A rule is an app whose route differs from the default, so the list
shows bypassing apps under the first and apps on the VPN under the second, and
the apps with a country under both. No mode has a switch: bypass turns on with
its first app and off with its last, the countries turn on with the first one,
and the default is the one explicit choice. The daemon model below is
unchanged; the view translates both ways (section 7).

## 1. Settings model

One settings struct, persisted on every platform that supports any mode
(Windows, macOS, Linux, Android). Today Linux persists nothing, because
exclusion there is launch-based; the new struct is persisted on Linux too.

```
AppRoutingSettings {
    split_mode: Off | Exclude | IncludeOnly,
    excluded_apps: set<AppId>,          // "Outside the VPN" rules, VPN default
    included_apps: set<AppId>,          // "Through the VPN" rules, direct default
    app_exits_enabled: bool,            // on with the first country
    app_exits: map<AppId, ExitChoice>,  // "another country" rules
    locked_apps: set<AppId>,            // "Never without the VPN" (section 8)
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
     everything else through the main connection;
  4. an app in `locked_apps` is never outside the tunnel: an `excluded_apps`
     entry is not in force for it (its country stays in force), it is
     tunneled under `IncludeOnly`, and it is blocked whenever the tunnel does
     not carry it (section 8). Locking an app drops its exclusion, so lifting
     the lock later does not send it outside the tunnel unasked.
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
| Android | `ConnectivityManager.getConnectionOwnerUid` (API 29+, a Binder call per new flow through JNI), then `PackageManager.getPackagesForUid` once per uid | per new flow, `betaBenchmarkRelease` on the `warren-test` emulator (API 35, 4 vCPU, 2 GB), bursts of 500 new UDP flows from `adb shell` (2026-09-27): p50 at most 1 to 2 ms, mean 1.5 ms on the first 1,000 lookups and 3.6 to 5.5 ms once the device was loaded, p99 16 to 131 ms, longest 212 to 288 ms. Made on the packet path, under the router's lock, it held every uplink packet of the tunnel that long; it now runs on four threads of the routing table (below) |

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
- On Android the owner lookup is not made on the packet path
  (`RoutingTable::with_owner_workers`, `Router::defer_owner_lookups`): a new
  flow's packets are held, sent nowhere, while one of four threads asks the
  platform, and every other flow keeps moving; once the owner is known the
  router decides with the same rules and the held packets go where it says
  (at most 64 packets per flow and 1,024 flows wait; past that a packet is
  dropped, as a full TUN queue drops it). The package of a uid is still read
  under the lock, once per uid. Measured with
  `android/scripts/perf/owner-lookup.sh` (bursts of 500 new UDP flows while an
  ICMP echo every 200 ms crosses the same uplink pump; echoes are never looked
  up), ping round trip through the NL exit, per burst:

  | build | p90 per burst | max per burst |
  |---|---|---|
  | no app with a country (no lookup) | 116 to 126 ms | 131 to 278 ms |
  | lookup on the packet path | 118, 143, 216, 464 ms | 155, 371, 433, 956 ms |
  | lookup on its threads | 101, 101, 108, 121, 183, 258 ms | 169 to 387 ms |

  Idle, the round trip was 85 to 111 ms. The emulator is loaded by the
  burst itself (500 processes spawned in 2 to 12 s), which the control row
  carries too; the lookups still cost CPU on the device, off the pump.
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
  supervisor as the main tunnel). It is admitted against the main session's
  anchor where the server offers route admission (warren-core doc 107), and on
  v7 anonymous tokens otherwise: **a route session never falls back to the
  wallet-signed v6 login.** When neither can admit it the route is shown as
  unavailable and its apps are blocked.
- Any number of distinct `ExitChoice`s is accepted (a country, or a city in
  one: `se` and `se`/`got` are two); the settings and the RPC refuse none. How
  many route sessions run at once is the server's answer, followed live by the
  tunnel (`route_capacity`): `max_routes` of the anchor once the control plane
  bound it (R = 32 on the server today), and 2 otherwise (main + 2 of the 5
  session tokens of an epoch, `TOKEN_QUOTA_PER_EPOCH`, which leaves 2 to the
  wallet's other devices), the case of a server
  with route admission off, a main session with no verdict yet, or one that
  cannot anchor. The routes of the plan past that number are reported
  `waiting for a free route`, their apps blocked, and start as soon as the
  answer grows or the plan shrinks; when it shrinks (an anchor lost), the
  routes past it are stopped and wait again. The router names a route with one
  byte, which bounds a tunnel at 255 route sessions whatever the server says.

Implementation (`mullvad-daemon/src/warren_app_routes.rs`,
`warren-app-routes/src/`, which the desktop tunnel re-exports as
`app_routes`):

- The rules of the plan and of what the user sees are written once, in
  `warren_app_routes::plan` (`warren-app-routes/src/plan.rs`: `plan`,
  `route_view`), and the desktop daemon and the Android engine each apply
  them through an adapter (`PlanRules`) that keeps only what the client does
  its own way: where the exits in force come from, how a city is spelled,
  how an exit is picked inside a choice, how the main exit is known, and the
  format of the statuses. `fixtures/app-routes-plan/` holds cases both adapters replay and
  must plan alike (schema, readers and skips in its README).
- The daemon's parameters generator resolves the exits in force every time
  one of its inputs changes (app routing settings, verified directory, main
  circuit, draining exits) and publishes an `AppRoutesPlan` on a watch
  channel. Every tunnel follows it live: a settings change starts or stops
  route sessions and never reconnects the main one. An unchanged plan is not
  republished, and the circuit a choice resolved to is kept while it stays
  valid, so a directory refresh does not move a route.
- A city choice matches the directory node whose city slug is the relay-list
  city code. A route has as many hops as the main connection, and a two-hop
  route keeps the main connection's entry constraint. A country or city
  chosen for an app constrains the exit only, so a two-hop route may enter
  outside that city, on both platforms (the fixture case
  `a_two_hop_city_route_enters_outside_that_city`).
- A route session runs the engine's supervisor and pumps with a random
  signing key (the wallet never enters it), one connection, and none of the
  hooks that feed process-wide state (dial-refusal cooldown, session
  placement, reconnect counter, entry RTT store, drain reactor). It is
  admitted by anchor or on tokens (see Route admission by anchor, and Tokens,
  below). After an end a retry may fix (no token, refused, failed) it waits
  60 s and dials again. A drain its exit announces is recorded like the main
  one's, and the new plan moves the route to another exit.
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

Route admission by anchor (warren-core doc 107 sections 10 and 11;
`talpid-warren-tunnel/src/app_routes/session.rs`, `route_anchor_for` in
`talpid-warren-tunnel/src/lib.rs`):

- The daemon's token manager reads the `route_admission` block of the session
  token directory on every refresh (warren-sdk-rs `TokenManager::route_admission`,
  validated: version 1, the KEM key signed by the API server key the daemon
  pins for its relay list and multi-hop directory and still inside its
  validity (warren-core doc 107 section 6.5), a usable X25519 key under a
  non-reserved key id, a non-zero R); the tunnel reads it through
  `RouteAdmissionSource`, the key when it starts and the listed exits before
  each route dial. An unusable, unsigned, badly signed or absent block reads
  as no route admission: the anchor and every locator are sealed only to a
  key the server key vouched for, since whoever could serve a key of its own
  would open them and link the main session to its routes.
- The daemon keeps the last signed block in its cache directory
  (`warren-route-admission.json`, `mullvad-daemon/src/warren_token_provider.rs`),
  and until the first directory read of a run it hands the tunnel that block,
  verified again (signature and validity) exactly as a fetched one. So a
  tunnel started right after a daemon start anchors: its main session binds
  the anchor at its first setup admitted on a token. The first directory read
  supersedes the kept block, and a directory without route admission removes
  it.
- A tunnel with per-app routes wired builds one `RouteAnchorHandle` from that
  key and hands it to its main supervisor (`with_route_anchor`) and to every
  route session. The daemon wires a plan into every desktop tunnel, so every
  desktop main session anchors while the directory offers a key, a country
  set or not, and a country chosen while connected then gets its route
  without a reconnect of the main session. A tunnel whose directory has
  offered no key yet builds the anchor without one
  (`RouteAnchorHandle::awaiting_key`, which reads `Unavailable`, so its
  routes run on tokens meanwhile) and hands it the key later (see A tunnel
  started before its credentials, below). The cost for a user with no country is one
  small control datagram after each main setup, and an anchor record in the
  API's RAM tied to a serial it already leases. The anchor secret stays
  inside the engine: it never crosses the daemon's gRPC or any FFI, and
  nothing logs it, a locator, a serial or an exit id.
- A v6 (wallet-signed) main session never anchors: the engine answers
  `Unavailable` and every route runs on tokens.
- A route to an exit the directory lists is dialed under
  `SessionAdmission::Route` while the anchor is bound or still waiting for its
  verdict (the engine waits up to 90 s for it before dialing). Any
  `MultiHopError::RouteRefused` (a `RouteRejected` code, a plain `Rejected`
  from an exit predating route admission, an anchor that is unavailable, a
  `RouteEnded`) is answered at once by the same route under `TokensOnly`, as
  routes ran before anchors. An exit that answered `not offered`, or predates
  route admission, is not asked by anchor again for the tunnel's life; any
  other refusal is asked by anchor again at the next attempt. A route to an
  exit the directory does not list, or with an anchor known unusable, runs on
  tokens directly.
- A route admitted by anchor is handed no token provider at all, so it holds
  no serial and leaves the wallet's five tokens to the main session and the
  routes that need them.
- At most 2 routes of a tunnel run on tokens at once, whatever the capacity:
  a route that has to run on tokens while two do is reported
  `waiting for a free route` and dials once one ends. Past the free serials a
  route would walk the serials the other sessions hold, which would show its
  exit the main session's serial.
- The capacity follows a bound anchor only: an anchor that becomes
  unavailable after it was bound (an API restart, a lost anchor the API
  cannot take back at once) leaves the running routes up, since they stay
  admitted at their exits until the control plane ends them (doc 107 section
  12); a route it ends falls back to a token, within the 2 token routes.
- A route added to the plan past the capacity waits; it never takes the place
  of a route that already runs.
- A route that runs on a token moves to the anchor once it could be admitted
  that way (`upgrade_by_anchor` in `app_routes/session.rs`), so it stops
  holding one of the wallet's five serials: once the anchor is bound and the
  directory lists the route's exit, a session by anchor to the same exit is
  dialed next to the token one; when it is up (its exit assigned its
  addresses), the route's packets move to it and only then does the session
  on the token end, releasing its serial and its place among the two token
  routes. The route has a live session throughout; the packets in flight at
  the switch may be lost, and connections the route carried are reset by the
  exit, since the new session has its own inner address there, as a route's
  reconnect resets them. A move that fails leaves the
  route on its token and is tried again 60 s later; an exit that refuses
  routes by anchor for good (`not offered`, or one predating route admission)
  is not asked again; a route whose by-anchor session was just refused waits
  60 s before a move, so a route cannot flap between the two.

A tunnel started before its credentials
(`warren-app-routes/src/main_anchor.rs`, shared by the desktop tunnel and
the Android engine):

- A fresh daemon has neither tokens nor the route admission key when its
  first tunnel starts: both come from the wallet's first refresh, which the
  first tunnel's parameters start. The daemon announces every finished
  refresh round on a watch channel (`Credentials`: rounds, and whether the
  wallet holds tokens this epoch), and the tunnel follows it. The Android
  engine does the same per process (`warren-jni/src/token_provider.rs`),
  and counts the current epoch's tokens of a restored bundle as already
  there, so the first tunnel of a process that restored them does not wait.
- The first tunnel of a daemon run holds its main session until the wallet
  holds tokens for the current epoch or the first round ends, 4 s at most
  (`FIRST_REFRESH_WAIT`; the firewall lets the daemon reach the API while
  connecting; a disconnect ends the wait). A round reads the token
  directory first, which carries the route admission key, then mints the
  current epoch and the later ones up to its horizon, one request each
  (warren-sdk-rs `TokenManager::refresh`), so the daemon looks at a running
  round every 100 ms and announces the current epoch's tokens as soon as
  they are there. A main session set up then is admitted on a
  token and anchors at once. A round that does not come (the API blocked on
  this network) costs the first connection those 4 s, once; later tunnels
  of the run never wait.
- When the key arrives after the main session was set up, the tunnel hands it
  to the anchor (`provide_key`), and the engine registers the anchor on the
  live session at once when that session was admitted on a token: no new
  setup, nothing reconnects.
- When the main session had to log in with the wallet (the engine says so:
  `MultiHopSupervisor::token_admission_rx`) and a round brings tokens while
  the anchor has its key, the tunnel asks the engine, once per tunnel, for a
  make-before-break setup of the main session
  (`MigrateHandle::overlap_reconnect`), which is admitted on a token and
  anchors. The exit places that session on another inner address than the
  wallet session's. The desktop tunnel adopts a new address by rebuilding
  itself (about 250 ms blocked, the connections of the moment reset), which
  is why the first tunnel waits for the first round instead. The Android
  engine follows it in place: its VpnService address is fixed and the main
  session's 1:1 remap reads the assigned addresses from a shared cell
  (`RemapAddresses`, `warren-jni/src/remap_tun.rs`), so the session goes on
  (before, any move of the inner address ended the session and handed it
  to Kotlin's drop policy); the connections of the moment reset as on
  desktop, and a forwarded port is asked for again, since the exit keys it
  on the inner address.
- Residual, evaluated 2026-09-27: the wallet session and the token session
  reach the same exit from the same address a moment apart (the wallet
  session closes as the token session takes over), so that exit can tie the
  token's serial, and the routes anchored on it, to the wallet; the source
  address alone would allow the same. Setting the token session up on
  another exit of the same country (`MigrateHandle::migrate_to`) was
  weighed and left out:
  - it removes the link from one exit's view only. The second exit sees the
    same client address on a one-hop circuit (the same entry relay on a
    two-hop one), and the wallet session's close at the first exit lines up
    in time with the token session's setup at the second, so whoever sees
    both exits, the fleet's operator included, still ties them;
  - it costs the user: the public address of every app on the main
    connection changes a few seconds after the connect (the setup on the
    same exit keeps that exit's address, only the inner one changes), a
    forwarded port does not follow to another exit, the location shown
    changes under the user, and a user who picked one server, or a country
    or city with a single exit, cannot be moved at all;
  - the beta fleet has one exit per country (six active exits, DE, FI, FR,
    NL, RO, SG, `GET /v1/exits` on 2026-09-27), so no such exit exists
    today.

  What narrows the residual is making the setup again rare: the first tunnel
  waits for the round's tokens, and on Android resolves the API host before
  its TUN so the round can finish during that wait (below), so the main
  session is admitted on a token from its first setup and the wallet never
  signs in; the setup again happens only when no token came within the 4 s.
- A route that ended for want of a token, or a waiting one, dials again as
  soon as a round brings tokens or the anchor is bound, not 60 s later.
- Android: the system resolver answers the app through its own VPN, so from
  the moment the TUN is established until the tunnel carries traffic a name
  lookup waits for the tunnel. The round's sockets are protected
  (`VpnService.protect`), its lookups were not: on the `warren-test`
  emulator the token directory request resolved in 2 to 20 ms, and the next
  request of the round (the current epoch's mint) waited 4.3 s for its
  lookup, answered 0.1 s after the tunnel came up, so the first tunnel's wait
  always ran out and its main session signed in with the wallet. The mint
  transports now keep a host's resolution for 5 minutes
  (`ResolveCache`, `warren-jni/src/protected_transport.rs`), primed before
  the TUN while Kotlin fetches the multi-hop directory, and dropped when none
  of its addresses connects.
- Measured 2026-09-27 on the `warren-test` emulator (API 35, arm64, a
  `betaRelease` build installed next to the existing app under another
  application id, fresh data, the routing test wallet 2), main on NL over two
  hops, Chrome given DE and FOSS Browser given FI, first connect of the
  process. Before the resolution fix (two runs): the wait ran out after 4 s,
  the main session signed in with the wallet, the round's tokens came 0.3
  to 0.4 s after the tunnel, the main session was set up again on a token
  and its new inner address followed in place, the anchor was bound 5.5 to
  5.9 s after the connect, and the two routes, started on tokens, moved to
  it. With it: the wait ended after 1.0 s on the current epoch's tokens,
  the main session was admitted on a token, the anchor was bound 1.4 s
  after the connect (its key comes with the end of the round, since the
  Android manager says it has read the directory only then), and both
  routes moved from their token onto the anchor within 1.8 s of the
  connect, with no setup again ("Reconnects 0"). Chrome read
  167.233.127.54 (DE), FOSS Browser 37.27.217.153 (FI), and a request from
  `adb shell`, which has no country, 50.7.46.90 (NL).
- Measured 2026-09-27 in a Debian 13 VM (Lima, aarch64, daemon-only package
  built with `./build.sh --daemon-only --optimize`, beta env), main on NL and
  five route countries (DE, FI, FR, RO, SG). Fresh install, first connect:
  the wait ended after 1.4 s on the current epoch's tokens, the tunnel was up
  1.7 s after the connect, the anchor was bound with it (`max_routes=32`),
  and each routed app egressed from its own exit's listed address, every
  route by anchor. Six daemon starts with auto-connect and no kept block:
  five the same way (tokens within 1.4 s, anchor bound, no refresh failure,
  no rebuild); in one, no token came within the 4 s, the first round ended
  with `request timed out` (a connection it had opened on the physical
  interface was stranded when the tunnel took the route, and is not sent
  again since it had connected), the round 15 s later brought the tokens,
  the wallet session was set up again on a token, the tunnel rebuilt itself
  once, and the anchor was bound 27 s after the start. Why that round was
  slow before the tunnel came up is not established. The released 1.1.37
  build, in the same VM, left the first route without a token or an anchor
  after such a start (`all API hosts are unreachable`).

Tokens (`mullvad-daemon/src/warren_token_provider.rs`):

- The wallet's batch is blinded from its seed, so every client of the wallet
  holds the same five tokens an epoch (three until 2026-09-27), and an exit
  leases each serial to one live session in the whole fleet. A session is
  handed the whole current-epoch batch and never consumes it. The exit spends
  the first token of a setup that verifies: a route session (tokens-only admission) sends the lead token
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
  then 30 s, doubling up to the 10 minutes (`tokens::refresh_forever`, the
  same loop on Android, which used to wait the whole 10 minutes).
- Why the first round used to fail (`all API hosts are unreachable`,
  reproduced 2026-09-27 in the Debian 13 VM on a daemon started with
  auto-connect): it opens its connection when the first tunnel's parameters
  are generated, on the physical interface, with the physical source address
  (tcpdump: the SYNs leave `eth0`). A tenth of a second later the tunnel
  takes the default route, and every retransmission of those SYNs then goes
  into the tunnel still carrying the physical source (tcpdump: `tun0 Out IP
  192.168.5.15.* > <API>.443 [S]` once a second), which the exit never
  answers. After the SDK's 5 s connect budget its only fallback, the same
  host without SNI, gets a TLS `internal_error` alert from the API (checked
  with `openssl s_client -noservername`), so the round ends with every host
  reported unreachable, the account standing poll with it. Mullvad's own API
  client resets its sockets on the same tunnel transitions; the Warren
  fetchers had nothing equivalent. Now the daemon's Warren transport
  (`mullvad-daemon/src/warren_api_transport.rs`) builds a fresh connection
  pool whenever the routes move (the tunnel comes up, a disconnect ends, an
  attempt fails), and sends a request again, twice at most, when it failed to
  connect while the routes moved. Only a failure to connect is sent again:
  the request then never reached the API, so its signature is not replayed,
  which the API would refuse (the SDK's own host fallback re-sends a signed
  request on the same condition).

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
- An app that changes session while connected (another country or city, its
  country removed, a country given to an app that had none, a route that
  ended for good and whose slot another session took) has its TCP
  connections reset toward it at once (`talpid-app-routing/src/reset.rs`). A
  connection lives at the exit it was opened through, which no longer sees
  it, and any other session would carry it to an exit that drops it as
  unknown: the app would wait on it until its own timeouts. The router keeps,
  per flow, the process found holding it and the last acknowledgment the app
  sent; a new policy asks it about each of those processes again, and for
  each TCP connection whose session changed it writes to the TUN a reset from
  the remote end carrying exactly the sequence number the app expects (the
  only one RFC 5961 accepts), then drops every later segment of that
  connection and answers each with the reset a closed port sends. The
  controller writes the resets right after the new policy, the main
  session's device the answers. A UDP flow of such an app is told its port
  is unreachable (an ICMP error from the remote end, which a connected
  socket reports as a refused connection) and forgotten: a QUIC connection is
  then dropped and opened again rather than migrated, which would show the
  server one connection arriving from both exits, and the next datagram of a
  socket that carries on is attributed again and takes the new session. A flow
  whose app keeps its session keeps it, whatever its route id becomes: the
  controller names each route's session (its slot's generation) to the
  router. A connection under way that the router never tracked (opened while
  no app had a country, or through main before its app got one) and that
  belongs to a routed app is reset at its first segment rather than carried
  to the route. While connections reset by a policy that routes nothing are
  closing (10 s after their last segment), the router stays in the path, so
  none of their segments reaches the main session; a connection it was
  already dropping (under way, with no owner found) is kept dropped for as
  long, and a new connection on the ports of a closing one goes through main.
  Residual: a reset is written once; an app that never receives it (a TUN
  write that fails) and then stays silent past the 10 s finds its connection
  attributed again.

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
address, `waiting for a free route` when it is past what the server admits at
once (section 2.2), or `unavailable` (`no token`; `limit` when a route on
tokens was refused every token, the usual cause being serials held by the
account's other sessions; `no relay` when nothing matches or the session
failed). The public IP is the address the exit
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

While include-only is active the desktop view says it under the default
("Direct connection by default. Only the apps below use the VPN."), warns when
no rule puts any app in the VPN, and the main screen labels the connection
state "Only selected apps are protected" with the number of apps on the VPN.
Choosing it asks once on desktop ("Only the apps you choose will use the VPN.
The rest of this device will not be protected."), since it takes every app
without a rule off the VPN at once; going back through the VPN never asks.
Android asks where the choice narrows a full tunnel (section 3.5).

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
- **Android.** Exclusion is `addDisallowedApplication`, so an excluded app's
  packets never reach the TUN, and `effectiveAppExits` (`lib/model`) drops its
  country, as the daemon does. In include-only an app with a country in force
  joins the allow list (`resolveAppRouting` takes the apps with a country), so
  choosing a country is enough to put it in the VPN; its flows then reach the
  TUN with the TUN address, which is what the owner lookup is asked about.
  While the list holds no app on the device, the tunnel is the full tunnel of
  section 3.4, so a first country would take every other app out of the VPN;
  the screen asks before that choice, and before its switch is turned on over
  saved countries (section 3.5).
  A change of the countries in force reaches the engine live and never
  reconnects the main session; a change that alters the include-only allow
  list re-establishes the TUN, as any list change does (section 3.4).
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
  keeps the chosen apps protected, and App routing says so in a warning
  (section 3.5). The connect screen label counts only installed apps, and is
  absent in that case. A first rule in that state, the VPN or a country for an
  app of the device (an app with a country joins the list, section 3.3),
  would turn the full tunnel into a list, so the screen asks first
  (section 3.5).
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
  the VPN, so with it on only the included apps have Internet; App routing
  says so under its list whenever some app is outside the VPN.
- DNS: an included app resolves through the tunnel like any tunneled app. An
  app outside the list uses the physical network's resolver, as an excluded
  app does (`split-tunneling.md`).

### 3.5 Android per-app country

The route sessions, their controller, their admission (anchor, then tokens)
and the router devices are the desktop code: they moved from
`talpid-warren-tunnel/src/app_routes/` into the `warren-app-routes` crate,
which the desktop tunnel re-exports as `app_routes` unchanged and `warren-jni`
runs too, with the session token leads (`tokens::session_source`, one provider
per session, each holding the serial it leads with) and the route admission
source (`admission::DirectoryRouteAdmission`, the kept signed block). What is
Android's own:

- **The flow owner** (`warren-jni/src/flow_owner.rs`). The router's "pid" is
  the uid `getConnectionOwnerUid` answers for the 5-tuple seen on the TUN
  (source `10.64.0.1` or `fd00::1`), asked through the Kotlin
  `FlowOwnerResolver` (`android_app_routes.rs`, kept by `proguard-rules.pro`),
  and its "program" is its package. The lookup is live, so there is no
  snapshot, and a call that fails (a Binder error, a Java exception) counts as
  an owner not found: the flow goes through the main connection under the rule
  of section 2.1, which on Android covers more failure modes than a table
  read does on desktop. An app id below 10000 in any user or profile (the
  system's own, among them the DNS resolver
  working for apps) has no owner and goes through the main session, as the
  system DNS does on desktop (section 2.5). A uid carrying several packages
  (a shared user id) is named after one of its packages with a country, the
  first by name: its packets cannot be told apart, and routing them keeps a
  routed app's packets off the main session. Traffic a system service sends
  for an app (a download through `DownloadManager`, a push delivery) belongs
  to that service's uid and goes through the main session. Without a
  registered lookup no route is dialed and every app with a country is shown
  unavailable, since its flows could not be told from the others. An install
  or an uninstall
  (`ACTION_PACKAGE_ADDED` or `_REMOVED`, `notifyPackagesChanged`) makes every
  uid be named again. Below Android 10 no lookup is registered and the tab
  says the feature needs Android 10.
- **The plan** (`warren-jni/src/app_routes_plan.rs`, `app_routes_session.rs`):
  the shared rules of `warren_app_routes::plan` that the desktop daemon
  applies too (a choice the main exit matches rides the main session, apps of
  one exit share one route, a choice nothing serves blocks its apps, a valid
  circuit is kept, a draining exit is planned away, and `route_view` for what
  the user sees), through Android's own adapter: the exit is picked inside a
  country the way the Android main connection picks (`pick_exit`, then
  `circuit_select` with the main connection's hops and entry country), a city
  matches the relay list's name or its code, the main exit is the one the main
  session is on, and the statuses are the JSON Kotlin parses. Both adapters
  replay `fixtures/app-routes-plan/`. One task per tunnel follows the
  countries Kotlin sets (`setAppRoutes`, live) and the route reports, and
  publishes the statuses Kotlin reads on every status wake
  (`getAppRoutesStatus`).
- **The datapath** (`AppRoutes` in `android_app_routes.rs`): `RoutedTun` over
  the VpnService TUN under the main session's `RemapTun`, whose local address
  is the router's main address, so the router's own address translation
  replaces the remap for a routed flow. The plan's policy is installed before
  the main pumps run; route sessions start once the main session is up and are
  stopped and awaited with each attempt. `VpnService.protect` is process wide,
  so a route's carrier needs no escape and no relay is named to a firewall.
  The main session anchors whenever the lookup is registered, a country set
  or not, and waits for the key when the token directory has offered none
  yet (a first connect; section 2.2, A tunnel started before its
  credentials, for the first connect of a process). The token manager
  of the wallet trusts the pinned server keys for that signature, keeps the
  last signed block in the app's files directory, and says when it has read
  the directory in this process (a restored token bundle knows its epoch
  before any directory read, which is what the daemon's test relies on).

The screen is the desktop's single list (section 7). "Other apps go"
chooses the default route, "Through the VPN" or "Outside the VPN" (which is
include-only), and "Rules per app" lists every app of the device whose route
differs from it, with a chip (a flag and the country, "Outside the VPN", or
"VPN") and, for a country, the status line of its route. A row, or an app
picked from "+ App" (the apps without a rule, a search and the system apps
switch), opens the app's route page: through the VPN, through the VPN from
another country, or outside the VPN. The option the other apps take carries
"Default", choosing it or "Remove the rule" gives the app the default again,
and an app picked from "+ App" gets no rule until another option is chosen.
The country option opens a page of the countries and cities with an active
server in the relay catalogue, which never moves the main connection. Below
Android 10 that option is disabled and says the feature needs Android 10.
There is no switch: bypass turns on with the first app sent outside the VPN
and off with the last one, and the countries turn on with the first one.

The translation between the list and the settings is `AppRoutingSettings`
(`lib/model/.../AppRoutingRules.kt`), the Kotlin twin of the desktop
`src/shared/app-routing.ts`, and `AppRoutingRulesTest` replays the desktop's
cases. `rules()` is the list, one route per app after the precedence of
section 1, with what is saved but not in force left out. `planAppRoute` and
`planDefaultRoute` are the writes of one change, and
`SplitTunnelingRepository.apply` makes them one at a time and in their order,
so the tunnel, which follows each write, never routes the app a third way and
never moves another app. The default change empties both lists and keeps the
countries: toward the VPN the mode goes off first, toward direct it becomes
include-only last.

Two parts are Android's own. With "Outside the VPN" as the default and no app
of the device in the list, the tunnel runs as the full tunnel of section 3.4,
so the list shows a warning that every app uses the VPN until one is added,
in place of the empty state. And a change that turns a tunnel carrying every
app, or every app but the bypassing ones, into a list asks first: the first
rule in that state ("Only <app> will use the VPN", saying every other app
will use the normal connection), and "Outside the VPN" chosen as the default
while apps have a country (only those apps will use the VPN). Cancelling
leaves the settings as they were. The question is `changeNarrowsTunnel`,
which resolves the settings before and after the change with
`resolveAppRouting`, the function the tunnel resolves its apps with. While
some app is outside the VPN, a note under the list says that the system's
"Block connections without VPN" leaves those apps without Internet.

The connect screen carries the "N apps in other countries" badge, red when a
route cannot run for a reason other than the tunnel being down, which opens
App routing. Code: `lib/feature/splittunneling/impl/` (the list, the route,
add and country pages, `SplitTunnelingViewModel`),
`lib/feature/home/impl/.../connectioninfo/AppCountriesSummary.kt`; the
settings live in `WarrenLocalSettingsRepository` (`split_tunneling_mode`, the
two app lists, `app_exits`, `app_exits_enabled`).

Validation, 2026-09-27, beta build of this branch on the `warren-test`
emulator (API 35, arm64), subscribed wallet already in the app, Chrome and a
second browser (FOSS Browser, from F-Droid) as the two apps, IP echo through
`api.ipify.org`:

| check | observed |
|---|---|
| two-hop main on NL (50.7.46.90), Chrome DE, the other browser FI | Chrome 167.233.127.54, the other 37.27.217.153; the tab showed each "Connected, IP" with that address; the connect screen "2 apps in other countries". First connect of the process: no kept block yet, so no anchor; the routes ran on tokens and waited 60 s for the first mint of the process |
| country removed while connected | the browser appeared from 50.7.46.90 at once |
| one-hop main on NL, second connect | main v7, `route anchor bound max_routes=32`, both routes up 0.2 s later with no token fallback; Chrome DE, the browser FI |
| FI relay dropped by `iptables` inside the emulator | the browser loaded nothing (no fallback to NL), its row read "Connecting...", Chrome stayed on 167.233.127.54; 25 s after the rule was removed the route was back and the browser appeared from 37.27.217.153; the main session never reconnected |
| Chrome moved from DE to FR while connected | the tab showed FR (135.136.60.142) at once; a URL by address (`1.1.1.1/cdn-cgi/trace`) answered `ip=135.136.60.142 loc=FR`; names did not resolve in Chrome until it was restarted (below) |
| Arabic | the tab mirrored, the address of "Connected, IP" kept left to right |

The Chrome row predates the reset of section 2.4: the router then forgot the
flows it knew on a new policy, so the app's connections under way moved to
the new route, whose exit dropped them, and Chrome waited on them (its name
resolution included) until it was restarted. With the reset, on the same
emulator (`betaBenchmarkRelease`, main on NL, 2026-09-27): Chrome showed
`api.ipify.org` from DE (167.233.127.54); moved to FR, the same page loaded
again from FR (135.136.60.142) and a name it had not resolved yet
(`icanhazip.com`) loaded from FR as well, each within 6 s of the move, with
no restart.

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
- Parity: the plan cases of `fixtures/app-routes-plan/`, replayed through the
  desktop and the Android planners, which must plan and show them alike.
- Real exits: a harness that runs the router with a main session and a route
  session to every other beta exit, and fetches the public IP through each
  (runs on macOS without touching the host network):
  `WARREN_MNEMONIC="$(cat ~/.warren/app-routing-test-wallet.mnemonic)" cargo
  test -p mullvad-daemon --lib real_exit -- --ignored --nocapture`
  (`mullvad-daemon/src/warren_app_routes/real_exit.rs`, both tests). It mints the current
  epoch's batch with the daemon's manager and hands it out through the
  daemon's token source. The main session is admitted on a token, under its
  default admission but with a random key in place of the wallet's, so an
  exit that admits it can only have admitted a token. When the token
  directory offers route admission the main session anchors and the routes
  to the listed exits are admitted by anchor, with no token each; otherwise
  every route runs on tokens under tokens-only admission, two at most, and
  the others wait. The wallet needs a subscription and free serials this
  epoch; a wallet whose epoch was issued to a client blinding at random is
  refused its derived batch. `WARREN_MAX_ROUTES` caps the number of routes.
  Measured 2026-09-26 on beta with route admission not yet offered by the
  token directory (the token fallback): main on RO 135.136.59.234, five
  routes planned (DE, FI, FR, NL, SG); DE (167.233.127.54) and FI
  (37.27.217.153) connected on tokens and each app appeared from its exit's
  listed address, FR, NL and SG were reported waiting for a free route and
  their apps got nothing, no routed packet reached the main session, the
  unrouted app kept the main exit, and a stopped route blocked its app. The
  anchor path's live proof is left to the validation run once the server side
  is on. Measured earlier the same day on beta
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
  tokens at once (the quota was three then). With the DE relay blocked by nft
  inside the VM the DE app got nothing (its route `connecting`, curl timing out) while the main exit
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
- A country change under an open connection (section 2.4), same harness:
  `an_app_moved_to_another_country_has_its_open_connection_reset_and_reconnects_there`
  runs one route at a time, so the second route takes the first one's slot.
  Measured 2026-09-27 on beta, main on RO (135.136.59.234), both routes on
  tokens: the app held a kept-alive HTTP connection through DE
  (167.233.127.54) and was moved to SG; the reset reached it 1.9 ms after the
  new plan, at the sequence number it expected, without it sending anything;
  a request on the old connection was answered with a reset, and none of its
  packets was routed or reached the main session; the app's next connection
  came from SG (5.223.49.152). The same run with the router forgetting its
  flows on a new policy, as before the fix: no reset within 10 s, the
  connection left hanging.

## 7. Desktop GUI

The view keeps the split tunneling route (`RoutePath.splitTunneling`), renamed
**App routing**. It shows the default ("Other apps go", two segments), then
"Rules per app" with a "+ App" button, one row per rule (the app, and a chip
with its route: a flag and the country or city, "Outside the VPN", or "VPN").
The add, route and country screens take the whole window in turn, like sheets,
and Escape or their bottom button closes them. The design is poka's mockup of
2026-09-28 (six states: empty, rules with the VPN as the default, the route of
an app, the country choice, Outside the VPN as the default, a build that cannot
run it).

The translation between the list and the daemon lives in
`desktop/packages/mullvad-vpn/src/shared/app-routing.ts`:

- `appRouteFor` is the route the daemon gives an app, after the precedence of
  section 1, and `appRules` the apps whose route differs from the default.
  Entries saved but not in force (an exclusion list while the mode is `Off`,
  countries while `app_exits_enabled` is off, lists left by the older tabs)
  are hidden.
- `planAppRoute` is the ordered daemon calls that give one app a route. The
  calls are separate RPCs, so every state between two of them carries traffic:
  the order keeps the app on its old or its new route at each step, never a
  third one, and never moves another app. Entries saved while a feature was off
  are cleared before it turns back on, so no rule nobody sees comes back.
- `planDefaultRoute` switches the default. Toward the VPN the mode goes off
  first; toward direct it goes last; both lists are emptied, the countries kept.
- Changes run one at a time across every screen
  (`split-tunneling/hooks/use-routing-actions.ts`): each is planned from the
  settings the daemon last reported, and the next waits until those settings
  show the previous one (`routingReflects`, up to 3 s), since the daemon
  commits two overlapping requests over each other. Entries are removed under
  the id the daemon stored, whose case can differ from the view's, and a
  program picked in the file dialog is resolved to its daemon id
  (`appRouting.resolveApplication`) before its route is planned.
- `test/unit/app-routing-rules.spec.ts` replays each plan call by call and
  checks those three properties at every step.

Availability (`splitModeAvailability`): Outside the VPN, as a default or for an
app, is what the classifier or the driver runs, so on macOS it needs a signed
build, macOS 13 and Full Disk Access. When it cannot run, its segment and its
option are disabled with the reason under them (the mockup's unsigned state);
Full Disk Access shows a line with the way to the settings pane and, once
opened, the service restart. Going back through the VPN, and removing a rule,
are never refused. A country needs none of this.

Linux keeps no list for either mode, since an app leaves or joins the VPN when
Warren opens it: the route screen offers "Open outside the VPN" (with the VPN
as the default) or "Open through the VPN" (with direct as the default) as an
action that launches the app from its desktop entry (`launchPath`, set by
`getPathBasedApplications`), and the rules are the countries only. A browser
that hands a new window to its running process says, in the warning colour,
to close it first; one that launches elsewhere cannot be opened this way.
Under direct as the default an app with a country uses it when opened that
way, and both its row and its route screen say so; the empty list there says
how opened apps join the VPN rather than warning that none does. Apps are keyed by the program they run,
resolved through `PATH`, symlinks and shell wrappers whose last line is
`exec [-a NAME] PROGRAM ... "$@"` with a literal program
(`desktop/packages/mullvad-vpn/src/main/linux-app-routing.ts`). A Flatpak or
Snap app, or a script whose program cannot be read, offers no country and says
why; the file picker reaches the real program.

Each rule with a country shows its route state from `AppRouteStatus` (pushed as
`DaemonEvent.app_routes`) on a line under the name and the chip: connecting,
connected with its public IP, or the reason it is unavailable. A route waiting
for the main connection is not shown as a fault. There is no limit on the
number of countries to mirror: a route past what the server admits shows
`Waiting for a free route`. The country screen lists every country and city
with an active server and tags the exits already in use by another app.

While include-only is on, the connection card's line under the state reads
"Only selected apps are protected" instead of "You are protected", above the
"VPN only for N apps" label (no count on Linux). "N apps in other countries"
sits among the feature badges (red when a route cannot run). Both open the
view.

- The mocked Playwright specs `app-routing.spec.ts` (the flow on macOS, the
  calls it makes to the daemon in order, the unsigned build, French),
  `app-routing-windows.spec.ts` and `app-routing-linux.spec.ts` render the
  Windows and Linux views on any host through `WARREN_E2E_PLATFORM`, which the
  preload reads only under the end-to-end harness (`CI=e2e`).
- `app-routing-locales.spec.ts` opens the view in other catalogs through
  `WARREN_E2E_LOCALE` (read by the mocked main under the same harness, and the
  only way to reach ar, fa and uk, which the language picker does not list),
  fails when a segment, a chip, a status line or a button is clipped, when the
  two segments differ in width, or when a right-to-left catalog does not
  mirror them, and screenshots each screen per locale.
  `APP_ROUTING_LOCALES=all` renders every catalog; run it after changing any
  App routing copy.

## 8. Never without the VPN (per-app lock)

A locked app reaches the network only through the Warren tunnel. It is never
outside the tunnel (section 1, rule 4), and while the tunnel does not carry it
(connecting, error, disconnected, a stopped daemon, a device that just booted)
it is blocked, every other app keeping whatever the state gives it. It is the
global lockdown mode narrowed to a list of apps.

| platform | mechanism | holds while no daemon runs | status |
|---|---|---|---|
| Windows | persistent WFP filters of their own (winfw `WinFw_SetLockedApps`) | yes, across a reboot | enforced, the real-engine tests run elevated in CI |
| Android | a VpnService interface that captures only the locked apps, with no pump | while Warren holds the VPN slot, from boot | enforced, validated on an emulator |
| Linux | nftables over a cgroup the app is opened in by Warren (`warren-include --locked`) | yes, until the app exits | enforced, the real-kernel tests run as root in a VM |
| macOS | needs a Network Extension content filter and a Developer ID build (section 8.5) | | not yet: the option is hidden |
| iOS | no per-app traffic identification outside MDM (section 8.5) | | not available |

The daemon refuses a lock (`AppRoutingError::LockUnavailable`,
`FAILED_PRECONDITION`) where the platform does not enforce one, so a list is
never saved that nothing holds.

### 8.1 Windows

`talpid-core/src/app_locks.rs` keeps the lock the daemon wants and puts it in
force through winfw; the daemon owns it and follows every tunnel state
transition (`follow_tunnel_with_app_locks` in `mullvad-daemon/src/lib.rs`).

- winfw installs, in a sublayer and under a provider of their own
  (`ProviderAppLocks`, `SublayerAppLocks`, salted per environment like every
  other key), a hard (`definitive`) block of the locked executables at
  `ALE_AUTH_CONNECT` and `ALE_AUTH_RECV_ACCEPT`, IPv4 and IPv6, for anything
  that is not loopback and not on the tunnel interface. Connected, the tunnel
  interface is the alias the connected policy is applied with; in any other
  state there is none and the apps get loopback only. With LAN sharing on, a
  permit of the LAN and multicast ranges weighs above the block in the same
  sublayer.
- The filters are persistent and belong to no policy. The purge winfw runs at
  initialization and teardown removes the providers `Provider` and
  `ProviderPersistent` only, so a daemon start, stop, crash or upgrade, and a
  reboot, leave the locks in force. `WinFw_ResetAllGenerations` (the
  uninstaller's `warren-setup reset-firewall`, and recovery) removes them with
  every other generation's objects; the startup sweep of foreign generations
  removes another environment's.
- A lock change is one WFP transaction: the old filters go and the new ones
  arrive together. An executable whose path does not resolve is skipped, and a
  list where none resolves installs nothing, since a filter with no app
  condition would match every app. An alias that no longer resolves (the
  adapter was removed since) leaves the apps blocked outside loopback.
- The daemon never locks its own executable, whose relay connection leaves
  outside the tunnel.
- Every change is put in force before the settings are saved, so a lock the
  firewall refuses is answered with an error and not saved.

Tests: `talpid-core/src/firewall/windows/winfw_tests.rs` (elevated, against
the real filtering engine: blocked with no tunnel and no policy, the LAN when
shared, the tunnel interface, an alias that no longer resolves, the lock
outliving winfw's initialization and teardown, recovery removing it, an
unresolvable app locking nothing), `talpid-core/src/app_locks.rs` (what is put
in force and when).

Residuals:
- The lock is by executable path, as every WFP app condition: a copy of the
  executable elsewhere, or a child process of another executable, is not held.
  WFP matches no process id, so holding the children of another program would
  mean holding that program everywhere; a copy is another program, which the
  user locks on its own.
- Name lookups go through the system resolver (the Dnscache service), which is
  not the app: while disconnected, a locked app's lookups reach the network's
  resolver, though no connection of the app follows them.

Validation end to end, 2026-10-07, the Windows 11 ARM64 VM (build 26100), a
native ARM64 debug build of `7c746f31f9` (daemon and winfw, beta) run as the
dev service, the test wallet, a copy of `curl.exe` at `C:\b1\lk` locked and
the system `curl.exe` as the control, `https://api.ipify.org` as the echo:

| check | observed |
|---|---|
| disconnected | the locked curl refused in 21 ms, the control on the VM's own address; with LAN sharing on, the locked curl reached the gateway |
| connected | both from the NL exit (50.7.46.90) |
| disconnected again | refused again |
| service stopped while connected | refused, the control on the VM's own address |
| reboot, service not started | still refused |
| another copy of the same `curl.exe` elsewhere | on the VM's own address (the path residual) |
| service started, disconnected | refused |
| lock removed, then set again | released at once, refused again at once |
| `warren-setup reset-firewall`, service stopped | released |
| service started after the reset | refused again: the daemon puts the saved lock back in force |

The first connect stayed blocked on "The split tunneling module reported an
error": the VM's settings held an exclusion from an earlier session and this
build had no split tunnel driver; split mode off, it connected.
`warren-setup reset-firewall` crashed in `combase.dll` (0xC0000005) after
removing the filters, on every run, the lock playing no part. It exited 0
with the Hyper-V step off (`TALPID_FIREWALL_BLOCK_HYPERV=0`), and exits 0 now
that the reset uses a WMI connection released before it returns instead of
the one its main thread kept until the process exited;
`mullvad-setup/tests/reset_firewall.rs` runs it in the elevated CI step.

### 8.2 Linux

Linux keeps no list here either (section 7): an app is locked when Warren opens
it, through "Open never without the VPN" on its route screen, which runs
`warren-include --locked`. The launcher joins
`/sys/fs/cgroup/warren-inclusions/warren-locked`, which the daemon creates
(`NftEnforcer` in `talpid-core/src/app_locks.rs`) and the launcher never does,
so a program asked to be locked never runs unlocked. Under the included cgroup,
include-only tunnels it too; its child processes inherit the cgroup.

- The lock is a table of its own, `<firewall id>-locks`
  (`set_app_lock` in `talpid-core/src/firewall/linux.rs`), which the policy
  table's reset leaves alone, and which stays while no daemon runs. Recovery
  (`reset_policy_all_generations`, the packages' `reset-firewall`) removes it.
- While no tunnel is up, its output chain accepts loopback (and the LAN when
  shared) for a socket of the locked cgroup and rejects the rest at once; its
  input chain drops the rest. While one is up it holds no rule: the policy then
  keeps every app that is not excluded in the tunnel, and the output hook could
  not judge an include-only packet once the mangle chain rerouted it, while
  `socket cgroupv2` is refused in postrouting.
- The daemon locks with no tunnel before its shutdown resets the policy
  (`finalize`), since the policy was what held them while connected.
- A Flatpak or Snap app starts in a cgroup of its own, so it cannot be opened
  locked: the option says so and is disabled.

Tests: `lock_tests` (the rules) and `lock_kernel_tests` (root, ignored by
default: this process in a cgroup two levels down is refused with no tunnel and
released by one; the lock outlives a policy reset and recovery removes it; a
process outside the cgroup is never held), run on 2026-10-07 in an Ubuntu 24.04
VM (kernel 6.8).

Validation end to end, 2026-10-07, the same VM (aarch64, debug build of
`7c746f31f9`, beta, the test wallet), `curl https://api.ipify.org` opened
through `warren-include --locked` against a plain `curl` as the control. The
packaged beta daemon of the VM was stopped for the run: two daemons of one
environment share the nft table names, and an unattended upgrade that
restarted it mid-run removed the test daemon's policy table.

| check | observed |
|---|---|
| disconnected | the locked curl refused in 3 ms (`Couldn't connect`), the control on the VM's own address; the locked app reached the LAN gateway with LAN sharing on |
| connected | both from the NL exit (50.7.46.90); the lock table held no rule |
| disconnected again | the locked curl refused again |
| daemon stopped while connected (SIGTERM) | the lock table stayed alone; the locked curl refused, the control on the VM's own address |
| daemon started again, disconnected | still refused |
| include-only, connected | the locked app and an app opened through `warren-include` both from the NL exit, the control on the VM's own address; two more connect cycles gave the same |
| include-only, disconnected | the locked app refused, the included one on the VM's own address |
| include-only, daemon stopped while connected | the locked app refused |
| `warren-setup reset-firewall` | the lock table gone, the locked app on the VM's own address |
| `warren app-routing lock add` | refused, `FailedPrecondition` "this build cannot lock an app to the VPN": Linux keeps no list |

The first include-only connect of the run, right after the policy table had
been removed under the daemon, gave the included and the locked apps a DNS
timeout while their traffic by address left from the exit; the cause was not
established, and the three later cycles resolved normally.

Residuals:
- A program already running may take the new window into its running process,
  which stays where it was (the route screen says to close it first, as for
  "Open through the VPN"); a program opened another way is not locked. A
  program that must always be locked can be started that way from its own
  desktop entry or autostart file (`warren-include --locked <program>`).
- Name lookups go through `systemd-resolved` where the distribution uses it,
  a process outside the cgroup: while disconnected, a locked app's lookups
  reach the network's resolver (measured: `getent ahosts` answered), though
  no connection of the app follows them.

### 8.3 Android

`WarrenQuinnAdapter` holds a third interface next to the tunnel and the kill
switch: the lock guard (`planLockGuard`), an allow list of the locked apps on
the device, every address family routed, no DNS, no pump. This app is not on
it, so its own API calls and every app that is not locked keep the network.

- It comes up wherever the traffic is handed back to the bare network: a user
  disconnect (`teardownLocked`), and a drop released without lockdown, an
  expiry, a ban (`releaseTraffic`). It is established before the interfaces it
  replaces close. It goes once the live interface is up on a connect, or the
  kill-switch blackhole is (which holds the locked apps too: every app, or an
  include-only list they are on).
- A system revoke (another VPN took the slot) and the service's destruction
  tear down without it: an interface established then would take the slot back
  or outlive its owner.
- The service stays in the foreground while it is up, and the disconnected
  notification counts the locked apps. It starts at boot
  (`LockedAppsBootCompletedReceiver`), when a lock is set with no tunnel
  (`KEY_HOLD_LOCKED_APPS_ACTION`), and whenever the service is created while
  this app is still the prepared VPN app (the UI binding it after the system
  killed the process).
- What the UI says follows the blackhole, never the list: its state reaches
  the connect screen and the notification (`WarrenLockGuardProvider`), so
  locked apps it does not hold (another VPN app took the slot) read "N locked
  apps can reach the Internet without the VPN", in the warning colour, rather
  than "blocked".
- An allow list with no app on the device would capture every app, so no
  locked app installed means no guard.
- "Allow LAN" does not open it, as it does not open the kill switch.

Validation, 2026-10-07, betaDebug of this branch on the `warren-test` emulator
(API 35, arm64), Chrome locked, FOSS Browser as the control, IP echo through
`api.ipify.org`:

| check | observed |
|---|---|
| Chrome locked, Warren disconnected | one VPN network, `Uids: <{10145-10145, 20145-20145}>` (Chrome's uid only); Chrome loaded nothing, the other browser showed the host's address; the connect screen read "1 app blocked until the VPN connects" |
| connect | the VPN network carried every uid; Chrome showed 50.7.46.90 (NL exit); the label went |
| disconnect | the guard back on Chrome's uid alone |
| reboot, no auto-connect | the guard up again before the app was opened; the service in the foreground, notification "Disconnected and unsecure" / "1 app blocked until the VPN connects" |
| lock lifted | no VPN network left |
| Warren force-stopped, then opened | no VPN network while stopped; on opening, the guard back on Chrome's uid alone |
| VPN consent withdrawn (`appops set ... ACTIVATE_VPN deny`), Warren force-stopped, then opened | no guard, and the connect screen read "1 locked app can reach the Internet without the VPN" |

Residuals: Android runs one VPN at a time, so turning another VPN app on, or
forcing Warren to stop, releases the locked apps; the route page says so.
A locked app traffic that a system service carries for it (a download through
`DownloadManager`, a push) belongs to that service, as in section 3.5.

### 8.4 What the user sees

The route page of an app carries a "Never without the VPN" switch under the
three routes, with what it does in one sentence. It cannot be turned on for an
app outside the VPN (it says to choose a route through the VPN first), and is
hidden on macOS. On Linux it is a launch action, "Open never without the VPN". Sending a locked app outside the VPN
lifts the lock first (`planAppRoute`), so the app never takes a third route on
the way. A locked app is a rule even on the default route; its chip ends with a
padlock, and while the VPN is not connected its line reads "Blocked until the
VPN connects". The main screen says how many apps wait for the VPN, and opens
App routing.

### 8.5 macOS and iOS

macOS has no per-app firewall a daemon can drive: pf matches no process, and
the split tunnel's classifier (section 3) only steers packets while the daemon
runs. The mechanism that holds is a content filter, an `NEFilterDataProvider`
shipped as a system extension inside the app. It sees every new flow with the
audit token of the process that opened it, so it can tell the app by its code
signature (firmer than the Windows path condition), and it runs whether the
daemon does or not. It would keep the lock list itself, drop a locked app's
flows unless the daemon reports a connected tunnel over XPC, and drop them
when it cannot reach the daemon.

What it needs before any code can ship, all of it on the Apple developer
account (the account holder's action):

- the Developer ID Application and Installer certificates, the same ones the
  split tunnel waits for (`docs/macos-signing.md`). On 2026-10-07 the build Mac
  had no code signing identity (`security find-identity -v -p codesigning`:
  0) and the repo no macOS signing secret, so release builds are signed ad hoc;
- App IDs for the app and the extension with the Network Extensions and System
  Extension capabilities, and a Developer ID provisioning profile for each,
  embedded in the bundle;
- the entitlements `com.apple.developer.networking.networkextension`
  (`content-filter-provider-systemextension`) on both and
  `com.apple.developer.system-extension.install` on the app.

On the user's Mac the app must run from `/Applications`, and the user approves
the extension in System Settings (Privacy and Security), then the filter
prompt; an MDM profile can approve both. These are Apple's documented
requirements, not yet tried on a Warren build.

iOS identifies an app's traffic only through per-app VPN or a content filter,
and Apple's documentation reserves both to supervised (MDM) devices, a content
filter otherwise needing the Family Controls entitlement Apple grants to
parental-control apps. Neither fits an App Store VPN, so iOS has no lock.
