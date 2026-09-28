# Upstream sync log

Every review of mullvad/mullvadvpn-app against this fork, newest first. The
policy is [`UPSTREAM-DETACH.md`](UPSTREAM-DETACH.md): no rebase, surgical
picks from the watch list (`scripts/dev/upstream-watch-paths.txt`) plus any
upstream security fix wherever it lands.

How a review runs: the `upstream-sync` skill (`.claude/skills/upstream-sync/`),
which drives `scripts/dev/upstream-triage.sh`. `.upstream-reviewed` holds the
last upstream commit reviewed; the next review starts after it.

Conventions:

- A picked commit carries `(upstream <sha10>)` at the end of its subject. The
  triage script finds picks that way, so keep the marker exact.
- Every watch-list commit of the range gets a verdict below. A commit that is
  not listed was not reviewed.
- **Open items** carry over: each review re-reads them and closes, keeps or
  drops each one, in the new entry.

## Open items

Carried from the latest review. Each is relevant to Warren and could not be
finished as a pick.

| upstream | what | why it is open |
|---|---|---|
| b5271f66c2, d76b53a840, and the binaries bumps d3f72f0a3d, c2c5b5e628 | Windows split tunnel: win-split-tunnel 1.3.0.0 takes its WFP sublayer GUIDs from the caller | Worth more to Warren than to Mullvad: Warren puts winfw in Mullvad's sublayers so the driver's filters land in them, and falls back to salted private sublayers (no split tunneling, include-only held back) when another product holds them. With 1.3.0.0 Warren could hand the driver its own GUIDs and coexist with Mullvad. Needs the `dist-assets/binaries` bump (it also drops the wireguard-nt DLLs), the IOCTL change, a rethink of `split_tunnel_sublayers_shared`, and a Windows run of split tunneling. A lot of its own; the driver ships for aarch64 too, so the ARM64 VM can test it. |
| none (Warren only) | macOS: the pf firewall's `Drop` lifts the block the state machine keeps for lockdown and for the restart an update arms, as it did on Linux | The Linux fix (`keep_policy_on_drop`) was not extended to macOS: `is_shutdown_user_initiated()` is always false there, so a kept block would also outlive the stop the uninstaller causes, and `uninstall_macos.sh --from-daemon` no longer resets pf. Needs the uninstall path settled first, then a run in a macOS VM (the tart VMs on this Mac are CI machines). |
| 9df9e5aedf | Windows: block Hyper-V guests while the tunnel is torn down | Ported (b8f390294e) and built by CI, not validated on a host with Hyper-V: the only Windows guest is ARM64 under QEMU on an M2, and the Hyper-V firewall needs nested virtualization, which Apple's hypervisor offers from M3 on (assumption from Apple's documentation, not measured). Validate on an x64 Windows 11 with WSL. |
| 02a8099325 | Android: reconnect after an app update when always-on VPN did not bring the service back | Upstream binds its service and lets its daemon restore the persisted target. Warren's Android service has no daemon and no persisted connect intent (`WarrenVpnService`), so binding would restore nothing; the port would be a stored intent plus a `KEY_CONNECT_ACTION` on `MY_PACKAGE_REPLACED`, as the boot receiver does. Not started: the symptom has to be reproduced first, and the only emulator was in use by another agent. |

## 2026-09-28 (second pass): the open items of the first review

No new upstream range; this pass worked the open items. Everything below was
validated where it runs, except where the Open items table says otherwise.

| upstream | here | outcome |
|---|---|---|
| d7944e34ab, f2282386a4 | 0799000538, f2e2cf98da | Picked, and it uncovered a Warren defect. Linux's `Firewall` gained a `Drop` in the fork that resets the policy, so the block the state machine keeps on shutdown (lockdown, the restart lock an update arms) was lifted a moment later, and the early-boot blocker lifted its own block as its process exited: every Linux boot ran unprotected from that exit to the daemon's start (3.7 s measured in the VM). Measured in an Ubuntu 24.04 VM across `systemctl restart` with the tunnel blocking: 1.1.35 as shipped, 538 clear-text egress successes over 19 s and the daemon back Disconnected; with both fixes and `RestartKillSignal=SIGUSR1`, 0 of 3,680 samples leaked and the daemon came back to its secured target. The sysvinit script restarts with USR1 too. |
| 1f170ae04c | 47341c9710 | Ported, with a test the upstream change lacked. |
| 80b14dd924, 63ad024026 | eb4926c679 | Picked as is. |
| b43b225318 | b38ba0e594, SDK 5b05eee, pin 2fa039f205 | Ported to both Warren clients that resumed: the daemon's reqwest clients and the SDK's `ReqwestTransport` (the SDK's marked transport and the engine's TLS already refused resumption). A preconfigured rustls config replaces reqwest's `tls_sni`, so SNI moved into the same config; tests check both against a local server that issues tickets. Found on the way: `api.beta.warrenbrowse.com` refuses TLS without SNI (alert `InternalError`), before and after the change, so the SDK's SNI-less fallback cannot succeed against it. |
| 9df9e5aedf | b8f390294e | Ported (Windows only); see Open items for what is not validated. |
| 12259737fa | none | Not applicable. The daemon binds the socket and chmods it afterwards; measured umask under systemd 0022, so the socket is `0755` in between and neither group nor others can connect (connecting needs write). launchd and sysvinit at 022 is an assumption, not measured. |

## 2026-09-28: baseline to upstream `0d06537e5f`

Range `440c97f36a` (`upstream-baseline-2026-05-06`, upstream main on
2026-05-05) to `0d06537e5f` (upstream main on 2026-09-28): 1,488 non-merge
commits, 62 on the watch list. First review since the detach. Reviewed by an
agent for poka.

Two findings about the method, both folded into the skill:

- The fork's import commit (`390e3b82ad`) already contains some upstream
  commits dated after the baseline, so "after the baseline" does not mean
  "missing". The triage script tests reverse application to catch them, and
  that test misses a commit whose code is here but whose context moved, so a
  CONFLICT still has to be checked against the code.
- Three of the upstream security fixes of the range landed outside the watch
  list of the time (cgroup, uninstaller, IPC pipe). The watch list now covers
  the privilege boundaries, and the review always reads the upstream
  changelogs' `### Security` entries.

### Picked

| upstream | subject | here | note |
|---|---|---|---|
| c78d774357 | Ignore cgroup path overrides when setuid (GHSA-6m4p-ggr4-5f76) | 5efe960a3c | Hardening: `warren-exclude` and `warren-include` already avoided the env overrides. Upstream's tests passed without the fix; rewritten so they fail without it. |
| 039ef15011 | Allow parsing of eslogger v11 output | ab7d41b208 | Split tunneling on macOS 27. |
| 2513dd7da6 | Extend DNS cache flush timeout | e35dafd026 | Warren flushes on every reset, so it hits this timeout more often than upstream. |
| 70fd624a00 | Fix potential LPE in macOS uninstaller | f897a2a47a | Ported by hand. Warren had the same flaw: the root uninstall ran `warren-setup` from `/Applications`. New test runs the script with a recording fake `sudo`. |
| e47c82ea29 | Fix missing check on pipeIsAdminOwned (GHSA-wchj-r66m-4m48) | 6215ca2303 | Ported by hand. Warren had the same flaw, and its GUI hands the wallet to that pipe. The check moved to `src/main/pipe-ownership.ts` to be testable. |

### Already present

| upstream | subject |
|---|---|
| b3440507fb, e8ab119af5, d4c459df5e | Split tunnel driver: log the load outcome, wait on a pending start, 2 minute timeout |
| 9aac4c0adf | `ct state established` on the custom DNS nft rule |
| b7fc39b2b3 | Typo in the `mullvad-exclude` error |
| ca1c2f298e | Clarify the "Core Foundation main loop exited" error |
| 1d0e5890c7 | rustls 0.23.45 (GHSA-2mjx-qc3c-rqvc), outside the watch list |

### Open

See **Open items** above: d7944e34ab, f2282386a4, 9df9e5aedf, 12259737fa,
b5271f66c2, d76b53a840, 1f170ae04c, 80b14dd924, 63ad024026, b43b225318,
02a8099325.

### Not taken

| upstream | subject | reason |
|---|---|---|
| 1154871955 | Do not flush DNS cache on DNS reset | Warren flushes on reset on purpose (`talpid-dns/src/windows/iphlpapi.rs`). |
| dacf3f7e58 | Remove `netsh`-based DNS configuration | Warren still ships and extends the netsh backend. |
| 1f4510e5ab, 0c04454d74, 47c1caec2e | Drop LAN traffic on the tunnel interface, then remove it again | Added and removed upstream within the range. What remains concerns WireGuard allowed IPs. |
| 1a9e096c62, e597b3c1dd, ab5d6b1584 | wireguard-go removal, WireGuard teardown logging, multiplexer routes | WireGuard and obfuscation code Warren does not run. |
| 55e12496f7, ff3d56a1e0, 7860488edd, b24d4cdcc5, 99249a4be3, df353c5c05 | DAITA, AllowedIps, log filter and settings-reset RPC changes in the management client | Mullvad settings model and RPC surface, not Warren's. |
| 1893399ace, a656a66713, 9b9b49d021 | `GrpcClient` refactors for the relay selector service | Refactor for a Mullvad feature. |
| 013a176dce | Move persistent block rules from WinFw to Rust | Large refactor, no behaviour fix. Revisit if a later Windows firewall fix depends on it. |
| 378ab24a2d, 9607684198 | systemd ordering: `After=network.target`, early-boot blocker before `network-pre.target` | Portability and boot-time ordering. Warren's `Before=basic.target` already orders the blocker ahead of network services. |
| 9ac15e5a31 | Nushell CLI completions | Feature, not a fix. |
| b13c70f77a, 46f53363fb, a0bd9c5d02, c0a93a2f8a | Lints of Rust 1.97, 1.98 and nightly | Warren pins 1.95.0. Take them with the toolchain bump if they fire. |
| e0fb4c0dd8, 7a57402979, 25df6e644d, 6cf0f7c09d, 66a5bb615e | Dependency swaps and bumps | Follow Warren's own dependency schedule. |
| ab3aa59c40, 7b1fbea5ee, e07de05258, f06b063909, 051a274c75, 4bc7c63bf1, 33abdd07cd, 565e9c4955 | Refactors (socket bypass, traceroute, resolv.conf, systemd-resolved, nft helper, docs) | No behaviour fix. |
| 964f75d77a, 8ca385997b, 1804b9a86c, 35ba672fde, 36f926828d | Formatting, manifest cleanup, spelling, import path | Churn. |
