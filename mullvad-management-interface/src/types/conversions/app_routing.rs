use mullvad_types::app_routing::{
    AppId, AppRouteState, AppRouteStatus, AppRoutingSettings, ExitChoice, SplitMode,
    UnavailableReason,
};

use crate::types::{FromProtobufTypeError, proto};

impl From<SplitMode> for proto::app_split_mode::Mode {
    fn from(mode: SplitMode) -> Self {
        match mode {
            SplitMode::Off => Self::Off,
            SplitMode::Exclude => Self::Exclude,
            SplitMode::IncludeOnly => Self::IncludeOnly,
        }
    }
}

impl From<proto::app_split_mode::Mode> for SplitMode {
    fn from(mode: proto::app_split_mode::Mode) -> Self {
        match mode {
            proto::app_split_mode::Mode::Off => Self::Off,
            proto::app_split_mode::Mode::Exclude => Self::Exclude,
            proto::app_split_mode::Mode::IncludeOnly => Self::IncludeOnly,
        }
    }
}

/// A split mode as it arrives on the wire.
///
/// # Errors
///
/// [`FromProtobufTypeError::InvalidArgument`] for a value this build does not
/// know, which is refused rather than read as `Off`.
pub fn split_mode(value: i32) -> Result<SplitMode, FromProtobufTypeError> {
    proto::app_split_mode::Mode::try_from(value)
        .map(SplitMode::from)
        .map_err(|_| FromProtobufTypeError::invalid_argument("unknown split mode"))
}

impl From<&ExitChoice> for proto::ExitChoice {
    fn from(choice: &ExitChoice) -> Self {
        Self {
            country: choice.country().to_owned(),
            city: choice.city().map(str::to_owned),
        }
    }
}

impl TryFrom<proto::ExitChoice> for ExitChoice {
    type Error = FromProtobufTypeError;

    fn try_from(choice: proto::ExitChoice) -> Result<Self, Self::Error> {
        ExitChoice::new(&choice.country, choice.city.as_deref())
            .map_err(|error| FromProtobufTypeError::invalid_argument(error.to_string()))
    }
}

/// Validates an app id received from a client.
///
/// # Errors
///
/// [`FromProtobufTypeError::InvalidArgument`] for a string that names no app
/// on this platform.
pub fn app_id(raw: &str) -> Result<AppId, FromProtobufTypeError> {
    AppId::parse(raw).map_err(|error| FromProtobufTypeError::invalid_argument(error.to_string()))
}

impl From<&AppRoutingSettings> for proto::AppRoutingSettings {
    fn from(settings: &AppRoutingSettings) -> Self {
        let apps = |apps: &std::collections::BTreeSet<AppId>| {
            apps.iter().map(|app| app.as_str().to_owned()).collect()
        };
        Self {
            split_mode: proto::app_split_mode::Mode::from(settings.split_mode) as i32,
            excluded_apps: apps(&settings.excluded_apps),
            included_apps: apps(&settings.included_apps),
            app_exits_enabled: settings.app_exits_enabled,
            app_exits: settings
                .app_exits
                .iter()
                .map(|(app, exit)| proto::AppExit {
                    app: app.as_str().to_owned(),
                    exit: Some(proto::ExitChoice::from(exit)),
                })
                .collect(),
        }
    }
}

impl TryFrom<proto::AppRoutingSettings> for AppRoutingSettings {
    type Error = FromProtobufTypeError;

    fn try_from(settings: proto::AppRoutingSettings) -> Result<Self, Self::Error> {
        // These come from the daemon, which validated them when they were
        // chosen: read them back as they are.
        let apps = |apps: &[String]| -> std::collections::BTreeSet<AppId> {
            apps.iter().map(|app| AppId::lenient(app)).collect()
        };
        let app_exits = settings
            .app_exits
            .into_iter()
            .map(|entry| {
                let exit = entry
                    .exit
                    .ok_or(FromProtobufTypeError::invalid_argument("missing app exit"))?;
                Ok((AppId::lenient(&entry.app), ExitChoice::try_from(exit)?))
            })
            .collect::<Result<_, FromProtobufTypeError>>()?;
        Ok(Self {
            split_mode: split_mode(settings.split_mode)?,
            excluded_apps: apps(&settings.excluded_apps),
            included_apps: apps(&settings.included_apps),
            app_exits_enabled: settings.app_exits_enabled,
            app_exits,
        })
    }
}

impl From<&AppRouteStatus> for proto::AppRouteStatus {
    fn from(status: &AppRouteStatus) -> Self {
        use proto::app_route_status::{State, UnavailableReason as Reason};
        let (state, reason) = match status.state {
            AppRouteState::Connecting => (State::Connecting, Reason::None),
            AppRouteState::Connected => (State::Connected, Reason::None),
            AppRouteState::Unavailable(reason) => (
                State::Unavailable,
                match reason {
                    UnavailableReason::TunnelDown => Reason::TunnelDown,
                    UnavailableReason::NoToken => Reason::NoToken,
                    UnavailableReason::LimitReached => Reason::LimitReached,
                    UnavailableReason::NoRelay => Reason::NoRelay,
                },
            ),
        };
        Self {
            exit: Some(proto::ExitChoice::from(&status.exit)),
            state: state as i32,
            reason: reason as i32,
            public_ip: status.public_ip.map(|ip| ip.to_string()),
            apps: status
                .apps
                .iter()
                .map(|app| app.as_str().to_owned())
                .collect(),
        }
    }
}

impl TryFrom<proto::AppRouteStatus> for AppRouteStatus {
    type Error = FromProtobufTypeError;

    fn try_from(status: proto::AppRouteStatus) -> Result<Self, Self::Error> {
        use proto::app_route_status::{State, UnavailableReason as Reason};
        let invalid = FromProtobufTypeError::invalid_argument;
        let state =
            match State::try_from(status.state).map_err(|_| invalid("unknown route state"))? {
                State::Unspecified => return Err(invalid("a route needs a state")),
                State::Connecting => AppRouteState::Connecting,
                State::Connected => AppRouteState::Connected,
                State::Unavailable => AppRouteState::Unavailable(
                    match Reason::try_from(status.reason).map_err(|_| invalid("unknown reason"))? {
                        Reason::None => return Err(invalid("an unavailable route needs a reason")),
                        Reason::TunnelDown => UnavailableReason::TunnelDown,
                        Reason::NoToken => UnavailableReason::NoToken,
                        Reason::LimitReached => UnavailableReason::LimitReached,
                        Reason::NoRelay => UnavailableReason::NoRelay,
                    },
                ),
            };
        let exit = status.exit.ok_or(invalid("missing route exit"))?;
        Ok(Self {
            exit: ExitChoice::try_from(exit)?,
            state,
            public_ip: status
                .public_ip
                .map(|ip| ip.parse().map_err(|_| invalid("invalid public address")))
                .transpose()?,
            apps: status.apps.iter().map(|app| AppId::lenient(app)).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(raw: &str) -> AppId {
        AppId::parse(raw).unwrap()
    }

    #[cfg(not(any(windows, target_os = "android")))]
    const APPS: [&str; 3] = ["/usr/bin/curl", "/opt/app/browser", "/opt/app/mailer"];
    #[cfg(windows)]
    const APPS: [&str; 3] = [r"C:\curl.exe", r"C:\browser.exe", r"C:\mailer.exe"];
    #[cfg(target_os = "android")]
    const APPS: [&str; 3] = ["net.curl", "org.browser", "org.mailer"];

    fn settings() -> AppRoutingSettings {
        let mut settings = AppRoutingSettings {
            split_mode: SplitMode::IncludeOnly,
            app_exits_enabled: true,
            ..Default::default()
        };
        settings.excluded_apps.insert(app(APPS[0]));
        settings.included_apps.insert(app(APPS[1]));
        settings
            .set_app_exit(app(APPS[2]), ExitChoice::new("se", Some("got")).unwrap())
            .unwrap();
        settings
            .set_app_exit(app(APPS[1]), ExitChoice::new("de", None).unwrap())
            .unwrap();
        settings
    }

    #[test]
    fn app_routing_settings_survive_the_wire() {
        let settings = settings();

        let wire = proto::AppRoutingSettings::from(&settings);
        let back = AppRoutingSettings::try_from(wire.clone()).unwrap();

        assert_eq!(back, settings);
        assert_eq!(
            wire.split_mode,
            proto::app_split_mode::Mode::IncludeOnly as i32
        );
        assert_eq!(wire.app_exits.len(), 2);
    }

    #[test]
    fn every_split_mode_survives_the_wire() {
        for mode in [SplitMode::Off, SplitMode::Exclude, SplitMode::IncludeOnly] {
            let wire = proto::app_split_mode::Mode::from(mode);

            assert_eq!(SplitMode::from(wire), mode);
            assert_eq!(split_mode(wire as i32).ok(), Some(mode));
        }
    }

    #[test]
    fn refuses_an_unknown_split_mode() {
        assert!(split_mode(7).is_err());
    }

    #[test]
    fn keeps_an_app_id_the_daemon_holds_even_if_it_names_no_app() {
        let mut wire = proto::AppRoutingSettings::from(&settings());
        wire.included_apps.push("relative-path".to_owned());

        let settings = AppRoutingSettings::try_from(wire).unwrap();

        assert!(
            settings
                .included_apps
                .contains(&AppId::lenient("relative-path"))
        );
    }

    #[test]
    fn refuses_an_invalid_exit_or_client_app_id() {
        let mut bad_exit = proto::AppRoutingSettings::from(&settings());
        bad_exit.app_exits[0].exit = Some(proto::ExitChoice {
            country: "sweden".to_owned(),
            city: None,
        });
        let mut missing_exit = proto::AppRoutingSettings::from(&settings());
        missing_exit.app_exits[0].exit = None;

        assert!(AppRoutingSettings::try_from(bad_exit).is_err());
        assert!(AppRoutingSettings::try_from(missing_exit).is_err());
        assert!(app_id("").is_err());
    }

    #[test]
    fn route_statuses_survive_the_wire() {
        let statuses = [
            AppRouteStatus {
                exit: ExitChoice::new("se", None).unwrap(),
                state: AppRouteState::Connected,
                public_ip: Some("203.0.113.5".parse().unwrap()),
                apps: vec![app(APPS[0])],
            },
            AppRouteStatus {
                exit: ExitChoice::new("de", Some("ber")).unwrap(),
                state: AppRouteState::Connecting,
                public_ip: None,
                apps: vec![app(APPS[1]), app(APPS[2])],
            },
        ];
        let reasons = [
            UnavailableReason::TunnelDown,
            UnavailableReason::NoToken,
            UnavailableReason::LimitReached,
            UnavailableReason::NoRelay,
        ]
        .map(|reason| AppRouteStatus {
            state: AppRouteState::Unavailable(reason),
            ..statuses[0].clone()
        });

        for status in statuses.iter().chain(&reasons) {
            let back = AppRouteStatus::try_from(proto::AppRouteStatus::from(status)).unwrap();

            assert_eq!(&back, status);
        }
    }

    #[test]
    fn refuses_a_route_without_a_state() {
        let mut wire = proto::AppRouteStatus::from(&AppRouteStatus {
            exit: ExitChoice::new("se", None).unwrap(),
            state: AppRouteState::Connected,
            public_ip: None,
            apps: vec![],
        });
        wire.state = proto::app_route_status::State::Unspecified as i32;

        assert!(AppRouteStatus::try_from(wire).is_err());
    }

    #[test]
    fn refuses_an_unavailable_route_without_a_reason() {
        let mut wire = proto::AppRouteStatus::from(&AppRouteStatus {
            exit: ExitChoice::new("se", None).unwrap(),
            state: AppRouteState::Unavailable(UnavailableReason::NoToken),
            public_ip: None,
            apps: vec![],
        });
        wire.reason = proto::app_route_status::UnavailableReason::None as i32;

        assert!(AppRouteStatus::try_from(wire).is_err());
    }
}
