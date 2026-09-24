//! `warren doctor`: one flat pass over what the daemon already knows.
//!
//! # Why
//!
//! A tunnel that is up but wrong looks exactly like a healthy one from every
//! surface a user can reach: the app says Connected and shows the same feature
//! chips. The facts that would settle it (the bundle width, which legs
//! deliver, the carrier bind verdict remembered for this network, whether the
//! host holds two interfaces on one LAN) all exist inside the daemon and none
//! of them had a way out.
//!
//! Each line reports what was OBSERVED. Where the observation does not support
//! a conclusion, the line says so instead of inventing one: an absent leg
//! count is "not measured", never "healthy".

use anyhow::Result;
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::states::TunnelState;
use mullvad_types::warren_diagnostics::CarrierVerdictKind;
use talpid_types::net::TunnelEndpoint;

/// How a probe came out. Ordering IS the severity ranking: the run's verdict is
/// the worst status it produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    /// Observed, and the observation is what it should be.
    Ok,
    /// Observed, and it is neither good nor bad on its own.
    Info,
    /// Observed something that degrades the connection without breaking it.
    Warn,
    /// The probe could not be answered at all.
    Fail,
}

impl Status {
    /// Fixed width so the probe column lines up whatever the statuses are.
    const fn token(self) -> &'static str {
        match self {
            Status::Ok => "OK  ",
            Status::Info => "INFO",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        }
    }
}

/// Process exit code for a run: 0 when nothing worse than an observation came
/// out, 1 on a degradation, 2 when a probe could not be answered. Distinct
/// codes so a script can tell "your tunnel is degraded" from "I could not look".
fn exit_code(statuses: &[Status]) -> i32 {
    match statuses.iter().max() {
        Some(Status::Fail) => 2,
        Some(Status::Warn) => 1,
        _ => 0,
    }
}

fn probe_line(status: Status, probe: &str, observation: &str) -> String {
    format!("{}  {probe:<22}{observation}", status.token())
}

/// Compact age, so a cached verdict's staleness is readable at a glance.
fn format_age(seconds: u64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3_600),
        s => format!("{}d", s / 86_400),
    }
}

#[derive(Default)]
struct Report {
    statuses: Vec<Status>,
}

impl Report {
    fn push(&mut self, status: Status, probe: &str, observation: impl AsRef<str>) {
        println!("{}", probe_line(status, probe, observation.as_ref()));
        self.statuses.push(status);
    }
}

pub async fn run() -> Result<()> {
    let mut report = Report::default();

    let mut client = match MullvadProxyClient::new().await {
        Ok(client) => {
            report.push(Status::Ok, "daemon", "reachable over the management socket");
            client
        }
        Err(error) => {
            report.push(Status::Fail, "daemon", format!("not reachable: {error}"));
            finish(&report);
        }
    };

    match client.get_tunnel_state().await {
        Ok(state) => report_tunnel_state(&mut report, &state),
        Err(error) => report.push(
            Status::Fail,
            "tunnel state",
            format!("could not be read: {error}"),
        ),
    }

    match client.get_feature_indicators().await {
        Ok(indicators) if indicators.is_empty() => {
            report.push(Status::Info, "features", "none active")
        }
        Ok(indicators) => report.push(Status::Info, "features", indicators.to_string()),
        Err(error) => report.push(
            Status::Fail,
            "features",
            format!("could not be read: {error}"),
        ),
    }

    match client.get_warren_diagnostics().await {
        Ok(diagnostics) => {
            report.push(
                Status::Info,
                "requested legs",
                format!(
                    "{} parallel connections requested for the next connect",
                    diagnostics.requested_n_connections
                ),
            );
            match diagnostics.carrier_verdict {
                Some(verdict) => {
                    let age = format_age(verdict.age_seconds);
                    let ttl = format_age(verdict.ttl_seconds);
                    match verdict.kind {
                        CarrierVerdictKind::BindOk => report.push(
                            Status::Ok,
                            "carrier bind",
                            format!(
                                "the bound carrier egressed on this network \
                                 (measured {age} ago, kept for {ttl})"
                            ),
                        ),
                        CarrierVerdictKind::RouteOnly => report.push(
                            Status::Warn,
                            "carrier bind",
                            format!(
                                "the bind black-holed on this network, so connects use the \
                                 wider carrier route exception (measured {age} ago, kept for \
                                 {ttl})"
                            ),
                        ),
                    }
                }
                None => report.push(
                    Status::Info,
                    "carrier bind",
                    "no verdict cached for the current network",
                ),
            }
            if diagnostics.dual_homed_interfaces.len() >= 2 {
                report.push(
                    Status::Warn,
                    "dual-homed host",
                    format!(
                        "{} reach the default gateway's subnet, so the LAN picks which one \
                         carries the tunnel's replies and part of the downlink can be dropped",
                        diagnostics.dual_homed_interfaces.join(" and ")
                    ),
                );
            } else {
                report.push(
                    Status::Info,
                    "dual-homed host",
                    "the daemon reported no second interface on the default gateway's subnet",
                );
            }
        }
        Err(error) => report.push(
            Status::Fail,
            "diagnostics",
            format!("could not be read: {error}"),
        ),
    }

    finish(&report);
}

fn report_tunnel_state(report: &mut Report, state: &TunnelState) {
    let (endpoint, location, status, label) = match state {
        TunnelState::Connected {
            endpoint, location, ..
        } => (endpoint, location.as_ref(), Status::Ok, "connected"),
        TunnelState::Connecting {
            endpoint, location, ..
        } => (endpoint, location.as_ref(), Status::Info, "connecting"),
        TunnelState::Disconnected { .. } => {
            report.push(Status::Info, "tunnel state", "disconnected");
            return;
        }
        TunnelState::Disconnecting(_) => {
            report.push(Status::Info, "tunnel state", "disconnecting");
            return;
        }
        TunnelState::Error(error) => {
            report.push(
                Status::Fail,
                "tunnel state",
                format!("error state: {}", error.cause()),
            );
            return;
        }
    };

    // The relay HOSTNAME, not its address: it names the same machine for an
    // operator without putting an endpoint in output a user is likely to paste
    // into a support thread.
    let relay = location
        .and_then(|location| location.hostname.clone())
        .unwrap_or_else(|| "unknown relay".to_owned());
    report.push(
        status,
        "tunnel state",
        format!("{label} via {relay} ({})", endpoint.tunnel_type),
    );

    match endpoint.effective_mtu {
        Some(mtu) => report.push(
            Status::Warn,
            "path MTU",
            format!("the live path carries at most {mtu} bytes of inner packet, below the tunnel default"),
        ),
        None => report.push(
            Status::Info,
            "path MTU",
            "no reduction measured on the live path",
        ),
    }

    let (status, observation) = bonded_legs_observation(endpoint);
    report.push(status, "bonded legs", observation);
}

/// The bonded-legs line. A leg counts as delivering only on the probe
/// evidence the tunnel monitor publishes: a leg's QUIC counters keep climbing
/// on the exit's acknowledgements while the exit drops every frame it carries.
fn bonded_legs_observation(endpoint: &TunnelEndpoint) -> (Status, String) {
    let bonded = endpoint.legs_bonded;
    if bonded == 0 {
        return (
            Status::Info,
            "not measured yet (in-tunnel probes measure every leg, and their first round \
             lands a few seconds after connecting)"
                .to_owned(),
        );
    }
    match endpoint.legs_not_delivering {
        0 => (
            Status::Ok,
            format!(
                "{bonded} bonded, all {bonded} delivering (each got the latest in-tunnel probe \
                 through the exit)"
            ),
        ),
        not_delivering => (
            Status::Warn,
            format!(
                "{bonded} bonded, {} delivering ({not_delivering} got no in-tunnel probe through \
                 the exit, or kept sending while receiving nothing back)",
                bonded.saturating_sub(not_delivering)
            ),
        ),
    }
}

/// Print the verdict and leave with the matching code.
fn finish(report: &Report) -> ! {
    let code = exit_code(&report.statuses);
    println!();
    println!(
        "{}",
        match code {
            0 => "Nothing observed to be wrong.",
            1 => "Something is degraded; the WARN lines say what was observed.",
            _ => "A probe could not be answered; the FAIL lines say which.",
        }
    );
    // `println!` writes through a line-buffered handle, so every line above has
    // already left the process by the time this runs.
    std::process::exit(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_run_exits_zero() {
        assert_eq!(exit_code(&[Status::Ok, Status::Info, Status::Ok]), 0);
    }

    #[test]
    fn a_degradation_exits_one_and_an_unanswerable_probe_exits_two() {
        assert_eq!(exit_code(&[Status::Ok, Status::Warn, Status::Info]), 1);
        assert_eq!(exit_code(&[Status::Ok, Status::Warn, Status::Fail]), 2);
    }

    #[test]
    fn an_empty_run_exits_zero() {
        assert_eq!(exit_code(&[]), 0);
    }

    #[test]
    fn every_status_token_has_the_same_width() {
        let widths: Vec<usize> = [Status::Ok, Status::Info, Status::Warn, Status::Fail]
            .iter()
            .map(|status| status.token().len())
            .collect();
        assert!(
            widths.iter().all(|width| *width == widths[0]),
            "a ragged status column defeats the point of a fixed token: {widths:?}"
        );
    }

    #[test]
    fn a_probe_line_starts_with_its_status_and_names_the_probe() {
        let line = probe_line(Status::Warn, "bonded legs", "8 bonded, 7 delivering");
        assert!(line.starts_with("WARN"), "{line}");
        assert!(line.contains("bonded legs"), "{line}");
        assert!(line.ends_with("8 bonded, 7 delivering"), "{line}");
    }

    fn endpoint(legs_bonded: u8, legs_not_delivering: u8) -> TunnelEndpoint {
        TunnelEndpoint {
            endpoint: talpid_types::net::Endpoint {
                address: "198.51.100.1:443".parse().unwrap(),
                protocol: talpid_types::net::TransportProtocol::Udp,
            },
            quantum_resistant: false,
            obfuscation: None,
            entry_endpoint: None,
            tunnel_interface: None,
            daita: false,
            effective_mtu: None,
            legs_bonded,
            legs_not_delivering,
            tunnel_type: talpid_types::net::TunnelType::Warren,
        }
    }

    #[test]
    fn a_bond_with_legs_that_do_not_deliver_is_a_warning_counting_those_that_do() {
        let (status, observation) = bonded_legs_observation(&endpoint(8, 3));
        assert_eq!(status, Status::Warn);
        assert!(
            observation.starts_with("8 bonded, 5 delivering"),
            "{observation}"
        );
    }

    #[test]
    fn a_bond_whose_every_leg_delivers_is_ok() {
        let (status, observation) = bonded_legs_observation(&endpoint(8, 0));
        assert_eq!(status, Status::Ok);
        assert!(
            observation.starts_with("8 bonded, all 8 delivering"),
            "{observation}"
        );
    }

    #[test]
    fn an_unmeasured_bond_claims_nothing() {
        let (status, observation) = bonded_legs_observation(&endpoint(0, 0));
        assert_eq!(status, Status::Info);
        assert!(observation.starts_with("not measured yet"), "{observation}");
    }

    #[test]
    fn ages_are_reported_in_the_largest_unit_that_fits() {
        assert_eq!(format_age(45), "45s");
        assert_eq!(format_age(3_599), "59m");
        assert_eq!(format_age(3_600), "1h");
        assert_eq!(format_age(86_400), "1d");
        assert_eq!(format_age(7 * 86_400), "7d");
    }
}
