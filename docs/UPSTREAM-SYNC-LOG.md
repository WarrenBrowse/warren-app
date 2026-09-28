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
taken as a mechanical pick.

| upstream | what | why it is open |
|---|---|---|
| d7944e34ab, f2282386a4 | Linux: `SIGUSR1` makes the daemon stop without tearing the firewall down, and the unit sets `RestartKillSignal=SIGUSR1`, so `systemctl restart` does not leak | Warren has neither (`dist-assets/linux/warren-daemon.service`). Daemon change plus unit change, to validate on a real Linux host (egress watched during a restart). Warren's `DaemonCommand::PrepareRestart` already exists. |
| 9df9e5aedf | Windows: block Hyper-V traffic while the tunnel is disconnecting | Port the Windows half only. Upstream also adds a `Disconnecting` policy that blocks everything on Linux and macOS; whether that would drop the QUIC close Warren sends on teardown has not been checked. Needs the Windows VM. Warren's `FirewallPolicy` also carries `lan_networks`, which upstream's variant lacks. |
| 12259737fa | GHSA-p9rr-wc9m-qmwg: the management socket existed with loose permissions before the group restriction was applied | Upstream's fix does not apply: Warren binds the socket with std and chmods it afterwards (`mullvad-management-interface/src/lib.rs`, `apply_socket_permissions`), and `RpcGate` authorizes every RPC against the caller's uid. Residual: between `bind` and `chmod` the socket has the process umask's permissions. Decide whether to bind under a restrictive umask. |
| b5271f66c2, d76b53a840 | Windows split tunnel: pass the WFP sublayer GUIDs to the driver, new GUIDs to avoid clashing with other software | Needs upstream's newer `mullvad-split-tunnel.sys`. Warren already salts its own sublayer GUIDs (`windows/winfw/src/winfw/mullvadguids.cpp`); how the two interact is unchecked. |
| 1f170ae04c | macOS split tunnel: throttle the "failed to parse eslogger message" log spam | Low value, conflicts with Warren's rewrite of `split_tunnel/macos/process.rs`. Port by hand if the spam shows up in problem reports. |
| 80b14dd924, 63ad024026 | Deterministic Windows PE timestamps, no absolute PDB paths | Reproducible builds. Outside the watch list; worth a decision of its own. |
| b43b225318 | TLS session tickets disabled in every API client, against tracking across connections | Concerns Warren's API client, which lives in warren-sdk-rs, not here. To raise there. |
| 02a8099325 | Android: bind the VPN service after an app update so always-on reconnects | Warren's Android app was rewritten without the daemon. Check whether it shows the same symptom before porting anything. |

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
