//! CLI subcommands to control Warren NAT-PMP port forwarding on a
//! headless host: enable/disable the feature, manage the per-rule
//! forward list (multi-port), tune the requested mapping lifetime, and
//! observe the granted public port (one-shot, streamed, or awaited).
//! Wraps the daemon's GetNatPmpSettings / SetNatPmpSettings /
//! NatPmpStatusUpdates RPCs, so it is the GUI-less equivalent of the
//! Electron port-forward panel.
//!
//! The observation side is scriptable without reading human text:
//! `--json` prints one frozen JSON object per snapshot, `--wait` blocks
//! until every rule holds a grant and leaves with a verdict exit code,
//! and `--exec` runs a command whenever a granted public port changes.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use futures::StreamExt;
use mullvad_management_interface::{
    Error as ManagementError, MullvadProxyClient,
    types::{
        NatPmpSettings, NatPmpStatus,
        nat_pmp_settings::{Proto, Rule},
        nat_pmp_status::{ErrorReason, Mapping, State},
    },
};
use serde::Serialize;
use std::{io::Write as _, time::Duration};

/// Exit codes of `port-forward status --wait`, so a wrapper script can
/// branch on the verdict without reading any output.
const WAIT_EXIT_NOTHING_TO_WAIT_FOR: i32 = 1;
const WAIT_EXIT_TIMED_OUT: i32 = 2;
const WAIT_EXIT_FAILED: i32 = 3;

/// How long an `--exec` hook may run before it is killed. It runs between
/// two snapshots of the watch, so a hook that never returns would stall
/// every later port change.
const HOOK_TIMEOUT: Duration = Duration::from_secs(30);

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

/// Options of `port-forward status`. Its three modes are exclusive by
/// construction: one shot, `--watch` (optionally with `--exec`), or
/// `--wait` (optionally with `--timeout`).
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Keep watching and print every status transition until
    /// interrupted (Ctrl-C), instead of a single snapshot.
    #[arg(long, short = 'w')]
    watch: bool,

    /// Print the status as one JSON object on one line instead of the
    /// human lines. With --watch the output is NDJSON: one object per
    /// snapshot, flushed as it happens.
    #[arg(long)]
    json: bool,

    /// Block until every rule holds a granted public port, then print
    /// the snapshot and exit 0. Exit 1 when there is no rule to wait for
    /// (port forwarding off, or no rule configured), exit 2 when
    /// --timeout expires, exit 3 when a rule failed.
    #[arg(long, conflicts_with = "watch")]
    wait: bool,

    /// Give up waiting after this many seconds and exit 2 (only with
    /// --wait).
    #[arg(long, requires = "wait", value_name = "SECS")]
    timeout: Option<u64>,

    /// Run this command through the shell every time a rule's granted
    /// public port changes: a first grant, a move to another port, or
    /// the loss of a grant (only with --watch). A renewal that keeps the
    /// same port does not run it. `{{PORT}}` in the command is replaced
    /// by the new public port (empty on a loss), and the child is given
    /// WARREN_PF_PORT, WARREN_PF_INTERNAL_PORT, WARREN_PF_PROTOCOL and
    /// WARREN_PF_STATE (mapped|lost). The command is killed after 30s;
    /// a failure is reported on stderr and never stops the watch.
    #[arg(long, requires = "watch", value_name = "COMMAND")]
    exec: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum PortForward {
    /// Show the persisted port-forwarding settings: the ON/OFF toggle,
    /// the requested lifetime and the configured rule list.
    Get {
        /// Print the settings as one JSON object on one line instead of
        /// the human lines.
        #[arg(long)]
        json: bool,
    },

    /// Show the live mapping status: the granted public port per rule.
    Status {
        #[command(flatten)]
        args: StatusArgs,
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
            PortForward::Get { json } => Self::get(json).await,
            PortForward::Status { args } => Self::status(args).await,
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

    async fn get(json: bool) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        let settings = rpc.get_nat_pmp_settings().await?;
        if json {
            println!("{}", settings_json(&settings)?);
            return Ok(());
        }
        print_settings(&settings);
        println!("(run `warren port-forward status` for the granted public port)");
        Ok(())
    }

    async fn status(args: StatusArgs) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        // The daemon backs this stream with a `watch` channel, so the
        // first item is always the current snapshot: a single `next()`
        // gives the one-shot view without a separate query RPC.
        let mut stream = rpc.nat_pmp_status_updates().await?;
        if args.wait {
            return Self::wait_for_grant(&mut stream, &args).await;
        }
        if args.watch && !args.json {
            println!("Watching NAT-PMP status (Ctrl-C to stop)...");
        }
        // The first snapshot is compared against an empty previous one,
        // so a watcher that starts on an already granted port still runs
        // the hook once: a script started after the mapping exists must
        // learn the port it is meant to announce.
        let mut previous: Vec<Mapping> = Vec::new();
        while let Some(status) = stream.next().await {
            let mappings = status?.mappings;
            print_status(&mappings, args.json)?;
            if !args.watch {
                break;
            }
            if let Some(command) = &args.exec {
                for change in port_changes(&previous, &mappings) {
                    run_hook(command, &change, HOOK_TIMEOUT).await;
                }
            }
            previous = mappings;
        }
        Ok(())
    }

    /// Read snapshots until every rule holds a grant, then leave with the
    /// verdict's exit code. Generic over the stream so the RPC type does
    /// not have to be spelled out here.
    async fn wait_for_grant<S>(stream: &mut S, args: &StatusArgs) -> Result<()>
    where
        S: futures::Stream<Item = std::result::Result<NatPmpStatus, ManagementError>> + Unpin,
    {
        // One absolute deadline for the whole wait: a per-snapshot
        // timeout would never expire while the exit keeps sending
        // "requesting".
        let deadline = args
            .timeout
            .map(|secs| tokio::time::Instant::now() + Duration::from_secs(secs));
        let mut last: Vec<Mapping> = Vec::new();
        loop {
            let item = match deadline {
                Some(deadline) => match tokio::time::timeout_at(deadline, stream.next()).await {
                    Ok(item) => item,
                    Err(_) => {
                        print_status(&last, args.json)?;
                        std::process::exit(WAIT_EXIT_TIMED_OUT);
                    }
                },
                None => stream.next().await,
            };
            let Some(status) = item else {
                bail!("The daemon closed the NAT-PMP status stream.");
            };
            let mappings = status?.mappings;
            match wait_verdict(&mappings) {
                WaitVerdict::Done => {
                    print_status(&mappings, args.json)?;
                    return Ok(());
                }
                WaitVerdict::Failed => {
                    print_status(&mappings, args.json)?;
                    std::process::exit(WAIT_EXIT_FAILED);
                }
                WaitVerdict::Nothing => {
                    print_status(&mappings, args.json)?;
                    std::process::exit(WAIT_EXIT_NOTHING_TO_WAIT_FOR);
                }
                WaitVerdict::Keep => last = mappings,
            }
        }
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

/// One indented line describing a configured rule, as `port-forward get`
/// lists it.
///
/// The shape is a parsed interface: the Warren container's
/// `parse_forward_rules` (`warren-cli/docker/entrypoint.sh`) reads these
/// lines to find the rules it must clear before installing its own. It
/// selects a line by the ` internal ` and ` -> external ` separators and
/// reads the head back as `<internal_port>/<PROTO>`. A cosmetic edit here
/// strands stale rules in every container, so `settings_rule_line_*` pins
/// the whole line byte for byte.
fn settings_rule_line(r: &Rule) -> String {
    let external = if r.suggested_external_port == 0 {
        "auto".to_owned()
    } else {
        r.suggested_external_port.to_string()
    };
    format!(
        "  {}/{} internal {} -> external {external}",
        r.internal_port,
        proto_label(r.protocol),
        r.internal_port
    )
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
        println!("{}", settings_rule_line(r));
    }
}

/// Machine-readable protocol label: the same three words the `--protocol`
/// option takes, so a script can feed a JSON field straight back to the
/// CLI. `unknown` guards a discriminant a newer daemon might send.
fn proto_json_label(proto: i32) -> &'static str {
    match Proto::try_from(proto) {
        Ok(Proto::Udp) => "udp",
        Ok(Proto::Tcp) => "tcp",
        Ok(Proto::Both) => "both",
        Err(_) => "unknown",
    }
}

/// Machine-readable mapping state. Lower snake case so the values can be
/// compared in a shell without quoting rules.
fn state_json_label(state: i32) -> &'static str {
    match State::try_from(state) {
        Ok(State::Mapped) => "mapped",
        Ok(State::Requesting) => "requesting",
        Ok(State::Disabled) => "disabled",
        Ok(State::RateLimited) => "rate_limited",
        Ok(State::Failed) => "failed",
        Err(_) => "unknown",
    }
}

/// Machine-readable failure category, the stable key a script branches
/// on instead of matching the free-form `error_message`.
fn error_reason_json_label(reason: i32) -> &'static str {
    match ErrorReason::try_from(reason) {
        Ok(ErrorReason::SuggestedPortInUse) => "suggested_port_in_use",
        Ok(ErrorReason::OutOfResources) => "out_of_resources",
        Ok(ErrorReason::NotAuthorized) => "not_authorized",
        Ok(ErrorReason::Unknown) | Err(_) => "unknown",
    }
}

/// JSON view of the persisted settings.
///
/// A local struct rather than a `Serialize` on the prost type: this JSON
/// is a consumed contract (a `jq` line in someone's torrent-client
/// wrapper), while the proto is free to grow, rename or reorder fields.
/// `get_json_pins_the_settings_shape` pins the exact string.
#[derive(Serialize)]
struct SettingsJson<'a> {
    enabled: bool,
    lifetime_secs: u32,
    rules: Vec<RuleJson<'a>>,
}

#[derive(Serialize)]
struct RuleJson<'a> {
    protocol: &'a str,
    internal_port: u32,
    suggested_external_port: u32,
}

/// JSON view of one status snapshot.
///
/// Every optional field is serialized as `null` rather than omitted, so
/// a consumer can read `.mappings[0].external_port` without first
/// testing that the key exists.
#[derive(Serialize)]
struct StatusJson<'a> {
    mappings: Vec<MappingJson<'a>>,
}

#[derive(Serialize)]
struct MappingJson<'a> {
    internal_port: u32,
    protocol: &'a str,
    state: &'a str,
    external_port: Option<u32>,
    lifetime_granted_secs: Option<u32>,
    error_reason: Option<&'a str>,
    error_message: Option<&'a str>,
    retry_after_secs: Option<u32>,
    attempts_remaining: Option<u32>,
    window_reset_secs: Option<u32>,
}

/// The one-line JSON object `port-forward get --json` prints.
fn settings_json(settings: &NatPmpSettings) -> Result<String> {
    let view = SettingsJson {
        enabled: settings.enabled,
        lifetime_secs: settings.lifetime_secs,
        rules: settings
            .rules
            .iter()
            .map(|r| RuleJson {
                protocol: proto_json_label(r.protocol),
                internal_port: r.internal_port,
                suggested_external_port: r.suggested_external_port,
            })
            .collect(),
    };
    serde_json::to_string(&view).context("Failed to format the port-forward settings as JSON")
}

/// The one-line JSON object `port-forward status --json` prints, once
/// per snapshot.
fn status_json(mappings: &[Mapping]) -> Result<String> {
    let view = StatusJson {
        mappings: mappings
            .iter()
            .map(|m| MappingJson {
                internal_port: m.internal_port,
                protocol: proto_json_label(m.protocol),
                state: state_json_label(m.state),
                external_port: m.external_port,
                lifetime_granted_secs: m.lifetime_granted_secs,
                error_reason: m.error_reason.map(error_reason_json_label),
                error_message: m.error_message.as_deref(),
                retry_after_secs: m.retry_after_secs,
                attempts_remaining: m.attempts_remaining,
                window_reset_secs: m.window_reset_secs,
            })
            .collect(),
    };
    serde_json::to_string(&view).context("Failed to format the NAT-PMP status as JSON")
}

/// Print one status snapshot in the mode the caller asked for. JSON mode
/// flushes, so a `--watch --json` consumer reading the pipe sees each
/// snapshot when it happens rather than when the buffer fills.
fn print_status(mappings: &[Mapping], json: bool) -> Result<()> {
    if !json {
        print_mappings(mappings);
        return Ok(());
    }
    println!("{}", status_json(mappings)?);
    std::io::stdout()
        .flush()
        .context("Failed to flush the JSON status line")
}

/// What a `--wait` caller must do with one snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitVerdict {
    /// Every rule holds a grant: print it and leave with 0.
    Done,
    /// Still in flight: read the next snapshot.
    Keep,
    /// At least one rule failed: print it and leave with 3, so a script
    /// reports the refusal instead of blocking on it.
    Failed,
    /// No rule at all, so no grant can ever arrive: print it and leave
    /// with 1 rather than block forever on a feature that is off.
    Nothing,
}

/// Decide a `--wait` from one snapshot. `requesting` and `rate_limited`
/// are both transient (the daemon retries on its own), so they keep the
/// wait alive and only `--timeout` ends it.
fn wait_verdict(mappings: &[Mapping]) -> WaitVerdict {
    if mappings.is_empty() {
        return WaitVerdict::Nothing;
    }
    if mappings.iter().any(|m| m.state == State::Failed as i32) {
        return WaitVerdict::Failed;
    }
    if mappings.iter().all(|m| m.state == State::Mapped as i32) {
        return WaitVerdict::Done;
    }
    WaitVerdict::Keep
}

/// A rule whose granted public port is not what it was on the previous
/// snapshot. `port` is `None` when the grant was lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PortChange {
    internal_port: u32,
    protocol: i32,
    port: Option<u32>,
}

/// The public port a mapping currently holds, or `None` when it holds
/// none. Only MAPPED counts: a port carried on any other state is stale,
/// and a hook fired for it would point an application at a port the exit
/// no longer forwards.
fn granted_port(m: &Mapping) -> Option<u32> {
    if m.state == State::Mapped as i32 {
        m.external_port
    } else {
        None
    }
}

/// The rules whose grant changed between two snapshots, in snapshot
/// order. A renewal that keeps the same port produces nothing: the hook
/// exists to tell an application that its port MOVED, and firing on
/// every renewal would restart a torrent client twice an hour for
/// nothing.
///
/// A rule that disappears from the snapshot after holding a grant is as
/// much a loss as one that leaves MAPPED in place, so those come last,
/// after the rules the new snapshot still carries.
fn port_changes(previous: &[Mapping], next: &[Mapping]) -> Vec<PortChange> {
    let same_rule =
        |a: &Mapping, b: &Mapping| a.internal_port == b.internal_port && a.protocol == b.protocol;
    let mut changes: Vec<PortChange> = next
        .iter()
        .filter_map(|m| {
            let port = granted_port(m);
            let was = previous
                .iter()
                .find(|p| same_rule(p, m))
                .and_then(granted_port);
            (port != was).then_some(PortChange {
                internal_port: m.internal_port,
                protocol: m.protocol,
                port,
            })
        })
        .collect();
    changes.extend(
        previous
            .iter()
            .filter(|p| granted_port(p).is_some() && !next.iter().any(|m| same_rule(m, p)))
            .map(|p| PortChange {
                internal_port: p.internal_port,
                protocol: p.protocol,
                port: None,
            }),
    );
    changes
}

/// What running one hook produced. Returned rather than logged only, so
/// the runner can be tested without reading stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookOutcome {
    Succeeded,
    Failed,
    SpawnFailed,
    TimedOut,
}

/// Secrets a hook must never inherit. A child gets this process's
/// environment, so an operator who exported the recovery phrase would
/// hand it to every hook command and to everything that command spawns,
/// where it is readable from the process table.
const SECRET_ENV: [&str; 2] = ["WARREN_MNEMONIC", "WARREN_MNEMONIC_FILE"];

/// Build the child for one hook invocation: the operator's line through
/// the platform's own shell, `{{PORT}}` substituted, plus the variables
/// a script reads instead of parsing the command line it was given.
fn hook_command(command: &str, change: &PortChange) -> tokio::process::Command {
    let port = change.port.map_or_else(String::new, |p| p.to_string());
    let resolved = command.replace("{{PORT}}", &port);
    // A hook is an operator-written command line, so it runs under the
    // platform's own shell rather than being parsed here; without this
    // the feature is dead on Windows, which has no `sh`.
    #[cfg(unix)]
    let mut child = {
        let mut child = tokio::process::Command::new("sh");
        child.arg("-c").arg(&resolved);
        child
    };
    #[cfg(not(unix))]
    let mut child = {
        let mut child = tokio::process::Command::new("cmd");
        child.arg("/C").arg(&resolved);
        child
    };
    child
        .env("WARREN_PF_PORT", &port)
        .env("WARREN_PF_INTERNAL_PORT", change.internal_port.to_string())
        .env("WARREN_PF_PROTOCOL", proto_json_label(change.protocol))
        .env(
            "WARREN_PF_STATE",
            if change.port.is_some() {
                "mapped"
            } else {
                "lost"
            },
        );
    for name in SECRET_ENV {
        child.env_remove(name);
    }
    child
}

/// Run one hook invocation. Never returns an error: a broken hook must
/// not stop the watch that feeds it, so every outcome is reported on
/// stderr and the caller carries on.
async fn run_hook(command: &str, change: &PortChange, timeout: Duration) -> HookOutcome {
    let mut child = match hook_command(command, change).spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("port-forward hook failed to spawn: {err}");
            return HookOutcome::SpawnFailed;
        }
    };
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) if status.success() => HookOutcome::Succeeded,
        Ok(Ok(status)) => {
            eprintln!("port-forward hook exited with {status}");
            HookOutcome::Failed
        }
        Ok(Err(err)) => {
            eprintln!("port-forward hook could not be waited on: {err}");
            HookOutcome::SpawnFailed
        }
        Err(_) => {
            // The shell alone, on purpose: it stays in this process's
            // group, so Ctrl-C in the terminal still reaches the whole
            // watch, hook included.
            let _ = child.kill().await;
            eprintln!("port-forward hook exceeded {timeout:?} and was killed");
            HookOutcome::TimedOut
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(proto: Protocol, internal: u16, external: u16) -> Rule {
        make_rule(proto, internal, external)
    }

    fn mapped(internal_port: u32, protocol: Proto, external_port: u32) -> Mapping {
        Mapping {
            internal_port,
            protocol: protocol as i32,
            state: State::Mapped as i32,
            external_port: Some(external_port),
            ..Default::default()
        }
    }

    fn in_state(internal_port: u32, protocol: Proto, state: State) -> Mapping {
        Mapping {
            internal_port,
            protocol: protocol as i32,
            state: state as i32,
            ..Default::default()
        }
    }

    fn change_of(internal_port: u32, protocol: Proto, port: Option<u32>) -> PortChange {
        PortChange {
            internal_port,
            protocol: protocol as i32,
            port,
        }
    }

    /// A hook that outlives any sane timeout. `sleep` is not a `cmd`
    /// builtin, so Windows gets the usual `ping` idiom.
    fn never_returns() -> &'static str {
        if cfg!(windows) {
            "ping -n 31 127.0.0.1"
        } else {
            "sleep 30"
        }
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
    fn settings_rule_line_pins_the_auto_external_shape() {
        // Consumer: `parse_forward_rules` in warren-cli/docker/entrypoint.sh.
        // It splits on " internal " and reads the head back as
        // "<internal_port>/<PROTO>", so the separators, the slash and the
        // "TCP+UDP" spelling are what the container's stale-rule cleanup
        // depends on.
        assert_eq!(
            settings_rule_line(&make_rule(Protocol::Both, 6881, 0)),
            "  6881/TCP+UDP internal 6881 -> external auto"
        );
    }

    #[test]
    fn settings_rule_line_pins_the_explicit_external_shape() {
        assert_eq!(
            settings_rule_line(&make_rule(Protocol::Tcp, 6881, 51413)),
            "  6881/TCP internal 6881 -> external 51413"
        );
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

    #[test]
    fn get_json_pins_the_settings_shape() {
        // Consumed by scripts through `jq`, so the key names, the key
        // order and the protocol spelling are the contract.
        let settings = NatPmpSettings {
            enabled: true,
            lifetime_secs: 3600,
            rules: vec![make_rule(Protocol::Both, 6881, 0)],
            ..Default::default()
        };
        assert_eq!(
            settings_json(&settings).expect("settings must serialize"),
            "{\"enabled\":true,\"lifetime_secs\":3600,\"rules\":[{\"protocol\":\"both\",\
             \"internal_port\":6881,\"suggested_external_port\":0}]}"
        );
    }

    #[test]
    fn status_json_pins_a_mapped_mapping() {
        let m = Mapping {
            internal_port: 6881,
            protocol: Proto::Both as i32,
            state: State::Mapped as i32,
            external_port: Some(58291),
            lifetime_granted_secs: Some(3600),
            ..Default::default()
        };
        assert_eq!(
            status_json(&[m]).expect("status must serialize"),
            "{\"mappings\":[{\"internal_port\":6881,\"protocol\":\"both\",\"state\":\"mapped\",\
             \"external_port\":58291,\"lifetime_granted_secs\":3600,\"error_reason\":null,\
             \"error_message\":null,\"retry_after_secs\":null,\"attempts_remaining\":null,\
             \"window_reset_secs\":null}]}"
        );
    }

    #[test]
    fn status_json_pins_a_failed_mapping_with_nulls_for_what_is_unset() {
        let m = Mapping {
            internal_port: 6881,
            protocol: Proto::Tcp as i32,
            state: State::Failed as i32,
            error_reason: Some(ErrorReason::SuggestedPortInUse as i32),
            error_message: Some("port in use".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            status_json(&[m]).expect("status must serialize"),
            "{\"mappings\":[{\"internal_port\":6881,\"protocol\":\"tcp\",\"state\":\"failed\",\
             \"external_port\":null,\"lifetime_granted_secs\":null,\
             \"error_reason\":\"suggested_port_in_use\",\"error_message\":\"port in use\",\
             \"retry_after_secs\":null,\"attempts_remaining\":null,\"window_reset_secs\":null}]}"
        );
    }

    #[test]
    fn status_json_pins_the_empty_mapping_list() {
        // The human notice must not leak into the JSON: a consumer reads
        // an empty array, never a sentence.
        assert_eq!(
            status_json(&[]).expect("an empty status must serialize"),
            "{\"mappings\":[]}"
        );
    }

    #[test]
    fn json_protocol_labels_are_the_protocol_option_values() {
        assert_eq!(proto_json_label(Proto::Udp as i32), "udp");
        assert_eq!(proto_json_label(Proto::Tcp as i32), "tcp");
        assert_eq!(proto_json_label(Proto::Both as i32), "both");
        assert_eq!(proto_json_label(42), "unknown", "forward compatibility");
    }

    #[test]
    fn json_state_labels_cover_every_state() {
        assert_eq!(state_json_label(State::Mapped as i32), "mapped");
        assert_eq!(state_json_label(State::Requesting as i32), "requesting");
        assert_eq!(state_json_label(State::Disabled as i32), "disabled");
        assert_eq!(state_json_label(State::RateLimited as i32), "rate_limited");
        assert_eq!(state_json_label(State::Failed as i32), "failed");
        assert_eq!(state_json_label(42), "unknown", "forward compatibility");
    }

    #[test]
    fn json_error_reason_labels_cover_every_reason() {
        assert_eq!(
            error_reason_json_label(ErrorReason::SuggestedPortInUse as i32),
            "suggested_port_in_use"
        );
        assert_eq!(
            error_reason_json_label(ErrorReason::OutOfResources as i32),
            "out_of_resources"
        );
        assert_eq!(
            error_reason_json_label(ErrorReason::NotAuthorized as i32),
            "not_authorized"
        );
        assert_eq!(
            error_reason_json_label(ErrorReason::Unknown as i32),
            "unknown"
        );
    }

    #[test]
    fn wait_is_done_once_every_rule_is_mapped() {
        let snapshot = [
            mapped(6881, Proto::Both, 58291),
            mapped(51820, Proto::Udp, 49152),
        ];
        assert_eq!(wait_verdict(&snapshot), WaitVerdict::Done);
    }

    #[test]
    fn wait_keeps_going_through_requesting_and_rate_limited() {
        let requesting = [
            mapped(6881, Proto::Both, 58291),
            in_state(51820, Proto::Udp, State::Requesting),
        ];
        assert_eq!(wait_verdict(&requesting), WaitVerdict::Keep);
        let rate_limited = [in_state(6881, Proto::Both, State::RateLimited)];
        assert_eq!(wait_verdict(&rate_limited), WaitVerdict::Keep);
    }

    #[test]
    fn wait_gives_up_when_any_rule_failed() {
        let snapshot = [
            mapped(6881, Proto::Both, 58291),
            in_state(51820, Proto::Udp, State::Failed),
        ];
        assert_eq!(wait_verdict(&snapshot), WaitVerdict::Failed);
    }

    #[test]
    fn wait_has_nothing_to_wait_for_on_an_empty_snapshot() {
        assert_eq!(wait_verdict(&[]), WaitVerdict::Nothing);
    }

    #[test]
    fn a_first_grant_is_a_port_change() {
        let next = [mapped(6881, Proto::Both, 58291)];
        assert_eq!(
            port_changes(&[], &next),
            vec![change_of(6881, Proto::Both, Some(58291))]
        );
    }

    #[test]
    fn a_renewal_on_the_same_port_is_not_a_port_change() {
        let previous = [mapped(6881, Proto::Both, 58291)];
        let next = [Mapping {
            lifetime_granted_secs: Some(3600),
            ..mapped(6881, Proto::Both, 58291)
        }];
        assert_eq!(
            port_changes(&previous, &next),
            vec![],
            "a renewal that keeps the port must not restart the application"
        );
    }

    #[test]
    fn a_moved_port_is_reported_with_the_new_value() {
        let previous = [mapped(6881, Proto::Both, 58291)];
        let next = [mapped(6881, Proto::Both, 49152)];
        assert_eq!(
            port_changes(&previous, &next),
            vec![change_of(6881, Proto::Both, Some(49152))]
        );
    }

    #[test]
    fn leaving_the_mapped_state_is_reported_as_a_lost_grant() {
        let previous = [mapped(6881, Proto::Both, 58291)];
        let next = [in_state(6881, Proto::Both, State::Failed)];
        assert_eq!(
            port_changes(&previous, &next),
            vec![change_of(6881, Proto::Both, None)]
        );
    }

    #[test]
    fn a_rule_dropped_from_the_snapshot_is_reported_as_a_lost_grant() {
        // Turning the feature off empties the list, and an application
        // still announcing that port has to be told.
        let previous = [mapped(6881, Proto::Both, 58291)];
        assert_eq!(
            port_changes(&previous, &[]),
            vec![change_of(6881, Proto::Both, None)]
        );
    }

    #[test]
    fn two_rules_that_both_move_produce_two_events_in_snapshot_order() {
        let previous = [
            mapped(6881, Proto::Both, 58291),
            mapped(51820, Proto::Udp, 49152),
        ];
        let next = [
            mapped(6881, Proto::Both, 58292),
            mapped(51820, Proto::Udp, 49153),
        ];
        assert_eq!(
            port_changes(&previous, &next),
            vec![
                change_of(6881, Proto::Both, Some(58292)),
                change_of(51820, Proto::Udp, Some(49153)),
            ]
        );
    }

    #[test]
    fn a_rule_that_is_still_requesting_is_not_a_port_change() {
        let next = [in_state(6881, Proto::Both, State::Requesting)];
        assert_eq!(
            port_changes(&[], &next),
            vec![],
            "no grant yet means nothing to announce"
        );
    }

    #[test]
    fn a_hook_child_never_inherits_the_recovery_phrase() {
        // Asserted on the spawn configuration rather than by reading the
        // child's environment back: setting a variable in this process
        // to observe it there is unsafe under edition 2024, and racy
        // against every other test that spawns a child.
        let command = hook_command("true", &change_of(6881, Proto::Both, Some(1)));
        let removed: Vec<String> = command
            .as_std()
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        assert!(
            removed.contains(&"WARREN_MNEMONIC".to_owned()),
            "a hook must never inherit the recovery phrase, removed: {removed:?}"
        );
        assert!(
            removed.contains(&"WARREN_MNEMONIC_FILE".to_owned()),
            "a hook must not be handed the secret's path either, removed: {removed:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_port_variables_reach_the_hook_child() {
        let path = std::env::temp_dir().join(format!("warren-pf-hook-env-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let command = format!(
            "printf '%s %s %s %s' \"$WARREN_PF_PORT\" \"$WARREN_PF_INTERNAL_PORT\" \
             \"$WARREN_PF_PROTOCOL\" \"$WARREN_PF_STATE\" > {}",
            path.display()
        );

        let outcome = run_hook(
            &command,
            &change_of(6881, Proto::Both, Some(58291)),
            HOOK_TIMEOUT,
        )
        .await;

        assert_eq!(outcome, HookOutcome::Succeeded);
        let written = std::fs::read_to_string(&path).expect("the hook must have run");
        assert_eq!(written, "58291 6881 both mapped");
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_public_port_is_substituted_into_the_command() {
        let path =
            std::env::temp_dir().join(format!("warren-pf-hook-subst-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        // Built by concatenation: `{{PORT}}` inside a format! literal
        // would be eaten as an escaped brace.
        let command = "printf '%s' port-{{PORT}}-ok > ".to_owned() + &path.display().to_string();

        let outcome = run_hook(
            &command,
            &change_of(6881, Proto::Both, Some(58291)),
            HOOK_TIMEOUT,
        )
        .await;

        assert_eq!(outcome, HookOutcome::Succeeded);
        assert_eq!(
            std::fs::read_to_string(&path).expect("the hook must have run"),
            "port-58291-ok"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_lost_grant_reaches_the_hook_as_an_empty_port() {
        let path = std::env::temp_dir().join(format!("warren-pf-hook-lost-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let command = "printf '%s|%s|%s' \"$WARREN_PF_PORT\" \"$WARREN_PF_STATE\" x{{PORT}}y > "
            .to_owned()
            + &path.display().to_string();

        let outcome = run_hook(&command, &change_of(6881, Proto::Both, None), HOOK_TIMEOUT).await;

        assert_eq!(outcome, HookOutcome::Succeeded);
        assert_eq!(
            std::fs::read_to_string(&path).expect("the hook must have run"),
            "|lost|xy"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn a_hook_that_never_returns_is_killed_by_its_timeout() {
        let started = std::time::Instant::now();

        let outcome = run_hook(
            never_returns(),
            &change_of(6881, Proto::Both, Some(58291)),
            Duration::from_millis(200),
        )
        .await;

        assert_eq!(
            outcome,
            HookOutcome::TimedOut,
            "a hook that never returns must be killed, not awaited forever"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the timeout must cut the wait short, took {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn a_failing_hook_is_reported_and_the_watch_carries_on() {
        assert_eq!(
            run_hook(
                "exit 3",
                &change_of(6881, Proto::Both, Some(58291)),
                HOOK_TIMEOUT
            )
            .await,
            HookOutcome::Failed,
            "a non-zero exit is an observation, never an error that stops the watch"
        );
    }
}
