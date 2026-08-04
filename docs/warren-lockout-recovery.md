# Recovering from a kill-switch lock-out (Warren)

This document describes how to recover network connectivity when the
Warren kill-switch ("lockdown mode") has blocked all traffic and the block
persists, including across reboots.

## Symptom

- No internet access at all, even with the app stopped.
- The daemon reports a blocked error state, typically:
  `Blocked: Failure to select a matching tunnel relay`.
- The block **survives a reboot**.

## Cause

The kill-switch is designed to fail **closed**: when lockdown mode is on, it
blocks all non-tunnel traffic so nothing leaks while you are disconnected.

A lock-out happens when **all three** of these hold at once:

1. **Lockdown mode is on** (`block when disconnected` / kill-switch).
2. **Auto-connect is on**, so the daemon tries to (re)connect on boot.
3. The daemon **cannot select a usable relay** (e.g. the relay is
   unreachable, marked inactive, or the relay list is empty).

The connection attempt fails, the daemon enters the blocked error state, and
because lockdown is on it keeps all traffic blocked. The early-boot blocking
service plus the persisted configuration re-apply the block on every boot, so
the device stays offline until lockdown is turned off.

## Recovery

**No-CLI path (preferred): the app itself.** If the daemon is alive (the
common case: the blocked error state IS a live daemon), open the app and
click **Disconnect**, or turn lockdown mode off in Settings. If the daemon
is dead and cannot restart, the launch screen ("Unable to contact the
Warren system service") offers **Restore internet without VPN**: it runs
`warren-setup reset-firewall` behind the OS elevation prompt (UAC / macOS
administrator / polkit), which removes the firewall block AND repairs DNS
left pointing at a dead in-tunnel resolver. For a non-lockdown user a plain
reboot also clears the block on all three platforms (the runtime rules do
not survive it).

CLI equivalent, does **not** require internet access:

```sh
warren lockdown-mode set off
```

Then restart the daemon (or reconnect):

```sh
# Linux (systemd)
sudo systemctl restart warren-daemon.service
```

With lockdown off, the block no longer re-arms on boot: after the restart
the daemon comes up disconnected with the firewall open. You can re-enable
lockdown once a working connection is confirmed:

```sh
warren lockdown-mode set on
```

## Notes for safe testing

Before running any test that could prevent the tunnel from connecting (relay
maintenance, transport changes, offline benches), disarm the fail-closed
first so you cannot lock yourself out:

```sh
warren auto-connect set off && warren lockdown-mode set off
```

Re-enable both only after a stable connection has been validated.

## Status / mitigations

- The error state (tunnel failed while the user wanted it up) **always
  blocks**, lockdown or not: that is the kill switch working as designed,
  and the daemon is alive there, so one Disconnect click always restores
  traffic. Lockdown only governs the deliberately disconnected state.
- A daemon **crash** is fail-closed too (the kernel firewall rules outlive
  the process, and a caught panic exits without resetting them) and
  self-healing: the supervisor restarts the daemon, which boots blocked and
  reconnects on its own. systemd retries forever (`StartLimitIntervalSec=0`),
  like launchd and the Windows SCM.
- For the residual case (a daemon that cannot stay up at all), the launch
  screen's **Restore internet without VPN** button and, for non-lockdown
  users, a reboot both clear the block without any CLI.
- A deliberate stop (`systemctl stop`, quit, uninstall) still resets the
  firewall unless lockdown is armed: "daemon not running" implies "traffic
  allowed" for users who never opted into a persistent kill switch.
- The historical "tunnel mode" toggle that could leave the only POC relay
  unusable has been removed (Warren is the only mode).
- A relay-selection failure can still arm the fail-closed when lockdown is
  on; the recovery above always applies.
