use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use mullvad_management_interface::MullvadProxyClient;
use mullvad_types::app_routing::{
    AppRouteState, AppRouteStatus, AppRoutingSettings, ExitChoice, SplitMode, UnavailableReason,
};
use std::path::PathBuf;

use super::BooleanOption;

/// Choose which apps bypass the VPN, which apps alone use it, and which apps
/// leave the Internet from a country of their own
#[derive(Subcommand, Debug)]
pub enum AppRouting {
    /// Display the split mode, the app lists and the per-app countries
    Get,

    /// Choose the split mode
    Mode {
        #[arg(value_enum)]
        mode: Mode,
    },

    /// Manage the apps that bypass the VPN while the mode is `exclude`
    #[clap(subcommand)]
    Exclude(AppList),

    /// Manage the apps alone in the VPN while the mode is `include-only`
    #[clap(subcommand)]
    Include(AppList),

    /// Turn the per-app countries on or off
    Exits { policy: BooleanOption },

    /// Manage the country each app leaves the Internet from
    #[clap(subcommand)]
    Exit(Exit),

    /// Show where the connection of each chosen country stands
    Status,
}

/// A split mode, as typed on the command line.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Every app uses the VPN
    Off,
    /// The excluded apps bypass the VPN
    Exclude,
    /// Only the included apps use the VPN
    IncludeOnly,
}

impl From<Mode> for SplitMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Off => SplitMode::Off,
            Mode::Exclude => SplitMode::Exclude,
            Mode::IncludeOnly => SplitMode::IncludeOnly,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum AppList {
    Add { path: PathBuf },
    Remove { path: PathBuf },
}

#[derive(Subcommand, Debug)]
pub enum Exit {
    /// Make an app leave the Internet from a country, and optionally a city
    Set {
        path: PathBuf,
        /// Two-letter country code, such as `se`
        country: String,
        /// City code, such as `got`
        city: Option<String>,
    },
    /// Send an app back through the main connection
    Clear { path: PathBuf },
}

impl AppRouting {
    pub async fn handle(self) -> Result<()> {
        let mut rpc = MullvadProxyClient::new().await?;
        match self {
            AppRouting::Get => {
                let settings = rpc.get_settings().await?.app_routing;
                print!("{}", render_settings(&settings));
            }
            AppRouting::Mode { mode } => {
                rpc.set_app_split_mode(SplitMode::from(mode)).await?;
                println!("Split mode: {}", mode_label(SplitMode::from(mode)));
            }
            AppRouting::Exclude(AppList::Add { path }) => {
                rpc.add_split_tunnel_app(path).await?;
                println!("Added the app to the apps that bypass the VPN");
            }
            AppRouting::Exclude(AppList::Remove { path }) => {
                rpc.remove_split_tunnel_app(path).await?;
                println!("Removed the app from the apps that bypass the VPN");
            }
            AppRouting::Include(AppList::Add { path }) => {
                rpc.add_included_app(path).await?;
                println!("Added the app to the apps alone in the VPN");
            }
            AppRouting::Include(AppList::Remove { path }) => {
                rpc.remove_included_app(path).await?;
                println!("Removed the app from the apps alone in the VPN");
            }
            AppRouting::Exits { policy } => {
                rpc.set_app_exits_enabled(*policy).await?;
                println!("Per-app countries: {policy}");
            }
            AppRouting::Exit(Exit::Set {
                path,
                country,
                city,
            }) => {
                let exit = ExitChoice::new(&country, city.as_deref())?;
                rpc.set_app_exit(path, &exit).await?;
                println!("The app now leaves from {}", exit_label(&exit));
            }
            AppRouting::Exit(Exit::Clear { path }) => {
                rpc.clear_app_exit(path).await?;
                println!("The app now uses the main connection");
            }
            AppRouting::Status => {
                let statuses = rpc.get_app_route_status().await?;
                print!("{}", render_statuses(&statuses));
            }
        }
        Ok(())
    }
}

fn mode_label(mode: SplitMode) -> &'static str {
    match mode {
        SplitMode::Off => "off",
        SplitMode::Exclude => "exclude",
        SplitMode::IncludeOnly => "include-only",
    }
}

fn exit_label(exit: &ExitChoice) -> String {
    match exit.city() {
        Some(city) => format!("{}, {city}", exit.country()),
        None => exit.country().to_owned(),
    }
}

fn render_settings(settings: &AppRoutingSettings) -> String {
    let mut out = format!("Split mode: {}\n", mode_label(settings.split_mode));
    out.push_str("Apps that bypass the VPN:\n");
    for app in &settings.excluded_apps {
        out.push_str(&format!("    {}\n", app.as_str()));
    }
    out.push_str("Apps alone in the VPN:\n");
    for app in &settings.included_apps {
        out.push_str(&format!("    {}\n", app.as_str()));
    }
    let exits = BooleanOption::from(settings.app_exits_enabled);
    out.push_str(&format!("Per-app countries: {exits}\n"));
    for (app, exit) in &settings.app_exits {
        out.push_str(&format!("    {}: {}\n", app.as_str(), exit_label(exit)));
    }
    out
}

fn render_statuses(statuses: &[AppRouteStatus]) -> String {
    if statuses.is_empty() {
        return "No app has a country of its own\n".to_owned();
    }
    let mut out = String::new();
    for status in statuses {
        out.push_str(&format!(
            "{}: {}",
            exit_label(&status.exit),
            state_label(status.state)
        ));
        if let Some(ip) = status.public_ip {
            out.push_str(&format!(", from {ip}"));
        }
        out.push('\n');
        for app in &status.apps {
            out.push_str(&format!("    {}\n", app.as_str()));
        }
    }
    out
}

fn state_label(state: AppRouteState) -> &'static str {
    match state {
        AppRouteState::Connecting => "connecting",
        AppRouteState::Connected => "connected",
        AppRouteState::Unavailable(UnavailableReason::TunnelDown) => {
            "unavailable, the VPN is not connected"
        }
        AppRouteState::Unavailable(UnavailableReason::NoToken) => {
            "unavailable, no session token left"
        }
        AppRouteState::Unavailable(UnavailableReason::LimitReached) => {
            "unavailable, the session limit is reached"
        }
        AppRouteState::Unavailable(UnavailableReason::NoRelay) => {
            "unavailable, no server in this location"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use mullvad_types::app_routing::AppId;

    #[cfg(not(windows))]
    const BROWSER: &str = "/opt/app/browser";
    #[cfg(windows)]
    const BROWSER: &str = r"C:\app\browser.exe";

    fn app(raw: &str) -> AppId {
        AppId::parse(raw).unwrap()
    }

    #[test]
    fn renders_every_part_of_the_settings() {
        let mut settings = AppRoutingSettings {
            split_mode: SplitMode::IncludeOnly,
            app_exits_enabled: true,
            ..Default::default()
        };
        settings.included_apps.insert(app(BROWSER));
        settings
            .set_app_exit(app(BROWSER), ExitChoice::new("se", Some("got")).unwrap())
            .unwrap();

        let rendered = render_settings(&settings);

        assert_eq!(
            rendered,
            format!(
                "Split mode: include-only\n\
                 Apps that bypass the VPN:\n\
                 Apps alone in the VPN:\n    {BROWSER}\n\
                 Per-app countries: on\n    {BROWSER}: se, got\n"
            )
        );
    }

    #[test]
    fn renders_each_route_with_its_state_address_and_apps() {
        let statuses = [
            AppRouteStatus {
                exit: ExitChoice::new("se", None).unwrap(),
                state: AppRouteState::Connected,
                public_ip: Some("203.0.113.5".parse().unwrap()),
                apps: vec![app(BROWSER)],
            },
            AppRouteStatus {
                exit: ExitChoice::new("de", Some("ber")).unwrap(),
                state: AppRouteState::Unavailable(UnavailableReason::NoToken),
                public_ip: None,
                apps: vec![],
            },
        ];

        let rendered = render_statuses(&statuses);

        assert_eq!(
            rendered,
            format!(
                "se: connected, from 203.0.113.5\n    {BROWSER}\n\
                 de, ber: unavailable, no session token left\n"
            )
        );
    }

    #[test]
    fn renders_no_route_as_such() {
        assert_eq!(render_statuses(&[]), "No app has a country of its own\n");
    }

    #[test]
    fn names_every_unavailable_reason() {
        let labels = [
            UnavailableReason::TunnelDown,
            UnavailableReason::NoToken,
            UnavailableReason::LimitReached,
            UnavailableReason::NoRelay,
        ]
        .map(|reason| state_label(AppRouteState::Unavailable(reason)));

        assert_eq!(
            labels,
            [
                "unavailable, the VPN is not connected",
                "unavailable, no session token left",
                "unavailable, the session limit is reached",
                "unavailable, no server in this location",
            ]
        );
    }

    #[test]
    fn parses_each_subcommand() {
        let parse = |args: &[&str]| {
            crate::Cli::try_parse_from([&["warren", "app-routing"], args].concat()).is_ok()
        };

        assert!(parse(&["get"]));
        assert!(parse(&["mode", "include-only"]));
        assert!(parse(&["exclude", "add", BROWSER]));
        assert!(parse(&["include", "remove", BROWSER]));
        assert!(parse(&["exits", "on"]));
        assert!(parse(&["exit", "set", BROWSER, "se", "got"]));
        assert!(parse(&["exit", "clear", BROWSER]));
        assert!(parse(&["status"]));
        assert!(!parse(&["mode", "sideways"]));
    }
}
