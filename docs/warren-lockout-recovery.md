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

The fix is a **local CLI command**, it does **not** require internet access:

```sh
warren lockdown-mode set off
```

Then restart the daemon (or reconnect):

```sh
# Linux (systemd)
sudo systemctl restart warren-daemon.service
```

With lockdown off, a failed/unavailable relay no longer cuts all traffic: the
daemon resets the firewall and lets normal traffic through while
disconnected. You can re-enable lockdown once a working connection is
confirmed:

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

- With lockdown **off**, the daemon already resets the firewall on transient
  errors (missing relay, offline, etc.) instead of cutting all traffic.
- The historical "tunnel mode" toggle that could leave the only POC relay
  unusable has been removed (Warren is the only mode).
- A relay-selection failure can still arm the fail-closed when lockdown is
  on; the recovery above always applies.
