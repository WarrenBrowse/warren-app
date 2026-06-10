use crate::{
    access_method,
    constraints::Constraint,
    custom_list::CustomListsSettings,
    relay_constraints::{
        GeographicLocationConstraint, LocationConstraint, ObfuscationSettings, RelayConstraints,
        RelayOverride, RelaySettings, RelaySettingsFormatter, SelectedObfuscation,
        WireguardConstraints,
    },
    wireguard,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(any(windows, target_os = "android", target_os = "macos"))]
use std::collections::HashSet;
use talpid_types::net::GenericTunnelOptions;

mod dns;

/// The version used by the current version of the code. Should always be the
/// latest version that exists in `SettingsVersion`.
/// This should be bumped when a new version is introduced along with a migration
/// being added to `mullvad-daemon`.
pub const CURRENT_SETTINGS_VERSION: SettingsVersion = SettingsVersion::V16;

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Copy)]
#[repr(u32)]
pub enum SettingsVersion {
    V2 = 2,
    V3 = 3,
    V4 = 4,
    V5 = 5,
    V6 = 6,
    V7 = 7,
    V8 = 8,
    V9 = 9,
    V10 = 10,
    V11 = 11,
    V12 = 12,
    V13 = 13,
    V14 = 14,
    V15 = 15,
    V16 = 16,
}

impl<'de> Deserialize<'de> for SettingsVersion {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match <u32>::deserialize(deserializer)? {
            v if v == SettingsVersion::V2 as u32 => Ok(SettingsVersion::V2),
            v if v == SettingsVersion::V3 as u32 => Ok(SettingsVersion::V3),
            v if v == SettingsVersion::V4 as u32 => Ok(SettingsVersion::V4),
            v if v == SettingsVersion::V5 as u32 => Ok(SettingsVersion::V5),
            v if v == SettingsVersion::V6 as u32 => Ok(SettingsVersion::V6),
            v if v == SettingsVersion::V7 as u32 => Ok(SettingsVersion::V7),
            v if v == SettingsVersion::V8 as u32 => Ok(SettingsVersion::V8),
            v if v == SettingsVersion::V9 as u32 => Ok(SettingsVersion::V9),
            v if v == SettingsVersion::V10 as u32 => Ok(SettingsVersion::V10),
            v if v == SettingsVersion::V11 as u32 => Ok(SettingsVersion::V11),
            v if v == SettingsVersion::V12 as u32 => Ok(SettingsVersion::V12),
            v if v == SettingsVersion::V13 as u32 => Ok(SettingsVersion::V13),
            v if v == SettingsVersion::V14 as u32 => Ok(SettingsVersion::V14),
            v if v == SettingsVersion::V15 as u32 => Ok(SettingsVersion::V15),
            v if v == SettingsVersion::V16 as u32 => Ok(SettingsVersion::V16),
            v => Err(serde::de::Error::custom(format!(
                "{v} is not a valid SettingsVersion"
            ))),
        }
    }
}

impl Serialize for SettingsVersion {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(*self as u32)
    }
}

/// Mullvad daemon settings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub relay_settings: RelaySettings,
    pub obfuscation_settings: ObfuscationSettings,
    /// All of the custom relay lists
    pub custom_lists: CustomListsSettings,
    /// API access methods
    pub api_access_methods: access_method::Settings,
    // If the default location in `relay_settings` should be updated based on the user's geolocation.
    pub update_default_location: bool,
    /// If the daemon should allow communication with private (LAN) networks.
    pub allow_lan: bool,
    /// Extra level of kill switch. When this setting is on, the disconnected state will block
    /// the firewall to not allow any traffic in or out.
    #[cfg(not(target_os = "android"))]
    pub lockdown_mode: bool,
    /// If the daemon should connect the VPN tunnel directly on start or not.
    pub auto_connect: bool,
    /// Options that should be applied to tunnels of a specific type regardless of where the relays
    /// might be located.
    pub tunnel_options: TunnelOptions,
    /// Overrides for relays
    pub relay_overrides: Vec<RelayOverride>,
    /// Whether to notify users of beta updates.
    pub show_beta_releases: bool,
    /// Split tunneling settings
    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    pub split_tunnel: SplitTunnelSettings,
    /// Specifies settings schema version
    pub settings_version: SettingsVersion,
    /// Stores the user's recently connected locations. If None recents have been disabled by the user.
    pub recents: Option<Vec<Recent>>,
    /// A randomly generated number used as input when determining if the client should update. Note that this
    /// number is not solely responsible for determining _when_ the client should be updated, but
    /// it is expected to be fairly unique.
    ///
    /// This is an Option to make the Default implementation deterministic.
    #[cfg(not(target_os = "android"))]
    pub rollout_threshold_seed: Option<u32>,
    /// URL of the warren-api server used by the
    /// `WarrenRemote{Account,Device}Backend`.
    ///
    /// Expected format: `http(s)://host:port` without trailing slash, e.g.
    /// `https://api.warrenbrowse.com` or `http://127.0.0.1:8080`.
    ///
    /// `None` = no warren-remote mode -> fallback to `RemoteAccountBackend`
    /// (legacy upstream Mullvad path via `api.mullvad.net`). Can be
    /// overridden via env var `WARREN_API_URL` (takes priority over Settings).
    #[serde(default)]
    pub warren_api_url: Option<String>,
    /// Number of parallel QUIC connections for the Warren tunnel.
    /// `None` resolves to the compiled default (8, cf. warren-core
    /// `m3e-multi-conn-sweep`: the throughput curve plateaus at N=8).
    /// Valid range 1..=16; out-of-range persisted values fall back to
    /// the default at parameter-production time. Can be overridden via
    /// env var `WARREN_N_CONNECTIONS` (takes priority over Settings).
    #[serde(default)]
    pub warren_n_connections: Option<u8>,
    /// Warren two-relayed QUIC multi-hop settings (M4.E.D stack).
    /// Default = OFF per doctrine `warren_multihop_doctrine_v1`
    /// (opt-in privacy, full bandwidth single-hop). The env var
    /// `WARREN_MULTI_HOP=1` overrides this for POC.
    #[serde(default)]
    pub warren_multi_hop: WarrenMultiHopSettings,
    /// Warren NAT-PMP port-forwarding settings. Default OFF; the
    /// daemon-side `NatPmpManager` only spawns a refresh loop when
    /// `enabled = true`. Differentiator product surface (Mullvad and
    /// IVPN dropped port-forwarding in 2023).
    #[serde(default)]
    pub warren_nat_pmp: WarrenNatPmpSettings,
    /// Warren TOFU pinning of exit Ed25519 pubkeys. Populated on first
    /// connect to a given exit. Subsequent connects with a divergent
    /// pubkey for the same exit identity surface a mismatch event to
    /// the UI and refuse the connection until the user acknowledges
    /// the change ("Trust new key") or resets the pin table.
    ///
    /// See `.planning/a4-pubkey-pinning-design.md` for the design
    /// blueprint - in particular, the open question of the stable
    /// `exit_id` field at the warren-core/backend level (deferred).
    #[serde(default)]
    pub warren_pinned_exit_pubkeys: WarrenPinnedExitPubkeys,
}

/// Warren two-relayed QUIC multi-hop settings (M4.E.D). Persisted in
/// [`Settings::warren_multi_hop`] and surfaced via the
/// `GetWarrenMultiHopSettings` gRPC rpc. The `entry_country` and
/// `exit_country` are ISO 3166 alpha-2 codes; empty string means
/// auto-pick from the relay list.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WarrenMultiHopSettings {
    /// Toggle ON/OFF. Default `false` per doctrine.
    pub enabled: bool,
    /// ISO 3166 alpha-2 entry country code. Empty = auto-pick.
    pub entry_country: String,
    /// ISO 3166 alpha-2 exit country code. Empty = auto-pick.
    pub exit_country: String,
    /// HPKE epoch rotation interval, capped to 8h by warren-core
    /// doctrine. Default 4h matches `warren_multihop_doctrine_v1`.
    pub hpke_epoch_rotation: std::time::Duration,
}

impl Default for WarrenMultiHopSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            entry_country: String::new(),
            exit_country: String::new(),
            hpke_epoch_rotation: std::time::Duration::from_secs(4 * 60 * 60),
        }
    }
}

/// Warren TOFU pinning store for exit Ed25519 pubkeys. Persisted as
/// part of [`Settings`].
///
/// Storage key is the exit identifier (`exit_id_hex` - currently the
/// 32-byte Ed25519 pubkey hex per /v1 limitation; the design doc
/// recommends a future stable 128-bit `exit_id` field added to the
/// signed warren-core relay list, see
/// `.planning/a4-pubkey-pinning-design.md`).
///
/// Empty = no pins yet (all exits get pinned on first connect, TOFU
/// pattern). Reset via the `ResetPinnedExitKeys` gRPC RPC.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct WarrenPinnedExitPubkeys {
    /// Ordered map for deterministic JSON serialisation. The key is a
    /// lower-case hex string (currently the pubkey itself; switches to
    /// the stable 128-bit `exit_id` once warren-core wires that field).
    pub entries: std::collections::BTreeMap<String, WarrenPinnedExitPubkey>,
}

/// One entry in [`WarrenPinnedExitPubkeys`]. Stores the pubkey first
/// seen for a given exit identifier plus the first/last observation
/// timestamps for forensic context.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WarrenPinnedExitPubkey {
    /// Hex-encoded 32-byte Ed25519 verifying key the user's daemon
    /// has trusted for this exit identity.
    pub pubkey_hex: String,
    /// Unix timestamp (seconds) when this pin was first established.
    pub first_seen_unix: u64,
    /// Unix timestamp (seconds) of the most recent successful match.
    /// Bumped on every reconnect where the observed pubkey matches
    /// `pubkey_hex`. Lets the UI surface staleness ("you last
    /// connected to this exit X days ago").
    pub last_seen_unix: u64,
    /// Optional cached forensic context: ISO 3166 alpha-2 country
    /// code + city as advertised by the relay list at the moment of
    /// pinning. Carried so the UI can show "this used to be FR Paris"
    /// even after the entry is removed from the active relay list.
    /// Empty strings on first introduction if the daemon does not know
    /// the location yet.
    #[serde(default)]
    pub country_code: String,
    #[serde(default)]
    pub city: String,
}

/// Transport protocol selector for NAT-PMP port-forwarding. Stored on
/// disk as a string discriminant to keep the JSON settings forward-
/// compatible (adding a `Both` variant later does not need a numeric
/// migration). Matches the RFC 6886 opcode mapping (UDP = 1, TCP = 2).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub enum WarrenNatPmpProto {
    /// UDP mapping (RFC 6886 opcode 1).
    #[default]
    Udp,
    /// TCP mapping (RFC 6886 opcode 2).
    Tcp,
}

/// One NAT-PMP port-forward rule. A client may hold several of these
/// simultaneously, up to the exit-enforced per-client quota
/// (`warren_config::NATPMP_QUOTA_PER_CLIENT_IP`).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WarrenNatPmpRule {
    /// Transport protocol (UDP or TCP).
    pub protocol: WarrenNatPmpProto,
    /// Suggested external port (0 = server picks from its pool).
    pub suggested_external_port: u16,
    /// Internal port the user's application binds (0 = unset; the exit
    /// then DNATs the granted external port to the same number on the
    /// client - the "same port on your device" model).
    pub internal_port: u16,
}

/// Warren NAT-PMP port-forwarding settings. Persisted in
/// [`Settings::warren_nat_pmp`] and surfaced via the
/// `GetNatPmpSettings` gRPC rpc. Default OFF; lifetime defaults to 1h
/// (the exit-side allocator clamps to 60..3600 s).
///
/// Multi-port model: [`Self::rules`] is the source of truth. The legacy
/// single-port fields (`protocol` / `suggested_external_port` /
/// `internal_port`) are retained ONLY so a settings.json written by a
/// pre-multi-port build still deserializes and its one forward is
/// preserved on upgrade (see [`Self::effective_rules`]). New writes
/// populate `rules` and leave the legacy fields at their defaults.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WarrenNatPmpSettings {
    /// Toggle ON/OFF. Default `false`.
    pub enabled: bool,
    /// Lifetime in seconds. Default 3600 (1 hour); the exit-side
    /// allocator clamps to its [60, 3600] range so larger values are
    /// silently capped server-side.
    pub lifetime_secs: u32,
    /// The set of port-forward rules the user wants active. Empty +
    /// `enabled` falls back to a single rule synthesized from the legacy
    /// fields (upgrade path). Capped client- and exit-side by the quota.
    #[serde(default)]
    pub rules: Vec<WarrenNatPmpRule>,
    /// Legacy single-port protocol. Kept for backward-compatible
    /// deserialization of old settings; superseded by `rules`.
    #[serde(default)]
    pub protocol: WarrenNatPmpProto,
    /// Legacy single-port suggested external port (see `protocol`).
    #[serde(default)]
    pub suggested_external_port: u16,
    /// Legacy single-port internal port (see `protocol`).
    #[serde(default)]
    pub internal_port: u16,
}

impl Default for WarrenNatPmpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            lifetime_secs: 3600,
            rules: Vec::new(),
            protocol: WarrenNatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 0,
        }
    }
}

impl WarrenNatPmpSettings {
    /// The effective list of port-forward rules to apply.
    ///
    /// New (multi-port) settings carry them in `rules`. A settings.json
    /// from a pre-multi-port build has `rules` empty but the legacy
    /// single-port fields set - we synthesize one rule from those so the
    /// existing forward survives the upgrade. A fresh default (disabled,
    /// no rules) yields an empty list.
    #[must_use]
    pub fn effective_rules(&self) -> Vec<WarrenNatPmpRule> {
        if !self.rules.is_empty() {
            return self.rules.clone();
        }
        // Legacy single-port fallback: only meaningful when something was
        // actually configured (a non-zero internal/external port). A
        // pristine default would otherwise synthesize a useless 0/0 rule.
        if self.internal_port != 0 || self.suggested_external_port != 0 {
            return vec![WarrenNatPmpRule {
                protocol: self.protocol,
                suggested_external_port: self.suggested_external_port,
                internal_port: self.internal_port,
            }];
        }
        Vec::new()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Recent {
    Singlehop(LocationConstraint),
    Multihop {
        entry: LocationConstraint,
        exit: LocationConstraint,
    },
}

impl TryFrom<&RelaySettings> for Recent {
    type Error = &'static str;

    fn try_from(value: &RelaySettings) -> Result<Self, Self::Error> {
        match value {
            RelaySettings::CustomTunnelEndpoint(_) => {
                Err("Cannot convert CustomTunnelEndpoint to Recent")
            }
            RelaySettings::Normal(constraints) => {
                let location = constraints
                    .location
                    .as_ref()
                    .option()
                    .ok_or("Location must be Constraint::Only")?
                    .clone();

                let recent = if constraints.wireguard_constraints.use_multihop {
                    let entry = constraints
                        .wireguard_constraints
                        .entry_location
                        .as_ref()
                        .option()
                        .ok_or("Location must be Constraint::Only")?
                        .clone();

                    if matches!(
                        entry,
                        LocationConstraint::Location(GeographicLocationConstraint::Hostname(..))
                    ) && matches!(
                        location,
                        LocationConstraint::Location(GeographicLocationConstraint::Hostname(..))
                    ) && entry == location
                    {
                        return Err(
                            "Multihop recent cannot have identical (country, city, host) triple.",
                        );
                    }

                    Recent::Multihop {
                        entry,
                        exit: location,
                    }
                } else {
                    Recent::Singlehop(location)
                };

                Ok(recent)
            }
        }
    }
}

#[cfg(any(windows, target_os = "android", target_os = "macos"))]
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SplitTunnelSettings {
    /// Toggles split tunneling on or off
    pub enable_exclusions: bool,
    /// Set of applications to exclude from the tunnel.
    pub apps: HashSet<SplitApp>,
}

/// An application whose traffic should be excluded from any active tunnel.
#[cfg(any(windows, target_os = "macos"))]
#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SplitApp(std::path::PathBuf);

/// An application whose traffic should be excluded from any active tunnel.
#[cfg(target_os = "android")]
#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SplitApp(String);

#[cfg(any(windows, target_os = "macos"))]
impl SplitApp {
    /// Convert the underlying path to a [`String`].
    /// This function will fail if the underlying path string is not valid UTF-8. See
    /// [`std::ffi::OsStr::to_str`] for details.
    pub fn to_string(self) -> Option<String> {
        self.0.as_os_str().to_str().map(str::to_string)
    }

    /// This is the String-representation as expected by `TunnelCommand::SetExcludedApps`
    pub fn to_tunnel_command_repr(self) -> std::ffi::OsString {
        self.0.as_os_str().to_owned()
    }

    pub fn display(&self) -> std::path::Display<'_> {
        self.0.display()
    }
}

#[cfg(target_os = "android")]
impl SplitApp {
    /// Convert the underlying app name to a [`String`].
    ///
    /// # Note
    /// This function is fallible due to the Window's dito being fallible, and it is convenient to
    /// have the same API across all platforms.
    pub fn to_string(self) -> Option<String> {
        Some(self.0)
    }

    /// This is the String-representation as expected by [`SetExcludedApps`].
    pub fn to_tunnel_command_repr(self) -> String {
        self.0
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl From<String> for SplitApp {
    fn from(value: String) -> Self {
        SplitApp::from(std::path::PathBuf::from(value))
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl From<std::path::PathBuf> for SplitApp {
    fn from(value: std::path::PathBuf) -> Self {
        SplitApp(value)
    }
}

#[cfg(target_os = "android")]
impl From<String> for SplitApp {
    fn from(value: String) -> Self {
        SplitApp(value)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            relay_settings: RelaySettings::Normal(RelayConstraints {
                location: Constraint::Only(LocationConstraint::Location(
                    GeographicLocationConstraint::Country("se".to_owned()),
                )),
                wireguard_constraints: WireguardConstraints {
                    entry_location: Constraint::Only(LocationConstraint::Location(
                        GeographicLocationConstraint::Country("se".to_owned()),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            }),
            // We only want to set this flag to true if the settings file hasn't been
            // created yet so that we don't affect existing users' relay settings.
            update_default_location: true,
            obfuscation_settings: ObfuscationSettings {
                selected_obfuscation: SelectedObfuscation::Auto,
                ..Default::default()
            },
            custom_lists: CustomListsSettings::default(),
            api_access_methods: access_method::Settings::default(),
            allow_lan: false,
            #[cfg(not(target_os = "android"))]
            lockdown_mode: false,
            auto_connect: false,
            tunnel_options: TunnelOptions::default(),
            relay_overrides: vec![],
            show_beta_releases: false,
            #[cfg(any(windows, target_os = "android", target_os = "macos"))]
            split_tunnel: SplitTunnelSettings::default(),
            settings_version: CURRENT_SETTINGS_VERSION,
            recents: Some(vec![]),
            #[cfg(not(target_os = "android"))]
            rollout_threshold_seed: None,
            // `None` here resolves to the compiled production default
            // (`warren_remote_config::DEFAULT_WARREN_API_URL`) at boot,
            // so the remote backend works without any manual `api-url
            // set`.
            warren_api_url: None,
            warren_n_connections: None,
            warren_multi_hop: WarrenMultiHopSettings::default(),
            warren_nat_pmp: WarrenNatPmpSettings::default(),
            warren_pinned_exit_pubkeys: WarrenPinnedExitPubkeys::default(),
        }
    }
}

impl Settings {
    /// The max number of recent entries that should be saved. When this number is exceeded the
    /// oldest recent is deleted.
    const RECENTS_MAX_COUNT: usize = 50;

    pub fn get_relay_settings(&self) -> RelaySettings {
        self.relay_settings.clone()
    }

    pub fn set_relay_settings(&mut self, new_settings: RelaySettings) {
        if self.relay_settings != new_settings {
            log::debug!(
                "Changing relay settings:\n\tfrom: {}\n\tto: {}",
                RelaySettingsFormatter {
                    settings: &self.relay_settings,
                    custom_lists: &self.custom_lists,
                },
                RelaySettingsFormatter {
                    settings: &new_settings,
                    custom_lists: &self.custom_lists,
                },
            );

            self.relay_settings = new_settings;
        }
    }

    pub fn set_relay_override(&mut self, relay_override: RelayOverride) {
        let existing_override = self
            .relay_overrides
            .iter_mut()
            .enumerate()
            .find(|(_, elem)| elem.hostname == relay_override.hostname);
        match existing_override {
            None => self.relay_overrides.push(relay_override),
            Some((index, elem)) => {
                if relay_override.is_empty() {
                    self.relay_overrides.swap_remove(index);
                } else {
                    *elem = relay_override;
                }
            }
        }
    }

    // Add the current RelaySettings to the recents list. If recents are disabled do nothing.
    pub fn update_recents(&mut self) {
        if let Some(recents) = self.recents.as_mut() {
            match Recent::try_from(&self.relay_settings) {
                Ok(new_recent) => {
                    recents.retain(|r| *r != new_recent);
                    recents.insert(0, new_recent);
                    recents.truncate(Self::RECENTS_MAX_COUNT);
                }
                Err(e) => {
                    log::debug!("Failed to convert {:?} to recent: {e}", recents);
                }
            }
        }
    }
}

/// TunnelOptions holds configuration data that applies to all kinds of tunnels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TunnelOptions {
    /// Contains wireguard tunnel options.
    pub wireguard: wireguard::TunnelOptions,

    // TODO: should this still exist?
    /// Contains generic tunnel options that may apply to more than a single tunnel type.
    pub generic: GenericTunnelOptions,
    /// DNS options.
    pub dns_options: DnsOptions,
}

pub use dns::{CustomDnsOptions, DefaultDnsOptions, DnsOptions, DnsState};

impl Default for TunnelOptions {
    fn default() -> Self {
        TunnelOptions {
            wireguard: wireguard::TunnelOptions::default(),
            generic: GenericTunnelOptions {
                // Warren /v1 is IPv4-only: the exit allocates no IPv6
                // tunnel address by default, so IPv6 must be BLOCKED by
                // the firewall, not routed. Upstream Mullvad defaults
                // this `true` on macOS/Android because WireGuard is
                // dual-stack; on Warren that default leaks IPv6 traffic
                // out the physical interface (apps reach IPv6
                // destinations *outside* the tunnel while it looks
                // "connected"). Default off until /v2 ships in-tunnel
                // IPv6 end to end.
                enable_ipv6: false,
            },
            dns_options: DnsOptions::default(),
        }
    }
}

#[cfg(test)]
mod warren_pinned_exit_pubkeys_tests {
    use super::*;

    /// Anti-regression: an empty pin table round-trips through the
    /// settings JSON without leaking any field that would change the
    /// shape from build to build. Settings drift between rebuilds is
    /// a known cause of "user has to re-approve permissions" UX
    /// regressions on macOS.
    #[test]
    fn empty_pin_table_round_trip_json() {
        let pins = WarrenPinnedExitPubkeys::default();
        let json = serde_json::to_string(&pins).unwrap();
        assert_eq!(json, r#"{"entries":{}}"#);
        let back: WarrenPinnedExitPubkeys = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pins);
    }

    /// One pinned exit deserialises to the exact same `BTreeMap` it
    /// was serialised from. Sentinel against a future field-shape
    /// drift (e.g. someone renaming `pubkey_hex` to `pubkey`).
    #[test]
    fn one_pin_round_trip_json() {
        let mut pins = WarrenPinnedExitPubkeys::default();
        pins.entries.insert(
            "abcd0123".repeat(8),
            WarrenPinnedExitPubkey {
                pubkey_hex: "ed25519aa".repeat(7) + "ed25519a",
                first_seen_unix: 1747740000,
                last_seen_unix: 1747740300,
                country_code: "fr".into(),
                city: "Paris".into(),
            },
        );
        let json = serde_json::to_string(&pins).unwrap();
        let back: WarrenPinnedExitPubkeys = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pins);
    }

    /// `BTreeMap` preserves key order, which means the on-disk JSON is
    /// deterministic regardless of insertion order. Important for diff-
    /// based settings inspection by the operator (and for our own
    /// regression suite, which would flake otherwise).
    #[test]
    fn pin_table_json_is_key_ordered() {
        let mut pins = WarrenPinnedExitPubkeys::default();
        pins.entries.insert(
            "zzzz".into(),
            WarrenPinnedExitPubkey {
                pubkey_hex: "z".repeat(64),
                first_seen_unix: 1,
                last_seen_unix: 2,
                country_code: String::new(),
                city: String::new(),
            },
        );
        pins.entries.insert(
            "aaaa".into(),
            WarrenPinnedExitPubkey {
                pubkey_hex: "a".repeat(64),
                first_seen_unix: 3,
                last_seen_unix: 4,
                country_code: String::new(),
                city: String::new(),
            },
        );
        let json = serde_json::to_string(&pins).unwrap();
        let i_aaaa = json.find("\"aaaa\"").expect("aaaa key present");
        let i_zzzz = json.find("\"zzzz\"").expect("zzzz key present");
        assert!(
            i_aaaa < i_zzzz,
            "BTreeMap must serialise keys in lexicographic order, got: {json}"
        );
    }

    /// The `Settings::default` initializer must produce an empty pin
    /// table - never silently pre-populate with stale data that would
    /// poison the TOFU contract on first boot.
    #[test]
    fn settings_default_has_empty_pin_table() {
        let s = Settings::default();
        assert!(s.warren_pinned_exit_pubkeys.entries.is_empty());
    }
}

#[cfg(test)]
mod warren_settings_default_tests {
    use super::*;

    /// Warren /v1 carries IPv4 only. IPv6 must default OFF so the
    /// firewall BLOCKS it instead of letting it leak out the physical
    /// interface. Upstream Mullvad defaults this `true` on macOS/Android
    /// (WireGuard is dual-stack); inheriting that default silently leaks
    /// every IPv6 connection around the tunnel.
    #[test]
    fn default_blocks_ipv6_because_tunnel_is_ipv4_only() {
        let opts = TunnelOptions::default();
        assert!(
            !opts.generic.enable_ipv6,
            "Warren is IPv4-only in /v1; enable_ipv6 must default to false \
             so IPv6 is blocked, not leaked outside the tunnel"
        );
    }
}
