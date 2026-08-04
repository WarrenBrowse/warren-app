//! CLI subcommands to control Warren NAT-PMP port forwarding on a
//! headless host: enable/disable the feature, manage the per-rule
//! forward list (multi-port), tune the requested mapping lifetime, and
//! observe the granted public port (one-shot or streamed). Wraps the
//! daemon's GetNatPmpSettings / SetNatPmpSettings / NatPmpStatusUpdates
//! RPCs, so it is the GUI-less equivalent of the Electron port-forward
//! panel.

use anyhow::{Result, bail};
use clap::{Subcommand, ValueEnum};
use futures::StreamExt;
use mullvad_management_interface::{
    MullvadProxyClient,
    types::{
        NatPmpSettings,
        nat_pmp_settings::{Proto, Rule},
        nat_pmp_status::{Mapping, State},
    },
};

/// Transport protocol of a forward rule: UDP, TCP, or both at once on
/// the same external port (an atomic pair). A rule's identity is
/// `(protocol, internal_port)`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Udp,
    Tcp,
    Both,
}

impl Protocol {
    /// The prost enum discriminant the wire message stores.
    fn as_proto_i32(self) -> i32 {
        match self {
            Protocol::Udp => Proto::Udp as i32,
            Protocol::Tcp => Proto::Tcp as i32,
            Protocol::Both => Proto::Both as i32,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum PortForward {
    /// Show the persisted port-forwarding settings: the ON/OFF toggle,
    /// the requested lifetime and the configured rule list.
    Get,

    /// Show the live mapping status: the granted public port per rule.
    Status {
        /// Keep watching and print every status transition until
        /// interrupted (Ctrl-C), instead of a single snapshot.
        #[arg(long, short = 'w')]
        watch: bool,
    },

    /// Turn port forwarding ON. With `--internal-port` it also adds (or
    /// updates) one rule in the same step; without it, the feature is
    /// enabled against the rules already configured.
    Enable {
        /// Internal port your application listens on locally. When set,
        /// a rule for it is added/updated before enabling.
        #[arg(long)]
        internal_port: Option<u16>,

        /// Transport protocol for the rule added via `--internal-port`.
        #[arg(long, value_enum, default_value_t = Protocol::Both)]
        protocol: Protocol,

        /// Suggested public port (0 = let the exit pick from its pool).
        /// The exit honours an explicit request or errors if it is taken.
        #[arg(long, default_value_t = 0)]
        external_port: u16,

        /// Requested mapping lifetime in seconds (the exit clamps to
        /// [60, 3600]). Omit to keep the current value.
        #[arg(long)]
        lifetime: Option<u32>,
    },

    /// Turn port forwarding OFF. Keeps the configured rules so a later
    /// `enable` restores them.
    Disable,

    /// Add or update one forward rule (identified by protocol +
    /// internal port). Does not change the ON/OFF toggle.
    Add {
        /// Internal port your application listens on locally.
        #[arg(long)]
        internal_port: u16,

        /// Transport protocol.
        #[arg(long, value_enum, default_value_t = Protocol::Both)]
        protocol: Protocol,

        /// Suggested public port (0 = let the exit pick from its pool).
        #[arg(long, default_value_t = 0)]
        external_port: u16,
    },

    /// Remove one forward rule (identified by protocol + internal port).
    Remove {
        /// Internal port of the rule to drop.
        #[arg(long)]
        internal_port: u16,

        /// Transport protocol of the rule to drop.
        #[arg(long, value_enum, default_value_t = Protocol::Both)]
        protocol: Protocol,
    },

    /// Set the requested mapping lifetime in seconds (exit clamps to
    /// [60, 3600]).
    Lifetime {
        #[arg(value_parser = clap::value_parser!(u32).range(1..))]
        secs: u32,
    },
}

impl PortForward {
    pub async fn handle(self) -> Result<()> {
        match self {
            PortForward::Get => Self::get().await,
            PortForward::Status { watch } => Self::status(watch).await,
            PortForward::Enable {
                internal_port,
                protocol,
                external_port,
                lifetime,
            } => Self::enable(internal_port, protocol, external_port, lifetime).await,
            PortForward::Disable => Self::disable().await,
            PortForward::Add {
                internal_port,
                protocol,
                external_port,
            } => Self::add(protocol, internal_port, external_port).await,
            PortForward::Remove {
                internal_port,
                protocol,
            } => Self::remove(protocol, internal_port).await,
            PortForward::Lifetime { secs } => Self::lifetime(secs).await,
        }
    }

    async fn get() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_nat_pmp_settings().await?;
        print_settings(&settings);
        println!("(run `warren port-forward status` for the granted public port)");
        Ok(())
    }

    async fn status(watch: bool) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        // The daemon backs this stream with a `watch` channel, so the
        // first item is always the current snapshot: a single `next()`
        // gives the one-shot view without a separate query RPC.
        let mut stream = rpc.nat_pmp_status_updates().await?;
        if watch {
            println!("Watching NAT-PMP status (Ctrl-C to stop)...");
        }
        while let Some(status) = stream.next().await {
            print_mappings(&status?.mappings);
            if !watch {
                break;
            }
        }
        Ok(())
    }

    async fn enable(
        internal_port: Option<u16>,
        protocol: Protocol,
        external_port: u16,
        lifetime: Option<u32>,
    ) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let mut settings = rpc.get_nat_pmp_settings().await?;
        settings.enabled = true;
        if let Some(secs) = lifetime {
            settings.lifetime_secs = secs;
        }
        if let Some(internal_port) = internal_port {
            upsert_rule(
                &mut settings.rules,
                make_rule(protocol, internal_port, external_port),
            );
        }
        if settings.rules.is_empty() {
            bail!(
                "No forward rule configured: pass --internal-port (e.g. \
                 `warren port-forward enable --internal-port 51820`) or add one \
                 with `warren port-forward add` first."
            );
        }
        rpc.set_nat_pmp_settings(settings).await?;
        println!("Port forwarding enabled.");
        Ok(())
    }

    async fn disable() -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let mut settings = rpc.get_nat_pmp_settings().await?;
        settings.enabled = false;
        rpc.set_nat_pmp_settings(settings).await?;
        println!("Port forwarding disabled (rules kept).");
        Ok(())
    }

    async fn add(protocol: Protocol, internal_port: u16, external_port: u16) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let mut settings = rpc.get_nat_pmp_settings().await?;
        upsert_rule(
            &mut settings.rules,
            make_rule(protocol, internal_port, external_port),
        );
        rpc.set_nat_pmp_settings(settings).await?;
        println!("Rule added: {protocol:?} internal {internal_port} -> external {external_port}.");
        Ok(())
    }

    async fn remove(protocol: Protocol, internal_port: u16) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let mut settings = rpc.get_nat_pmp_settings().await?;
        if !remove_rule(&mut settings.rules, protocol, internal_port) {
            bail!("No matching rule ({protocol:?} internal port {internal_port}).");
        }
        rpc.set_nat_pmp_settings(settings).await?;
        println!("Rule removed: {protocol:?} internal {internal_port}.");
        Ok(())
    }

    async fn lifetime(secs: u32) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let mut settings = rpc.get_nat_pmp_settings().await?;
        settings.lifetime_secs = secs;
        rpc.set_nat_pmp_settings(settings).await?;
        println!("Requested lifetime set to {secs}s (exit clamps to [60, 3600]).");
        Ok(())
    }
}

/// Build a wire `Rule` from the CLI-facing types. Ports widen to `u32`
/// because the proto field is 32-bit even though valid ports are 16-bit.
fn make_rule(protocol: Protocol, internal_port: u16, external_port: u16) -> Rule {
    Rule {
        protocol: protocol.as_proto_i32(),
        suggested_external_port: u32::from(external_port),
        internal_port: u32::from(internal_port),
    }
}

/// Insert `rule`, or, when one with the same identity
/// `(protocol, internal_port)` already exists, overwrite its suggested
/// external port. Keeps the rule list a set keyed by that identity so a
/// repeated `add` updates in place rather than creating a duplicate the
/// exit would reject against the per-client quota.
fn upsert_rule(rules: &mut Vec<Rule>, rule: Rule) {
    match rules
        .iter_mut()
        .find(|r| r.protocol == rule.protocol && r.internal_port == rule.internal_port)
    {
        Some(existing) => existing.suggested_external_port = rule.suggested_external_port,
        None => rules.push(rule),
    }
}

/// Drop the rule with identity `(protocol, internal_port)`. Returns
/// whether a rule was actually removed, so the caller can report a
/// no-op instead of silently succeeding.
fn remove_rule(rules: &mut Vec<Rule>, protocol: Protocol, internal_port: u16) -> bool {
    let proto = protocol.as_proto_i32();
    let internal_port = u32::from(internal_port);
    let before = rules.len();
    rules.retain(|r| !(r.protocol == proto && r.internal_port == internal_port));
    rules.len() != before
}

/// Human label for a stored protocol discriminant; `?` if the daemon
/// ever sends an unknown value (forward-compatibility guard).
fn proto_label(proto: i32) -> &'static str {
    match Proto::try_from(proto) {
        Ok(Proto::Udp) => "UDP",
        Ok(Proto::Tcp) => "TCP",
        Ok(Proto::Both) => "TCP+UDP",
        Err(_) => "?",
    }
}

/// One human-readable line for a per-rule mapping, surfacing the public
/// port when MAPPED and the reason otherwise (the differentiating
/// information a headless operator needs).
fn mapping_line(m: &Mapping) -> String {
    let head = format!("{}/{}", m.internal_port, proto_label(m.protocol));
    match State::try_from(m.state) {
        Ok(State::Mapped) => match m.external_port {
            Some(port) => format!("{head}: MAPPED, public port {port}"),
            None => format!("{head}: MAPPED"),
        },
        Ok(State::Requesting) => format!("{head}: requesting..."),
        Ok(State::Disabled) => format!("{head}: disabled"),
        Ok(State::RateLimited) => match m.retry_after_secs {
            Some(secs) => format!("{head}: rate-limited, retry in {secs}s"),
            None => format!("{head}: rate-limited"),
        },
        Ok(State::Failed) => match &m.error_message {
            Some(msg) => format!("{head}: failed ({msg})"),
            None => format!("{head}: failed"),
        },
        Err(_) => format!("{head}: unknown state"),
    }
}

/// Print one line per live mapping, or an explicit empty notice so the
/// operator can tell "no rules / feature off" from a missed update.
fn print_mappings(mappings: &[Mapping]) {
    if mappings.is_empty() {
        println!("No active mappings (port forwarding off or no rules).");
        return;
    }
    for m in mappings {
        println!("{}", mapping_line(m));
    }
}

/// Print the toggle, requested lifetime and the configured rule list.
fn print_settings(settings: &NatPmpSettings) {
    println!(
        "Port forwarding: {}",
        if settings.enabled { "on" } else { "off" }
    );
    println!("Requested lifetime: {}s", settings.lifetime_secs);
    if settings.rules.is_empty() {
        println!("Rules: none");
        return;
    }
    println!("Rules:");
    for r in &settings.rules {
        let external = if r.suggested_external_port == 0 {
            "auto".to_owned()
        } else {
            r.suggested_external_port.to_string()
        };
        println!(
            "  {}/{} internal {} -> external {external}",
            r.internal_port,
            proto_label(r.protocol),
            r.internal_port
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(proto: Protocol, internal: u16, external: u16) -> Rule {
        make_rule(proto, internal, external)
    }

    #[test]
    fn upsert_appends_distinct_rules() {
        let mut rules = vec![rule(Protocol::Udp, 51820, 0)];
        upsert_rule(&mut rules, rule(Protocol::Tcp, 51820, 0));
        upsert_rule(&mut rules, rule(Protocol::Udp, 8080, 0));
        assert_eq!(rules.len(), 3, "distinct identities must coexist");
    }

    #[test]
    fn upsert_replaces_external_port_for_same_identity() {
        let mut rules = vec![rule(Protocol::Udp, 51820, 1000)];
        upsert_rule(&mut rules, rule(Protocol::Udp, 51820, 2000));
        assert_eq!(rules.len(), 1, "same identity must not duplicate");
        assert_eq!(rules[0].suggested_external_port, 2000);
    }

    #[test]
    fn remove_rule_reports_hit_and_miss() {
        let mut rules = vec![rule(Protocol::Udp, 51820, 0), rule(Protocol::Tcp, 80, 0)];
        assert!(remove_rule(&mut rules, Protocol::Udp, 51820));
        assert_eq!(rules.len(), 1);
        assert!(
            !remove_rule(&mut rules, Protocol::Udp, 51820),
            "already gone"
        );
        // A protocol mismatch on the same port must not remove anything.
        assert!(!remove_rule(&mut rules, Protocol::Udp, 80));
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn mapping_line_shows_public_port_when_mapped() {
        let m = Mapping {
            internal_port: 51820,
            protocol: Proto::Udp as i32,
            state: State::Mapped as i32,
            external_port: Some(49152),
            ..Default::default()
        };
        assert_eq!(mapping_line(&m), "51820/UDP: MAPPED, public port 49152");
    }

    #[test]
    fn mapping_line_surfaces_failure_reason() {
        let m = Mapping {
            internal_port: 80,
            protocol: Proto::Tcp as i32,
            state: State::Failed as i32,
            error_message: Some("port in use".to_owned()),
            ..Default::default()
        };
        assert_eq!(mapping_line(&m), "80/TCP: failed (port in use)");
    }
}
