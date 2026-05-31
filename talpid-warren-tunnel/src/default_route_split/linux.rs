//! Policy routing that pushes Internet traffic through the Warren TUN
//! while preserving the Warren client's UDP socket connectivity to the
//! exit (or to the multi-hop entry relay).
//!
//! ## Why not `talpid_routing::RequiredRoute`
//!
//! `RequiredRoute` exposes `use_main_table(bool)` but no custom
//! `table_id` (only the `main` table or a talpid-internal one). We need
//! a **dedicated table 100** plus an `ip rule` so the exit IP bypass
//! wins on priority - `RequiredRoute` cannot express this.
//!
//! We work around it by shelling out to `ip` after `talpid_routing` has
//! installed its own routes. Same pattern as `warren-client`.
//!
//! ## Platforms
//!
//! Linux only. The parent module gates the import on
//! `target_os = "linux"` and re-exports `DefaultRouteSplitGuard`.

use std::net::Ipv4Addr;

use anyhow::{Context, Result, anyhow};
use tokio::process::Command;

/// Dedicated routing table number. Distinct from the classic fwmark
/// policy routing to avoid conflicts if both features are active.
const ROUTE_TABLE: u32 = 100;

/// Bypass exit IP priority (= evaluated FIRST so it wins over the
/// `lookup 100` rule).
const RULE_PREF_EXIT_BYPASS: u32 = 50;

/// Split-default via tun priority (= evaluated AFTER the exit bypass).
const RULE_PREF_TUN: u32 = 51;

/// Excluded-traffic priority (= evaluated BEFORE the `lookup 100` rule so
/// packets from `warren-exclude`d processes leave via the physical
/// interface instead of the TUN).
///
/// Without this rule the split-tunnel mark is applied by the firewall but
/// no policy route sends the marked packets to the `main` table; they fall
/// through to `RULE_PREF_TUN` (`lookup 100`), hit the TUN, and are dropped
/// by the firewall's "block marked in-tunnel traffic" rule → black-holed.
const RULE_PREF_EXCLUDE: u32 = 49;

/// Firewall mark that the split-tunnel nftables rules apply (as packet
/// `meta mark`) to traffic from excluded processes. Mirrors
/// `mullvad_types::TUNNEL_FWMARK` (kept as a local literal to avoid adding
/// a `mullvad-types` dependency edge to this crate). Expressed in hex so it
/// matches the `ip rule show` display form.
const SPLIT_TUNNEL_FWMARK: &str = "0x6d6f6c65";

/// Builds the list of `ip` commands to execute for install. Pure
/// (= testable without a Linux kernel). Returns `Vec<Vec<String>>`
/// where each inner vec is the args to `ip`.
#[must_use]
pub fn build_install_commands(exit_ip: Ipv4Addr, tun_name: &str) -> Vec<Vec<String>> {
    vec![
        vec![
            "route".into(),
            "add".into(),
            "0.0.0.0/1".into(),
            "dev".into(),
            tun_name.into(),
            "table".into(),
            ROUTE_TABLE.to_string(),
        ],
        vec![
            "route".into(),
            "add".into(),
            "128.0.0.0/1".into(),
            "dev".into(),
            tun_name.into(),
            "table".into(),
            ROUTE_TABLE.to_string(),
        ],
        vec![
            "rule".into(),
            "add".into(),
            "to".into(),
            format!("{exit_ip}/32"),
            "lookup".into(),
            "main".into(),
            "pref".into(),
            RULE_PREF_EXIT_BYPASS.to_string(),
        ],
        vec![
            "rule".into(),
            "add".into(),
            "lookup".into(),
            ROUTE_TABLE.to_string(),
            "pref".into(),
            RULE_PREF_TUN.to_string(),
        ],
        // Excluded (split-tunnel) traffic: the firewall tags it with
        // `SPLIT_TUNNEL_FWMARK`; route those packets via the `main` table so
        // they egress the physical interface (where the firewall masquerades
        // them) instead of being routed into the TUN and dropped.
        vec![
            "rule".into(),
            "add".into(),
            "fwmark".into(),
            SPLIT_TUNNEL_FWMARK.into(),
            "lookup".into(),
            "main".into(),
            "pref".into(),
            RULE_PREF_EXCLUDE.to_string(),
        ],
    ]
}

/// Builds the list of `ip` commands for uninstall (inverse order).
#[must_use]
pub fn build_uninstall_commands(exit_ip: Ipv4Addr) -> Vec<Vec<String>> {
    vec![
        vec![
            "rule".into(),
            "del".into(),
            "lookup".into(),
            ROUTE_TABLE.to_string(),
            "pref".into(),
            RULE_PREF_TUN.to_string(),
        ],
        vec![
            "rule".into(),
            "del".into(),
            "to".into(),
            format!("{exit_ip}/32"),
            "lookup".into(),
            "main".into(),
            "pref".into(),
            RULE_PREF_EXIT_BYPASS.to_string(),
        ],
        vec![
            "rule".into(),
            "del".into(),
            "fwmark".into(),
            SPLIT_TUNNEL_FWMARK.into(),
            "lookup".into(),
            "main".into(),
            "pref".into(),
            RULE_PREF_EXCLUDE.to_string(),
        ],
        vec![
            "route".into(),
            "flush".into(),
            "table".into(),
            ROUTE_TABLE.to_string(),
        ],
    ]
}

/// Minimal TUN name validation (= shell injection protection even
/// when passing via `Command::new`). 1-15 alphanum chars + `-`/`_`.
fn validate_tun_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 15 {
        return Err(anyhow!("tun_name length must be 1-15 chars"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!(
            "tun_name must contain only ASCII alphanum, '-' or '_'"
        ));
    }
    Ok(())
}

/// RAII guard: holds the "installed" state for automatic cleanup on drop.
#[derive(Debug)]
pub struct DefaultRouteSplitGuard {
    exit_ip: Ipv4Addr,
    installed: bool,
}

impl DefaultRouteSplitGuard {
    /// Installs the split-default policy routing for `tun_name` with
    /// `exit_ip` bypass. Idempotent (= "File exists" tolerated).
    ///
    /// # Errors
    ///
    /// - invalid `tun_name`
    /// - missing privileges (CAP_NET_ADMIN required)
    /// - `ip` not in PATH
    pub async fn install(exit_ip: Ipv4Addr, tun_name: &str) -> Result<Self> {
        validate_tun_name(tun_name).context("invalid tun_name")?;

        // Diagnostic: log ip rule + table 100 state BEFORE install to
        // detect any pre-existing rules (e.g. talpid_routing
        // posting its own netlink rules with SuppressPrefixLen + fwmark).
        if let Ok(out) = Command::new("ip").args(["rule", "show"]).output().await {
            log::debug!(
                "ip rule (pre-install): {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }

        let cmds = build_install_commands(exit_ip, tun_name);
        for args in &cmds {
            run_ip_tolerant_exists(args)
                .await
                .with_context(|| format!("ip {}", args.join(" ")))?;
        }

        // Diagnostic: log ip rule + table 100 state AFTER install.
        if let Ok(out) = Command::new("ip").args(["rule", "show"]).output().await {
            log::debug!(
                "ip rule (post-install): {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
        if let Ok(out) = Command::new("ip")
            .args(["route", "show", "table", "100"])
            .output()
            .await
        {
            log::debug!(
                "ip route table 100 (post-install): {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }

        log::info!(
            "Warren default-route split-tunnel installed: \
             tun={tun_name} exit={exit_ip} table={ROUTE_TABLE}"
        );

        Ok(Self {
            exit_ip,
            installed: true,
        })
    }

    /// Removes the rules. Idempotent: tolerates "No such file" if
    /// already removed. Best-effort: logs warn but does not fail.
    pub async fn uninstall(mut self) -> Result<()> {
        let cmds = build_uninstall_commands(self.exit_ip);
        for args in &cmds {
            if let Err(e) = run_ip_tolerant_no_such(args).await {
                log::warn!("ip cleanup failed (non-fatal): {e}");
            }
        }
        self.installed = false;
        Ok(())
    }
}

impl Drop for DefaultRouteSplitGuard {
    fn drop(&mut self) {
        if self.installed {
            log::warn!(
                "DefaultRouteSplitGuard dropped without explicit uninstall \
                 (exit_ip={}); routes may persist",
                self.exit_ip
            );
        }
    }
}

async fn run_ip_tolerant_exists(args: &[String]) -> Result<()> {
    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = Command::new("ip")
        .args(&str_args)
        .output()
        .await
        .with_context(|| format!("spawn ip {}", args.join(" ")))?;
    if out.status.success() {
        log::debug!("ip {} OK", args.join(" "));
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("File exists") {
        log::debug!(
            "ip {} returned 'File exists' (tolerated as idempotent)",
            args.join(" ")
        );
        return Ok(());
    }
    Err(anyhow!("ip {} failed: {}", args.join(" "), stderr.trim()))
}

async fn run_ip_tolerant_no_such(args: &[String]) -> Result<()> {
    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = Command::new("ip")
        .args(&str_args)
        .output()
        .await
        .with_context(|| format!("spawn ip {}", args.join(" ")))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("No such") || stderr.contains("not found") || stderr.contains("does not") {
        return Ok(());
    }
    Err(anyhow!("ip {} failed: {}", args.join(" "), stderr.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn install_commands_count_is_5() {
        let cmds = build_install_commands(ip("91.99.122.154"), "tun0");
        assert_eq!(
            cmds.len(),
            5,
            "expected 5 commands (2 routes + 3 rules: exit-bypass, tun, exclude)"
        );
    }

    #[test]
    fn install_first_two_commands_are_split_default_routes() {
        let cmds = build_install_commands(ip("91.99.122.154"), "tun0");
        assert_eq!(
            cmds[0],
            vec!["route", "add", "0.0.0.0/1", "dev", "tun0", "table", "100"]
        );
        assert_eq!(
            cmds[1],
            vec!["route", "add", "128.0.0.0/1", "dev", "tun0", "table", "100"]
        );
    }

    #[test]
    fn install_third_command_is_exit_bypass_rule() {
        // Anti-regression: if anyone removes this bypass, the
        // tunnel poisons itself (= routing loop cli -> tun -> exit).
        let cmds = build_install_commands(ip("91.99.122.154"), "tun0");
        assert_eq!(
            cmds[2],
            vec![
                "rule",
                "add",
                "to",
                "91.99.122.154/32",
                "lookup",
                "main",
                "pref",
                "50"
            ]
        );
    }

    #[test]
    fn install_fourth_command_is_tun_lookup_rule() {
        let cmds = build_install_commands(ip("91.99.122.154"), "tun0");
        assert_eq!(cmds[3], vec!["rule", "add", "lookup", "100", "pref", "51"]);
    }

    #[test]
    fn install_fifth_command_is_exclude_rule() {
        // Anti-regression for F1 (split tunnel black-hole): excluded
        // traffic is marked by the firewall but, without this rule, has no
        // policy route to the physical interface and gets dropped in-tunnel.
        // It MUST be evaluated before the `lookup 100` rule (pref 49 < 51).
        let cmds = build_install_commands(ip("91.99.122.154"), "tun0");
        assert_eq!(
            cmds[4],
            vec![
                "rule",
                "add",
                "fwmark",
                "0x6d6f6c65",
                "lookup",
                "main",
                "pref",
                "49"
            ]
        );
    }

    #[test]
    fn exclude_rule_pref_is_lower_than_tun_lookup() {
        // The exclude rule must win over the catch-all `lookup 100` rule.
        assert!(
            RULE_PREF_EXCLUDE < RULE_PREF_TUN,
            "excluded traffic must be matched before the TUN catch-all"
        );
    }

    #[test]
    fn uninstall_uses_inverse_order() {
        let cmds = build_uninstall_commands(ip("91.99.122.154"));
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[0][0], "rule", "first cleanup must be rule del");
        assert_eq!(cmds[1][0], "rule", "second cleanup must be rule del");
        assert_eq!(cmds[2][0], "rule", "third cleanup must be rule del");
        assert_eq!(cmds[3][0], "route", "last cleanup must be route flush");
    }

    #[test]
    fn uninstall_removes_exclude_rule() {
        let cmds = build_uninstall_commands(ip("91.99.122.154"));
        assert!(
            cmds.iter().any(|c| c
                == &vec![
                    "rule", "del", "fwmark", "0x6d6f6c65", "lookup", "main", "pref", "49"
                ]),
            "uninstall must remove the split-tunnel exclude rule"
        );
    }

    #[test]
    fn uninstall_flushes_dedicated_table_not_main() {
        let cmds = build_uninstall_commands(ip("91.99.122.154"));
        let flush_cmd = cmds.last().expect("uninstall has commands");
        assert_eq!(flush_cmd[0], "route", "last cleanup must be route flush");
        assert!(flush_cmd.contains(&"100".to_string()));
        assert!(
            !flush_cmd.contains(&"main".to_string()),
            "must NOT flush table main (= would destroy default route eth0)"
        );
    }

    #[test]
    fn install_uses_supplied_exit_ip() {
        let cmds = build_install_commands(ip("203.0.113.42"), "tun0");
        assert!(cmds[2].contains(&"203.0.113.42/32".to_string()));
    }

    #[test]
    fn install_uses_supplied_tun_name() {
        let cmds = build_install_commands(ip("1.2.3.4"), "warren-tun0");
        assert!(cmds[0].contains(&"warren-tun0".to_string()));
        assert!(cmds[1].contains(&"warren-tun0".to_string()));
    }

    #[test]
    fn validate_tun_name_accepts_valid() {
        assert!(validate_tun_name("tun0").is_ok());
        assert!(validate_tun_name("warren-tun0").is_ok());
        assert!(validate_tun_name("a_b").is_ok());
    }

    #[test]
    fn validate_tun_name_rejects_invalid() {
        assert!(validate_tun_name("").is_err());
        assert!(validate_tun_name(&"a".repeat(16)).is_err());
        assert!(validate_tun_name("evil; rm -rf").is_err());
        assert!(validate_tun_name("with space").is_err());
    }
}
