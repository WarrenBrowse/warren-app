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
| 9df9e5aedf | Windows: block Hyper-V guests while the tunnel is torn down | Ported (b8f390294e) and built by CI, not validated on a host with Hyper-V: the only Windows guest is ARM64 under QEMU on an M2, and the Hyper-V firewall needs nested virtualization, which Apple's hypervisor offers from M3 on (assumption from Apple's documentation, not measured). Validate on an x64 Windows 11 with WSL. |
| none (Warren only) | Android: the always-on VPN advice is written and never shown | `Android16UpdateWarningReceiver` sets `show_always_on_vpn_advice`, and nothing reads it. Its input is also unreadable: an app cannot read `Settings.Secure.always_on_vpn_app`, so the verdict is always UNKNOWN. `VpnService.isAlwaysOn()`, recorded since d10ada89e3 as `always_on_vpn`, is the readable source; `isLockdownEnabled()` would give the second half. Needs a UI to show it. |
| none (inference) | A Warren build before 2ef0cf6ecf next to a Mullvad that loaded split tunnel driver 1.3.0.0 | That build only speaks the 1.2 initialize code, which driver 1.3.0.0 does not define (`src/defs/ioctl.h`), so its split tunneling fails to initialize there. Inferred from the driver source and from the measured converse (a 1.2 driver refuses the 1.3 code); not measured. Fixed from 2ef0cf6ecf on, whichever driver is loaded. |

## 2026-09-28 (third pass): the remaining open items

Every item was validated on the platform it runs on.

| upstream | here | outcome |
|---|---|---|
| none (Warren only) | c313340238 | macOS kept the kill switch the daemon leaves only in name: pf's `Drop` lifted it, as on Linux. Measured in a macOS 26 VM (a copy-on-write clone of `macos-01` with its forge services disabled): with lockdown on, `launchctl kickstart -k` let 52 clear-text connections out over 2 s with 1.1.40, 0 of 75 with the fix; an update restart (`prepare-restart`) held too, and came back to the secured target. Keeping the block needed the uninstall path first, and that turned up a shipped defect: the daemon ran the prod uninstall script whatever its environment, so deleting the Beta app deleted the prod app and its settings and left the beta daemon running and blocking (measured on 1.1.40: decoy prod bundle and `/etc/warren-vpn` gone, 42 pf rules, no egress). The daemon now disarms (target, lockdown, auto-connect) before running a script renamed for its own environment, held to `fixtures/uninstall-macos/` together with the GUI package's copy. Measured with the fix: nothing left behind, egress back, prod decoys intact. |
| 02a8099325 | d10ada89e3 | Upstream's symptom did not reproduce: with always-on VPN really enabled, the emulator (Android 15) restarted the tunnel 3 s after each of 3 updates. What did reproduce is the rest of it: without always-on, an update leaves a connected user disconnected. Warren now records that a tunnel was requested and whether the OS runs it as always-on VPN, read once connected (`isAlwaysOn()` answers false while the tunnel is being set up), and restarts it on `MY_PACKAGE_REPLACED` unless always-on will. Measured: back in 3 s after each update without always-on, a single start with it, nothing after a user disconnect. |
| b5271f66c2, d3f72f0a3d, c2c5b5e628 | 2ef0cf6ecf | Driver 1.3.0.0 shipped (it also fixes a BSOD on reset and a leak of uninitialized bytes to user space). The daemon hands it the sublayers winfw really uses, shared or private, and falls back to the 1.2 initialize code when the loaded driver does not know the new one: a beta update does not unload the driver, and a co-installed Mullvad loads its own. Measured in the Windows 11 ARM64 VM with the aarch64 drivers, the daemon blocking (no subscription) and one excluded `curl.exe`: 1.3 with the shared sublayers free, excluded out, the rest blocked; 1.3 with another product holding them, winfw private and split tunneling working, where it used to be refused; 1.2, fallback logged and working; 1.2 with another product holding them, refused as before, nothing out. d76b53a840 (Mullvad's new sublayer keys) not taken: Warren keeps the old shared keys for 1.2 drivers and hands 1.3 its own. |

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
