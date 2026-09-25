//! App routing settings: which apps bypass the tunnel, which apps alone use
//! it, and which apps leave through an exit of their own.
//!
//! The design contract is `docs/app-routing.md`. The precedence rules of its
//! section 1 live here, so the daemon enforces them whatever a client sends.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::IpAddr,
};

use serde::{Deserialize, Serialize};

use crate::location::{CityCode, CountryCode};

/// Route sessions that can run next to the main one: the three session
/// tokens of an epoch, minus the main session's.
pub const MAX_APP_EXITS: usize = 2;

/// Which split mode is in force. The two lists are kept whatever the mode, so
/// switching back restores them.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    #[default]
    Off,
    /// The apps in `excluded_apps` talk to the network as if Warren were off.
    Exclude,
    /// Only the apps in `included_apps` (and the apps with an exit) are
    /// tunneled.
    IncludeOnly,
}

/// Why an app routing change is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AppRoutingError {
    #[error("the app id is empty")]
    EmptyAppId,
    #[error("the app id is not an absolute executable path")]
    NotAbsolutePath,
    #[error("the app id is not an Android package name")]
    NotPackageName,
    #[error("the country is not a two-letter country code")]
    InvalidCountry,
    #[error("the city is not a city code")]
    InvalidCity,
    #[error("at most {limit} different exits can be chosen for apps")]
    TooManyAppExits { limit: usize },
}

/// The conventions an app id follows on one platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIdFlavor {
    /// macOS and Linux: an absolute path.
    Unix,
    /// Windows: an absolute path with a drive letter or a UNC prefix.
    Windows,
    /// Android: a package name.
    Android,
}

impl AppIdFlavor {
    /// The conventions of the platform this code runs on.
    pub const HOST: Self = if cfg!(target_os = "android") {
        Self::Android
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::Unix
    };
}

/// An app as the user chose it: an executable path on Windows and Linux, the
/// path of an `.app` bundle or of an executable inside one on macOS, a package
/// name on Android.
///
/// Matching an executable to an app id (the outermost bundle on macOS, case on
/// Windows) is the router's business; an app id keeps what the user chose,
/// normalized only in ways that cannot change which app it names.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct AppId(String);

// Settings written before app ids were normalized (the split tunneling list)
// hold paths as the user typed them; reading them normalized is what lets a
// later removal, which is normalized, find them.
impl<'de> Deserialize<'de> for AppId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::lenient(&raw))
    }
}

impl AppId {
    /// Validates and normalizes `raw` for the host platform.
    ///
    /// # Errors
    ///
    /// [`AppRoutingError::EmptyAppId`], [`AppRoutingError::NotAbsolutePath`]
    /// or [`AppRoutingError::NotPackageName`] for input that names no app.
    pub fn parse(raw: &str) -> Result<Self, AppRoutingError> {
        Self::parse_as(AppIdFlavor::HOST, raw)
    }

    /// As [`Self::parse`], for the conventions of `flavor`.
    ///
    /// # Errors
    ///
    /// As [`Self::parse`].
    pub fn parse_as(flavor: AppIdFlavor, raw: &str) -> Result<Self, AppRoutingError> {
        if raw.contains('\0') {
            return Err(AppRoutingError::NotAbsolutePath);
        }
        match flavor {
            AppIdFlavor::Unix => {
                let path = raw.trim_end_matches('/');
                if path.is_empty() {
                    return Err(AppRoutingError::EmptyAppId);
                }
                if !path.starts_with('/') {
                    return Err(AppRoutingError::NotAbsolutePath);
                }
                Ok(Self(path.to_owned()))
            }
            AppIdFlavor::Windows => {
                let path = raw.replace('/', "\\");
                let path = path.trim_end_matches('\\');
                if path.is_empty() {
                    return Err(AppRoutingError::EmptyAppId);
                }
                let bytes = path.as_bytes();
                let drive = bytes.len() > 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && bytes[2] == b'\\';
                if !drive && !path.starts_with(r"\\") {
                    return Err(AppRoutingError::NotAbsolutePath);
                }
                Ok(Self(path.to_owned()))
            }
            AppIdFlavor::Android => {
                if raw.is_empty() {
                    return Err(AppRoutingError::EmptyAppId);
                }
                let segments_valid = raw.split('.').all(|segment| {
                    !segment.is_empty()
                        && segment
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_')
                });
                if !segments_valid || !raw.contains('.') {
                    return Err(AppRoutingError::NotPackageName);
                }
                Ok(Self(raw.to_owned()))
            }
        }
    }

    /// An app id the daemon already holds, read back from its settings or
    /// its answers: normalized when it is valid, kept as it is otherwise, so
    /// nothing saved is lost, rejected or left unremovable on the way back.
    /// Input from a client goes through [`Self::parse`] instead.
    pub fn lenient(raw: &str) -> Self {
        Self::lenient_as(AppIdFlavor::HOST, raw)
    }

    /// As [`Self::lenient`], for the conventions of `flavor`.
    pub fn lenient_as(flavor: AppIdFlavor, raw: &str) -> Self {
        Self::parse_as(flavor, raw).unwrap_or_else(|_| Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The form the tunnel's exclusion commands take.
    #[cfg(not(target_os = "android"))]
    pub fn to_tunnel_command_repr(&self) -> std::ffi::OsString {
        std::ffi::OsString::from(&self.0)
    }

    /// The form the tunnel's exclusion commands take.
    #[cfg(target_os = "android")]
    pub fn to_tunnel_command_repr(&self) -> String {
        self.0.clone()
    }
}

// An app id is a path on the user's machine: it stays out of any log line
// that renders a settings value.
impl fmt::Debug for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppId(..)")
    }
}

/// The exit an app leaves through: a country, and optionally a city in it.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "ExitChoiceRepr", into = "ExitChoiceRepr")]
pub struct ExitChoice {
    country: CountryCode,
    city: Option<CityCode>,
}

#[derive(Serialize, Deserialize)]
struct ExitChoiceRepr {
    country: CountryCode,
    #[serde(default)]
    city: Option<CityCode>,
}

impl ExitChoice {
    /// A choice of `country` (a two-letter code) and optionally `city` (a
    /// relay list city code), both lowercased.
    ///
    /// # Errors
    ///
    /// [`AppRoutingError::InvalidCountry`] or [`AppRoutingError::InvalidCity`].
    pub fn new(country: &str, city: Option<&str>) -> Result<Self, AppRoutingError> {
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(AppRoutingError::InvalidCountry);
        }
        let city = city
            .map(|city| {
                let valid = (1..=16).contains(&city.len())
                    && city.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
                valid
                    .then(|| city.to_ascii_lowercase())
                    .ok_or(AppRoutingError::InvalidCity)
            })
            .transpose()?;
        Ok(Self {
            country: country.to_ascii_lowercase(),
            city,
        })
    }

    pub fn country(&self) -> &str {
        &self.country
    }

    pub fn city(&self) -> Option<&str> {
        self.city.as_deref()
    }
}

// Where an app's traffic leaves is exit identity: no log line renders it.
impl fmt::Debug for ExitChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExitChoice(..)")
    }
}

impl TryFrom<ExitChoiceRepr> for ExitChoice {
    type Error = AppRoutingError;

    fn try_from(repr: ExitChoiceRepr) -> Result<Self, Self::Error> {
        Self::new(&repr.country, repr.city.as_deref())
    }
}

impl From<ExitChoice> for ExitChoiceRepr {
    fn from(choice: ExitChoice) -> Self {
        Self {
            country: choice.country,
            city: choice.city,
        }
    }
}

/// The app routing page, as persisted.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppRoutingSettings {
    pub split_mode: SplitMode,
    /// Apps outside the tunnel while `split_mode` is `Exclude`.
    pub excluded_apps: BTreeSet<AppId>,
    /// Apps alone in the tunnel while `split_mode` is `IncludeOnly`.
    pub included_apps: BTreeSet<AppId>,
    /// The master switch of the per-app exits.
    pub app_exits_enabled: bool,
    /// At most [`MAX_APP_EXITS`] different exits through
    /// [`Self::set_app_exit`]; a settings file edited by hand may hold more,
    /// so whatever opens route sessions caps them itself.
    pub app_exits: BTreeMap<AppId, ExitChoice>,
}

impl AppRoutingSettings {
    /// Whether the excluded apps are outside the tunnel right now.
    pub fn exclusions_active(&self) -> bool {
        self.split_mode == SplitMode::Exclude
    }

    /// Chooses `exit` for `app`.
    ///
    /// # Errors
    ///
    /// [`AppRoutingError::TooManyAppExits`] when the apps would then use more
    /// than [`MAX_APP_EXITS`] different exits. The settings are unchanged.
    pub fn set_app_exit(&mut self, app: AppId, exit: ExitChoice) -> Result<(), AppRoutingError> {
        let distinct: BTreeSet<&ExitChoice> = self
            .app_exits
            .iter()
            .filter(|(other, _)| **other != app)
            .map(|(_, choice)| choice)
            .chain(std::iter::once(&exit))
            .collect();
        if distinct.len() > MAX_APP_EXITS {
            return Err(AppRoutingError::TooManyAppExits {
                limit: MAX_APP_EXITS,
            });
        }
        self.app_exits.insert(app, exit);
        Ok(())
    }

    /// The exits in force, after the precedence rules: none while the master
    /// switch is off, and none for an app outside the tunnel because it is
    /// excluded.
    pub fn effective_app_exits(&self) -> BTreeMap<&AppId, &ExitChoice> {
        if !self.app_exits_enabled {
            return BTreeMap::new();
        }
        self.app_exits
            .iter()
            .filter(|(app, _)| !(self.exclusions_active() && self.excluded_apps.contains(*app)))
            .collect()
    }

    /// The apps tunneled while `split_mode` is `IncludeOnly`: the included
    /// apps, and every app with an exit in force, since choosing a country
    /// for an app in that mode puts it in the tunnel. Empty in other modes.
    pub fn effective_included_apps(&self) -> BTreeSet<&AppId> {
        if self.split_mode != SplitMode::IncludeOnly {
            return BTreeSet::new();
        }
        self.included_apps
            .iter()
            .chain(self.effective_app_exits().into_keys())
            .collect()
    }

    /// One status per exit in force, each naming the apps that use it, in the
    /// order of the exits. `state` says where the session of an exit stands.
    pub fn route_statuses(
        &self,
        mut state: impl FnMut(&ExitChoice) -> (AppRouteState, Option<IpAddr>),
    ) -> Vec<AppRouteStatus> {
        let mut by_exit: BTreeMap<&ExitChoice, Vec<AppId>> = BTreeMap::new();
        for (app, exit) in self.effective_app_exits() {
            by_exit.entry(exit).or_default().push(app.clone());
        }
        by_exit
            .into_iter()
            .map(|(exit, apps)| {
                let (route_state, public_ip) = state(exit);
                AppRouteStatus {
                    exit: exit.clone(),
                    state: route_state,
                    public_ip,
                    apps,
                }
            })
            .collect()
    }
}

/// Where the session of one exit stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppRouteState {
    Connecting,
    Connected,
    Unavailable(UnavailableReason),
}

/// Why the session of an exit cannot run. Its apps are blocked meanwhile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The main connection is down, and route sessions live inside it.
    TunnelDown,
    /// No anonymous session token is left for this epoch.
    NoToken,
    /// The account's session limit is reached.
    LimitReached,
    /// No exit matches the choice.
    NoRelay,
}

/// What the user sees for one exit: the state of its session, the public
/// address its apps appear from when known, and the apps that use it.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct AppRouteStatus {
    pub exit: ExitChoice,
    pub state: AppRouteState,
    pub public_ip: Option<IpAddr>,
    pub apps: Vec<AppId>,
}

impl fmt::Debug for AppRouteStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppRouteStatus")
            .field("state", &self.state)
            .field("public_ip", &self.public_ip.is_some())
            .field("apps", &self.apps.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(path: &str) -> AppId {
        AppId::parse_as(AppIdFlavor::Unix, path).unwrap()
    }

    fn exit(country: &str) -> ExitChoice {
        ExitChoice::new(country, None).unwrap()
    }

    #[test]
    fn a_unix_app_id_is_an_absolute_path_without_trailing_slash() {
        let bundle = AppId::parse_as(AppIdFlavor::Unix, "/Applications/Firefox.app/");

        assert_eq!(
            bundle.map(|id| id.as_str().to_owned()),
            Ok("/Applications/Firefox.app".to_owned())
        );
        assert_eq!(
            AppId::parse_as(AppIdFlavor::Unix, "Applications/Firefox.app").map(|_| ()),
            Err(AppRoutingError::NotAbsolutePath)
        );
        assert_eq!(
            AppId::parse_as(AppIdFlavor::Unix, "").map(|_| ()),
            Err(AppRoutingError::EmptyAppId)
        );
        assert_eq!(
            AppId::parse_as(AppIdFlavor::Unix, "/").map(|_| ()),
            Err(AppRoutingError::EmptyAppId)
        );
        assert_eq!(
            AppId::parse_as(AppIdFlavor::Unix, "/usr/bin/a\0b").map(|_| ()),
            Err(AppRoutingError::NotAbsolutePath)
        );
    }

    #[test]
    fn a_unix_app_id_keeps_its_case_and_its_place_in_a_bundle() {
        let id = AppId::parse_as(
            AppIdFlavor::Unix,
            "/Applications/Firefox.app/Contents/MacOS/Firefox",
        )
        .unwrap();

        assert_eq!(
            id.as_str(),
            "/Applications/Firefox.app/Contents/MacOS/Firefox"
        );
    }

    #[test]
    fn a_windows_app_id_takes_backslashes_and_a_drive_or_unc_root() {
        let drive = AppId::parse_as(AppIdFlavor::Windows, "C:/Program Files/App/app.exe");
        let unc = AppId::parse_as(AppIdFlavor::Windows, r"\\server\share\app.exe");
        let relative = AppId::parse_as(AppIdFlavor::Windows, r"app.exe");

        assert_eq!(drive.unwrap().as_str(), r"C:\Program Files\App\app.exe");
        assert_eq!(unc.unwrap().as_str(), r"\\server\share\app.exe");
        assert_eq!(relative.map(|_| ()), Err(AppRoutingError::NotAbsolutePath));
    }

    #[test]
    fn an_android_app_id_is_a_package_name() {
        let package = AppId::parse_as(AppIdFlavor::Android, "org.mozilla.firefox");
        let path = AppId::parse_as(AppIdFlavor::Android, "/system/bin/sh");
        let single = AppId::parse_as(AppIdFlavor::Android, "firefox");

        assert_eq!(package.unwrap().as_str(), "org.mozilla.firefox");
        assert_eq!(path.map(|_| ()), Err(AppRoutingError::NotPackageName));
        assert_eq!(single.map(|_| ()), Err(AppRoutingError::NotPackageName));
    }

    #[test]
    fn a_stored_app_id_is_read_back_normalized_when_valid() {
        let trailing = AppId::lenient_as(AppIdFlavor::Unix, "/Applications/Firefox.app/");
        let forward = AppId::lenient_as(AppIdFlavor::Windows, "C:/Tools/app.exe");

        assert_eq!(trailing.as_str(), "/Applications/Firefox.app");
        assert_eq!(forward.as_str(), r"C:\Tools\app.exe");
    }

    #[test]
    fn a_stored_app_id_that_names_no_app_is_kept_as_it_is() {
        let relative = AppId::lenient_as(AppIdFlavor::Unix, "firefox");

        assert_eq!(relative.as_str(), "firefox");
    }

    #[cfg(unix)]
    #[test]
    fn persisted_lists_merge_entries_that_name_the_same_app() {
        let json = r#"{"excluded_apps": ["/opt/app/browser/", "/opt/app/browser"]}"#;

        let settings: AppRoutingSettings = serde_json::from_str(json).unwrap();

        let apps: Vec<&str> = settings.excluded_apps.iter().map(AppId::as_str).collect();
        assert_eq!(apps, ["/opt/app/browser"]);
    }

    #[test]
    fn an_exit_choice_renders_without_its_location() {
        let rendered = format!("{:?}", ExitChoice::new("zq", Some("qxq")).unwrap());

        assert!(
            !rendered.contains("zq") && !rendered.contains("qxq"),
            "{rendered}"
        );
    }

    #[test]
    fn a_route_status_renders_without_its_exit_or_address() {
        let status = AppRouteStatus {
            exit: ExitChoice::new("zq", Some("qxq")).unwrap(),
            state: AppRouteState::Connected,
            public_ip: Some("203.0.113.5".parse().unwrap()),
            apps: vec![app("/opt/app/browser")],
        };

        let rendered = format!("{status:?}");

        for secret in ["zq", "qxq", "203.0.113.5", "browser"] {
            assert!(!rendered.contains(secret), "{secret} in {rendered}");
        }
    }

    #[test]
    fn an_app_id_renders_without_its_path() {
        let rendered = format!("{:?}", app("/Applications/Secret.app"));

        assert!(!rendered.contains("Secret"), "{rendered}");
    }

    #[test]
    fn an_exit_choice_is_a_lowercased_country_and_an_optional_city() {
        let choice = ExitChoice::new("SE", Some("GOT")).unwrap();

        assert_eq!((choice.country(), choice.city()), ("se", Some("got")));
        assert_eq!(
            ExitChoice::new("swe", None),
            Err(AppRoutingError::InvalidCountry)
        );
        assert_eq!(
            ExitChoice::new("s1", None),
            Err(AppRoutingError::InvalidCountry)
        );
        assert_eq!(
            ExitChoice::new("se", Some("")),
            Err(AppRoutingError::InvalidCity)
        );
        assert_eq!(
            ExitChoice::new("se", Some("g t")),
            Err(AppRoutingError::InvalidCity)
        );
    }

    #[test]
    fn a_persisted_exit_choice_is_validated_on_load() {
        let valid: Result<ExitChoice, _> = serde_json::from_str(r#"{"country":"DE","city":"ber"}"#);
        let invalid: Result<ExitChoice, _> = serde_json::from_str(r#"{"country":"germany"}"#);

        assert_eq!(valid.unwrap(), ExitChoice::new("de", Some("ber")).unwrap());
        assert!(invalid.is_err());
    }

    #[test]
    fn allows_as_many_distinct_exits_as_route_sessions() {
        let mut settings = AppRoutingSettings::default();
        settings.set_app_exit(app("/a"), exit("se")).unwrap();
        settings.set_app_exit(app("/b"), exit("de")).unwrap();
        settings.set_app_exit(app("/c"), exit("de")).unwrap();

        let third = settings.set_app_exit(app("/d"), exit("fr"));

        assert_eq!(
            third,
            Err(AppRoutingError::TooManyAppExits {
                limit: MAX_APP_EXITS
            })
        );
        assert!(!settings.app_exits.contains_key(&app("/d")));
    }

    #[test]
    fn moving_an_app_to_a_new_exit_frees_its_old_one() {
        let mut settings = AppRoutingSettings::default();
        settings.set_app_exit(app("/a"), exit("se")).unwrap();
        settings.set_app_exit(app("/b"), exit("de")).unwrap();

        let moved = settings.set_app_exit(app("/a"), exit("fr"));

        assert_eq!(moved, Ok(()));
        assert_eq!(settings.app_exits.get(&app("/a")), Some(&exit("fr")));
    }

    #[test]
    fn a_city_makes_a_distinct_exit() {
        let mut settings = AppRoutingSettings::default();
        settings.set_app_exit(app("/a"), exit("se")).unwrap();
        settings
            .set_app_exit(app("/b"), ExitChoice::new("se", Some("got")).unwrap())
            .unwrap();

        let third = settings.set_app_exit(app("/c"), ExitChoice::new("se", Some("sto")).unwrap());

        assert_eq!(
            third,
            Err(AppRoutingError::TooManyAppExits {
                limit: MAX_APP_EXITS
            })
        );
    }

    fn routed(mode: SplitMode) -> AppRoutingSettings {
        let mut settings = AppRoutingSettings {
            split_mode: mode,
            app_exits_enabled: true,
            ..Default::default()
        };
        settings.excluded_apps.insert(app("/excluded"));
        settings.included_apps.insert(app("/included"));
        settings.set_app_exit(app("/excluded"), exit("se")).unwrap();
        settings.set_app_exit(app("/routed"), exit("de")).unwrap();
        settings
    }

    #[test]
    fn an_excluded_app_ignores_its_exit_while_exclusion_is_on() {
        let excluding = routed(SplitMode::Exclude);
        let off = routed(SplitMode::Off);

        let excluding: Vec<&str> = excluding
            .effective_app_exits()
            .keys()
            .map(|id| id.as_str())
            .collect();
        let off: Vec<&str> = off
            .effective_app_exits()
            .keys()
            .map(|id| id.as_str())
            .collect();

        assert_eq!(excluding, ["/routed"]);
        assert_eq!(off, ["/excluded", "/routed"]);
    }

    #[test]
    fn no_exit_is_in_force_while_the_master_switch_is_off() {
        let mut settings = routed(SplitMode::Off);
        settings.app_exits_enabled = false;

        assert!(settings.effective_app_exits().is_empty());
    }

    #[test]
    fn choosing_an_exit_includes_an_app_in_include_only_mode() {
        let including = routed(SplitMode::IncludeOnly);

        let included: Vec<&str> = including
            .effective_included_apps()
            .iter()
            .map(|id| id.as_str())
            .collect();

        assert_eq!(included, ["/excluded", "/included", "/routed"]);
    }

    #[test]
    fn nothing_is_included_outside_include_only_mode() {
        assert!(routed(SplitMode::Off).effective_included_apps().is_empty());
        assert!(
            routed(SplitMode::Exclude)
                .effective_included_apps()
                .is_empty()
        );
    }

    #[test]
    fn an_exit_without_its_master_switch_does_not_include_an_app() {
        let mut settings = routed(SplitMode::IncludeOnly);
        settings.app_exits_enabled = false;

        let included: Vec<&str> = settings
            .effective_included_apps()
            .iter()
            .map(|id| id.as_str())
            .collect();

        assert_eq!(included, ["/included"]);
    }

    #[test]
    fn reports_one_status_per_exit_in_force_with_its_apps() {
        let mut settings = routed(SplitMode::Exclude);
        settings
            .set_app_exit(app("/also-routed"), exit("de"))
            .unwrap();
        let ip: IpAddr = "203.0.113.5".parse().unwrap();

        let statuses = settings.route_statuses(|_| (AppRouteState::Connected, Some(ip)));

        assert_eq!(
            statuses,
            vec![AppRouteStatus {
                exit: exit("de"),
                state: AppRouteState::Connected,
                public_ip: Some(ip),
                apps: vec![app("/also-routed"), app("/routed")],
            }]
        );
    }

    #[test]
    fn serializes_to_the_documented_shape() {
        let mut settings = AppRoutingSettings {
            split_mode: SplitMode::IncludeOnly,
            app_exits_enabled: true,
            ..Default::default()
        };
        settings.included_apps.insert(app("/usr/bin/curl"));
        settings
            .set_app_exit(
                app("/usr/bin/curl"),
                ExitChoice::new("se", Some("got")).unwrap(),
            )
            .unwrap();

        let json = serde_json::to_value(&settings).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "split_mode": "include_only",
                "excluded_apps": [],
                "included_apps": ["/usr/bin/curl"],
                "app_exits_enabled": true,
                "app_exits": { "/usr/bin/curl": { "country": "se", "city": "got" } },
            })
        );
        assert_eq!(
            serde_json::from_value::<AppRoutingSettings>(json).unwrap(),
            settings
        );
    }
}
