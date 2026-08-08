# Leak protection across an app update

What guarantees the user's traffic while Warren replaces itself, what happens
when that replacement fails, and the plan to close the gaps. Companion to
[`AUTO-UPDATE.md`](AUTO-UPDATE.md), which covers how an update is detected,
signed and delivered. Recovery procedures for a user already locked out live in
[`warren-lockout-recovery.md`](warren-lockout-recovery.md).

## The contract

An update replaces the privileged daemon, so there is a window where nothing
manages the tunnel. Three properties must hold across that window, and they pull
against each other:

1. **No leak.** A user who was connected must not egress in the clear between
   the old daemon exiting and the new one taking over.
2. **No brick.** A user whose install fails must regain internet without
   needing internet, without the app, and without a support call.
3. **No surprise.** A user who never enabled the kill switch must not be left
   with one after the update, and one who did must keep it.

Property 1 is armed today. Properties 2 and 3 hold only on the happy path.

## How it works today

The GUI never arms anything. `AppUpgrade.startInstaller`
(`desktop/packages/mullvad-vpn/src/main/app-upgrade.ts:125`) only spawns the
installer, and the arming is done by each installer's **preinstall** script,
which calls `warren-setup prepare-restart` against the still-running daemon:

| platform | script | on failure |
|---|---|---|
| macOS | `dist-assets/pkg-scripts/preinstall:26` | logged, install continues |
| Linux (systemd) | `dist-assets/linux/before-install.sh:6` | `\|\| true`, install continues |
| Linux (sysvinit) | `dist-assets/linux/sysvinit/preinst:15` | `\|\| true`, install continues |
| Windows | `dist-assets/windows/installer.nsh:1061` | **aborts the install** |

`prepare-restart` reaches `Daemon::on_prepare_restart`
(`mullvad-daemon/src/lib.rs:5018`), which does two things when the user's target
state is `Secured`:

- sends `TunnelCommand::LockdownMode(LockdownMode::yes().persist(persist))` to
  the tunnel state machine. This is an **in-memory** value
  (`shared_values.lockdown_mode`), never written to `settings.json`, so it is
  meant to die with the process;
- calls `target_state.lock()`, which keeps `target-start-state.json` on disk
  through shutdown so the new daemon reconnects on its own.

At shutdown the state machine keeps the blocking firewall in place when that
in-memory lockdown is set (`talpid-core/src/tunnel_state_machine/mod.rs:679`),
which is what closes the window. Measured on the 2026-08-08 incident: the host
was blocked from 11:54:29.468 to the new daemon taking over, no leak.

### `persist` is a Windows-only concept

`LockdownMode::should_persist()`
(`talpid-core/src/tunnel_state_machine/mod.rs:369`) returns the `persist` flag on
Windows and **unconditionally `true` everywhere else**, and `persist()` is a
no-op off Windows. The Windows arm of `on_prepare_restart` deliberately makes the
WFP filters non-persistent unless the user already opted into lockdown or
auto-connect, so a failed install is undone by a reboot. That reasoning is sound
on Windows because WFP filters survive a reboot. It was never ported because pf
(macOS) and nftables (Linux) rules are runtime-only.

The conclusion drawn from that ("so a reboot always frees a Mac or a Linux box")
is true, and it is also the entire safety net on those two platforms.

### Where the escape hatches are

- The app's launch screen offers **Restore internet without VPN**, which runs
  `warren-setup reset-firewall` behind the OS elevation prompt, when the daemon
  cannot be contacted.
- `warren unblock` (`mullvad-cli/src/cmds/unblock.rs`) does the same from a
  terminal, escalating to the privileged helper only when needed.
- A reboot clears the runtime rules on all three desktop platforms for a user
  who did not enable lockdown.

## The gaps

Six, ordered by how likely a user is to hit them.

**G1. A successful update can still block the host.** Not a leak-protection
defect, and the one that actually bit: the boot connect races the multi-hop
circuit publication and fails closed on `NoCircuit`, with no retry. Full chain in
`incidents/2026-08-08-auto-update-restart-nocircuit-race-blocked-host.md`. Every
daemon restart with target state `Secured` is exposed, so the update path hits it
most often.

**G2. No dead-man on the arming.** Nothing ever disarms a `prepare-restart`
lockdown except the daemon dying. Known since
`incidents/2026-07-13-phantom-lockdown-after-pkg-install.md`, whose follow-up
names exactly this. Two distinct failures live here:

- the daemon survives the preinstall (a manually launched daemon, a failed
  `launchctl unload`, a service the installer could not stop). It then runs for
  hours with lockdown armed while the GUI, the CLI and `settings.json` all
  report it off, and every disconnect blocks the host;
- the daemon dies as intended but the install never completes, so no daemon ever
  comes back to reset the firewall.

**G3. On macOS the recovery tools are deleted before the risky part.** The
preinstall runs `rm -rf "$INSTALL_DIR/Warren VPN.app"` (line 37) **after** arming
the lockdown, and `warren-setup` plus the GUI's Restore-internet screen live
inside that bundle. Between that `rm -rf` and the payload extraction the machine
is blocked with no local means to unblock it other than a reboot.

**G4. The macOS postinstall can abort after the point of no return.** It exits
non-zero on a `.localized` collision (line 29) and on a non-root-owned bundle
(line 35), both **before** `launchctl load -w` (line 121). The old daemon is gone,
the firewall is blocking, and no daemon is loaded.

**G5. `prepare-restart` failure is non-fatal on macOS and Linux.** Windows aborts
the install when the daemon does not acknowledge; the other three scripts log and
continue. A daemon that ignored the command is then killed with the firewall
in whatever state it had, which is the leak the arming exists to prevent.

**G6. Mobile is outside this mechanism entirely.** Android replaces the package,
which kills the `VpnService` and drops the tunnel; iOS updates through the App
Store. Neither app can arm a kill switch across its own replacement, because on
both platforms that is an OS setting the app may request but not set
(Android: Always-on VPN plus "Block connections without VPN"; iOS: on-demand
rules). Android already ships an `ACTION_MY_PACKAGE_REPLACED` receiver
(`Android16UpdateWarningReceiver`), so the hook exists.

## Plan

Six lots. Lot 0 comes first because without it the next occurrence cannot be
diagnosed. Lot 1 is independent of the rest. Lots 2 to 4 are the leak protection
proper and share one primitive. Lot 5 is mobile.

### Lot 0: be able to see the failure at all [DONE]

Two defects found while diagnosing the 2026-08-08 second occurrence, both of
which make every later lot harder to validate.

- **Preserve the post-update daemon's log.** The daemon rotates to
  `daemon.old.log` at every start and the postinstall copies the **replaced**
  daemon's log to `old-install-daemon.log`, so nothing preserves the daemon that
  came up after the install. Two restarts erased the window that mattered. Give
  the first post-update start its own retained copy, or widen rotation depth
  around an update.
- **Stop the boot stalling on a DNS lookup the kill switch blocks.**
  `ApiEndpoint::address` (`mullvad-api/src/lib.rs:284`) resolves with
  `ToSocketAddrs::to_socket_addrs`, a synchronous blocking lookup, called from
  the async `AddressCache::resolved_or_persisted`. In a blocked state it can only
  time out: measured 30.006 s of stalled boot on this machine, with the tokio
  runtime held. Consult the persisted address first, or move the lookup off the
  runtime with a bounded timeout.
- **Say which lockdown is armed.** `Persistent lockdown is enabled ...` is
  printed for the ten-second lockdown an installer arms, on a machine whose
  setting is off. Distinguish the two in the message, so a lock-out log states
  which one it is.

### Lot 1: an update that succeeds must not block the host (G1) [DONE]

Shipped in `c4f70d5572`, host tests only, still to be validated on a real
upgrade. The two parts as built:

- **The circuit is seeded before the boot connect.** `boot_seed` reads and
  verifies the on-disk signed directory and selects a circuit synchronously, and
  `Daemon::start` pushes it onto the generator before `run()` can dial. The
  verification is the code that already ran inside the updater task, moved ahead
  of the race: the task no longer reads the file, it adopts the seed through
  `UpdaterConfig.boot_seed`, and adopting the seeded circuit as its
  `last_circuit` is what stops its first pass requesting a reconnect the
  generator does not need.
- **A blocking start failure retries.** `reschedules_reconnect` replaces the
  inline `AuthFailed` check with an explicit cause matrix, and `StartTunnelError`
  joins it on the same 60 s timer. `WarrenTunnelFlapping` and
  `WarrenPubkeyMismatch` are excluded by name (both exist to stop retrying), and
  `IsOffline` stays with the connectivity edge that already revives it.
  Unbounded in count on purpose: bounding it leaves a user whose circuit arrives
  late permanently dark, which is the failure being fixed, and one dial per
  minute against a cancelable blocked state is the cheaper side of that trade.

This also downgrades the no-cache boot race that `2026-08-04` recorded: a machine
with no usable cache still dials before any circuit exists, but now recovers
within a minute instead of never.

### Lot 2: the in-process dead-man (G2, first half) [DONE]

`on_prepare_restart` arms a timer alongside the lockdown. If no shutdown follows
within a bounded window, the daemon reverts `shared_values.lockdown_mode` to the
value in `settings.json` and logs it loudly. Cheap, entirely inside the daemon,
and it closes the phantom-lockdown class for good: a daemon that outlives its own
installer stops lying about its state.

The window has to exceed a slow installer on a slow disk. Measured on the
2026-08-08 macOS upgrade: preinstall at 11:54:29, new daemon up at 11:54:39, so
10 s end to end. A window of a few minutes is two orders of margin and still
bounds the damage.

### Lot 3: the detached dead-man (G2 second half, G3, G4) [DONE, needs real-install validation]

The workspace rule is explicit and was bought by an incident: a dead-man must
never depend on the thing it is protecting against. The in-process timer dies
with the daemon, so it cannot cover a daemon that died with the install
unfinished.

Before it arms the lockdown, each preinstall stages a self-contained guard
outside the install directory, so `rm -rf` of the bundle cannot take it (G3):

- copy the `warren-setup` binary to a staging path owned by root;
- register a one-shot OS timer for a bounded delay (launchd `StartInterval` job
  on macOS, systemd transient timer on Linux, scheduled task on Windows);
- the timer's action checks whether a daemon is answering on the management
  socket. If one is, it removes itself and does nothing. If none is, it runs
  `reset-firewall` and removes itself.

Each postinstall disarms the guard on success. The guard is idempotent, removes
itself in every path, and its verdict rests on "is a daemon managing this
machine", which is precisely the condition that decides whether a blocking
firewall has an owner.

This also covers G4 without touching the postinstall's two aborts: they stay
fatal, and the guard restores the user regardless.

### Lot 4: align the three desktop platforms on the Windows caution (G5) [DONE, needs real-install validation]

- Make a failed `prepare-restart` fatal on macOS and Linux, matching the Windows
  arm. An installer that cannot arm the protection must not proceed to kill the
  daemon.
- Give macOS and Linux a real `persist` semantic instead of the unconditional
  `true`. The Windows rule is the right one on every platform: persist the block
  across the update only for a user who chose lockdown or auto-connect;
  otherwise arm it for the window and let it die with the daemon. This is
  exactly the "re-disable it afterwards" property, made explicit rather than
  inherited from the process dying.

### Lot 5: mobile (G6) [DONE, Android detection + advice; iOS documented as out of reach]

Neither app can arm the protection, so the honest deliverable is detection and
guidance, wired into surfaces that already exist:

- **Android.** On `ACTION_MY_PACKAGE_REPLACED`, read whether the OS has
  Always-on VPN with "Block connections without VPN" for this app and surface a
  one-time notice when it does not, pointing at the system settings page. The
  receiver already exists.
- **iOS.** Nothing to arm and nothing to detect across a store update. Document
  it here rather than leaving the table's "planned" implying otherwise.

Do not synthesize a userspace kill switch on mobile. Both platforms tear down
the tunnel on replacement and neither grants an app the authority to hold the
network closed.

## Ordering and risk

Lot 1 is independent and carries no lock-out risk, so it ships first.

Lots 3 and 4 touch the kill switch on three platforms and cannot be validated by
unit tests alone: each needs a real install, a real failed install, and a reboot,
on each platform. The Parallels Windows and Linux VMs plus this Mac cover all
three. Validate a deliberately broken install on every platform before any of it
reaches a user, and validate the disarm path on a machine where the user never
enabled lockdown.

Lot 4's fatal `prepare-restart` is the one change that can make an install fail
where it used to proceed. It is the correct trade (an install that cannot protect
the user should not start), and it is why lot 3 lands first: the guard must exist
before the abort does.

## Shipped state, 2026-08-08

Every lot is implemented and on `main`. What each rests on:

| lot | commits | validation |
|---|---|---|
| 0 | `34dd1469cb`, `f6257e3770` | host tests; the 30 s boot stall is measured, the rotation depth is asserted |
| 1 | `c4f70d5572` | host tests |
| 2 | `6c65c91f37` | host tests, the verdict proven to go red on reverted code |
| 3 | `4275b55e85` | host tests for the verdict and the staging path; **the OS timers are NOT exercised by any test** |
| 4 | `4275b55e85` | shell syntax only |
| 5 | this commit | host tests for the verdict; the `Settings.Secure` read is not exercised |

**Lots 3 and 4 still need what this document always said they need**: a real
install, a deliberately broken install, and a reboot, on macOS, Linux and
Windows. Nothing below that proves a scheduled task actually fires, that
`launchctl` accepts the plist, or that an aborted install leaves a machine that
recovers. The unit tests pin the decisions, not the platform integration.

The one change that can make a previously-succeeding install fail is lot 4's
fatal `prepare-restart`. It ships behind lot 3's guard, which is the ordering
this document required, so a machine that aborts still has something armed to
free it.
