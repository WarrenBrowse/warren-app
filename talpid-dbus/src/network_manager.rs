//! NetworkManager is the one-stop-shop of network configuration on Linux.
use super::systemd_resolved;
pub use dbus::arg::{RefArg, Variant};
use dbus::{
    arg,
    blocking::{Proxy, SyncConnection, stdintf::org_freedesktop_dbus::Properties},
    message::MatchRule,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    net::IpAddr,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

const NM_BUS: &str = "org.freedesktop.NetworkManager";
const NM_MANAGER: &str = "org.freedesktop.NetworkManager";
const NM_MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";
const CONNECTIVITY_CHECK_KEY: &str = "ConnectivityCheckEnabled";

const NM_DNS_MANAGER: &str = "org.freedesktop.NetworkManager.DnsManager";
const NM_DNS_MANAGER_PATH: &str = "/org/freedesktop/NetworkManager/DnsManager";
const NM_DEVICE: &str = "org.freedesktop.NetworkManager.Device";

const NM_IP4_CONFIG: &str = "org.freedesktop.NetworkManager.IP4Config";
const NM_IP6_CONFIG: &str = "org.freedesktop.NetworkManager.IP6Config";
const DEVICE_READY_TIMEOUT: Duration = Duration::from_secs(15);
const RC_MANAGEMENT_MODE_KEY: &str = "RcManager";
const DNS_MODE_KEY: &str = "Mode";
const DNS_FIRST_PRIORITY: i32 = -2147483647;

const NM_DEVICE_STATE_IP_CHECK: u32 = 80;
const NM_DEVICE_STATE_SECONDARY: u32 = 90;
const NM_DEVICE_STATE_ACTIVATED: u32 = 100;

const NM_SETTINGS_INTERFACE: &str = "org.freedesktop.NetworkManager.Settings";
const NM_SETTINGS_CONNECTION_INTERFACE: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const NM_SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const NM_CONNECTION_ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";

const NM_ADD_CONNECTION_VOLATILE: u32 = 0x2;

const RPC_TIMEOUT: std::time::Duration = Duration::from_secs(3);

const DBUS_UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";

const MINIMUM_SUPPORTED_MAJOR_VERSION: u32 = 1;
const MINIMUM_SUPPORTED_MINOR_VERSION: u32 = 16;

const MAXIMUM_SUPPORTED_MAJOR_VERSION: u32 = 1;
const MAXIMUM_SUPPORTED_MINOR_VERSION: u32 = 26;

const NM_DEVICE_STATE_CHANGED: &str = "StateChanged";

/// NetworkManager gained the `auto-route-ext-gw` property in 1.42. Older
/// versions reject a connection carrying a property they do not know.
const AUTO_ROUTE_EXT_GW_SINCE: (u32, u32) = (1, 42);

pub type Result<T> = std::result::Result<T, Error>;
type NetworkSettings<'a> = HashMap<String, HashMap<String, Variant<Box<dyn RefArg + 'a>>>>;

/// What Warren lets NetworkManager do with the tunnel's IP configuration:
/// as little as it can be told to do.
///
/// The engine owns routing (the split default, the per-exit host routes)
/// and DNS. NetworkManager is only being told that a VPN exists, so that
/// the desktop can say so. Anything it applied on top of a live tunnel
/// would be a leak, not a cosmetic issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnIpPolicy {
    pub never_default: bool,
    pub ignore_auto_dns: bool,
    pub ignore_auto_routes: bool,
    /// `NMTernary` value for `auto-route-ext-gw`, left out entirely on a
    /// NetworkManager too old to know the property. Such a version then
    /// installs its own host route to the peer, which is the route the
    /// engine installs anyway, so the fallback is harmless.
    pub auto_route_ext_gw: Option<i32>,
}

pub fn vpn_ip_policy(nm_version: (u32, u32)) -> VpnIpPolicy {
    VpnIpPolicy {
        never_default: true,
        ignore_auto_dns: true,
        ignore_auto_routes: true,
        auto_route_ext_gw: (nm_version >= AUTO_ROUTE_EXT_GW_SINCE).then_some(0),
    }
}

/// A live NetworkManager VPN connection standing for the running tunnel.
#[derive(Debug)]
pub struct VpnIndicatorConnection {
    config_path: dbus::Path<'static>,
    connection_path: dbus::Path<'static>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error while communicating over Dbus")]
    Dbus(#[from] dbus::Error),

    #[error("Failed to match the returned D-Bus object with expected type")]
    MatchDBusTypeError(#[from] dbus::arg::TypeMismatchError),

    #[error(
        "NM is configured to manage DNS via systemd-resolved but systemd-resolved is not managing /etc/resolv.conf: {0}"
    )]
    SystemdResolvedNotManagingResolvconf(systemd_resolved::Error),

    #[error("Configuration has no device associated to it")]
    NoDevice,

    #[error("NetworkManager is too old: {0}.{1}")]
    NMTooOld(u32, u32),

    #[error("NetworkManager is too new to manage DNS: {0}.{1}")]
    NMTooNewFroDns(u32, u32),

    #[error("Failed to parse NetworkManager version string: {0}")]
    ParseNmVersionError(String),

    #[error("Device inactive: {0}")]
    DeviceNotReady(u32),

    #[error("Device not found")]
    DeviceNotFound,

    #[error("NetworkManager not detected")]
    NetworkManagerNotDetected,

    #[error("NetworkManager is using dnsmasq to manage DNS")]
    UsingDnsmasq,

    #[error("NetworkManager is too old: {0}")]
    TooOldNetworkManager(String),

    #[error("NetworkManager is not managing DNS")]
    NetworkManagerNotManagingDns,

    #[error("Failed to get devices from NetworkManager object")]
    ObtainDevices,
}

pub type VariantRefArg = Variant<Box<dyn RefArg>>;
pub type VariantMap = HashMap<String, VariantRefArg>;
// settings are a{sa{sv}}
pub type DeviceConfig = HashMap<String, VariantMap>;

/// Implements functionality to control NetworkManager over DBus.
pub struct NetworkManager {
    connection: Arc<SyncConnection>,
}

impl NetworkManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            connection: crate::get_connection()?,
        })
    }

    pub fn create_wg_tunnel(&self, config: &DeviceConfig) -> Result<WireguardTunnel> {
        self.nm_supports_wireguard()?;
        let tunnel = self.create_wg_tunnel_inner(config)?;
        if let Err(err) = self.wait_until_device_is_ready(&tunnel.device_path) {
            if let Err(removal_error) = self.remove_tunnel(tunnel) {
                log::error!(
                    "Failed to remove WireGuard tunnel after it not becoming ready fast enough: {}",
                    removal_error
                );
            }
            return Err(err);
        }

        Ok(tunnel)
    }

    /// Tell NetworkManager that `tundev` is a VPN reaching `gateway`, so
    /// the desktop's network indicator says a VPN is up.
    ///
    /// Purely declarative: the tunnel is already built, carries its own
    /// addresses and is routed by the engine. The plugin named by
    /// `service_type` answers NetworkManager with what the interface
    /// actually holds, and retracts the connection if the interface goes
    /// away.
    pub fn publish_vpn_indicator(
        &self,
        service_type: &str,
        id: &str,
        tundev: &str,
        gateway: IpAddr,
    ) -> Result<VpnIndicatorConnection> {
        let settings = self.vpn_indicator_settings(service_type, id, tundev, gateway)?;

        let config_path: dbus::Path<'static> = match self.add_connection_2(&settings) {
            Ok((path, _result)) => path,
            Err(Error::Dbus(dbus_error)) if dbus_error.name() == Some(DBUS_UNKNOWN_METHOD) => {
                self.add_connection_unsaved(&settings)?.0
            }
            Err(error) => return Err(error),
        };

        let activate: std::result::Result<(dbus::Path<'static>,), _> =
            self.nm_manager().method_call(
                NM_MANAGER,
                "ActivateConnection",
                (
                    &config_path,
                    &dbus::Path::new("/").unwrap(),
                    &dbus::Path::new("/").unwrap(),
                ),
            );

        match activate {
            Ok((connection_path,)) => Ok(VpnIndicatorConnection {
                config_path,
                connection_path,
            }),
            Err(error) => {
                // A profile left behind would sit in the user's connection
                // list forever, describing a tunnel that never came up.
                self.delete_connection_profile(&config_path);
                Err(Error::Dbus(error))
            }
        }
    }

    /// Take the VPN connection back down. Called when the tunnel ends, and
    /// best-effort: the plugin's own watchdog retracts the connection too,
    /// so a failure here costs at most a few seconds of stale indicator.
    pub fn withdraw_vpn_indicator(&self, indicator: VpnIndicatorConnection) -> Result<()> {
        let result: std::result::Result<(), dbus::Error> = self.nm_manager().method_call(
            NM_MANAGER,
            "DeactivateConnection",
            (&indicator.connection_path,),
        );
        // The profile is volatile, so NetworkManager drops it once it goes
        // inactive; the explicit delete covers the version that does not.
        self.delete_connection_profile(&indicator.config_path);
        result.map_err(Error::Dbus)
    }

    fn delete_connection_profile(&self, config_path: &dbus::Path<'static>) {
        let proxy = Proxy::new(NM_BUS, config_path, RPC_TIMEOUT, &*self.connection);
        let deleted: std::result::Result<(), dbus::Error> =
            proxy.method_call(NM_SETTINGS_CONNECTION_INTERFACE, "Delete", ());
        if let Err(error) = deleted {
            log::debug!("NetworkManager had already dropped the VPN profile: {error}");
        }
    }

    fn vpn_indicator_settings(
        &self,
        service_type: &str,
        id: &str,
        tundev: &str,
        gateway: IpAddr,
    ) -> Result<DeviceConfig> {
        let policy = vpn_ip_policy(self.version()?);

        let mut connection = VariantMap::new();
        connection.insert("id".to_string(), Variant(Box::new(id.to_string())));
        connection.insert("type".to_string(), Variant(Box::new("vpn".to_string())));
        // Only the daemon ever brings this up, alongside a tunnel it built.
        connection.insert("autoconnect".to_string(), Variant(Box::new(false)));

        let mut data = HashMap::new();
        data.insert("tundev".to_string(), tundev.to_string());
        data.insert("gateway".to_string(), gateway.to_string());

        let mut vpn = VariantMap::new();
        vpn.insert(
            "service-type".to_string(),
            Variant(Box::new(service_type.to_string())),
        );
        vpn.insert("data".to_string(), Variant(Box::new(data)));

        let mut settings = DeviceConfig::new();
        settings.insert("connection".to_string(), connection);
        settings.insert("vpn".to_string(), vpn);
        settings.insert("ipv4".to_string(), ip_settings(&policy));
        settings.insert("ipv6".to_string(), ip_settings(&policy));
        Ok(settings)
    }

    pub fn get_interface_name(&self, tunnel: &WireguardTunnel) -> Result<String> {
        tunnel
            .device_proxy(&self.connection)
            .get(NM_DEVICE, "Interface")
            .map_err(Error::Dbus)
    }

    pub fn get_device_state(&self, device: &dbus::Path<'_>) -> Result<u32> {
        self.as_path(device)
            .get(NM_DEVICE, "State")
            .map_err(Error::Dbus)
    }

    fn create_wg_tunnel_inner(&self, config: &DeviceConfig) -> Result<WireguardTunnel> {
        let config_path: dbus::Path<'static> = match self.add_connection_2(config) {
            Ok((path, _result)) => path,
            Err(Error::Dbus(dbus_error)) if dbus_error.name() == Some(DBUS_UNKNOWN_METHOD) => {
                self.add_connection_unsaved(config)?.0
            }
            Err(err) => {
                log::error!(
                    "Failed to create a new interface via AddConnection2: {}",
                    err
                );
                return Err(err);
            }
        };

        let manager = self.nm_manager();
        let (connection_path,): (dbus::Path<'static>,) = manager
            .method_call(
                NM_MANAGER,
                "ActivateConnection",
                (
                    &config_path,
                    &dbus::Path::new("/").unwrap(),
                    &dbus::Path::new("/").unwrap(),
                ),
            )
            .map_err(Error::Dbus)?;

        let connection = Proxy::new(NM_BUS, &connection_path, RPC_TIMEOUT, &*self.connection);
        let device_paths: Vec<dbus::Path<'static>> = connection
            .get(NM_CONNECTION_ACTIVE, "Devices")
            .map_err(Error::Dbus)?;
        let device_path = device_paths.into_iter().next().ok_or(Error::NoDevice)?;

        Ok(WireguardTunnel {
            config_path,
            connection_path,
            device_path,
        })
    }

    pub fn nm_supports_wireguard(&self) -> Result<()> {
        let (major, minor) = self.version()?;
        Self::ensure_nm_is_new_enough_for_wireguard(major, minor)?;
        Self::ensure_nm_is_old_enough_for_dns(major, minor)
    }

    pub fn nm_version_dns_works(&self) -> Result<()> {
        let (major, minor) = self.version()?;
        Self::ensure_nm_is_old_enough_for_dns(major, minor)
    }

    pub fn version_string(&self) -> Result<String> {
        let manager = self.nm_manager();
        manager.get(NM_MANAGER, "Version").map_err(Error::Dbus)
    }

    fn ensure_nm_is_new_enough_for_wireguard(major: u32, minor: u32) -> Result<()> {
        if major < MINIMUM_SUPPORTED_MAJOR_VERSION
            || (minor < MINIMUM_SUPPORTED_MINOR_VERSION && major == MINIMUM_SUPPORTED_MAJOR_VERSION)
        {
            Err(Error::NMTooOld(major, minor))
        } else {
            Ok(())
        }
    }

    fn ensure_nm_is_old_enough_for_dns(major_version: u32, minor_version: u32) -> Result<()> {
        if major_version > MAXIMUM_SUPPORTED_MAJOR_VERSION
            || (minor_version > MAXIMUM_SUPPORTED_MINOR_VERSION
                && major_version >= MAXIMUM_SUPPORTED_MAJOR_VERSION)
        {
            Err(Error::NMTooNewFroDns(major_version, minor_version))
        } else {
            Ok(())
        }
    }

    fn version(&self) -> Result<(u32, u32)> {
        let version = self.version_string()?;
        Self::parse_nm_version(&version).ok_or(Error::ParseNmVersionError(version))
    }

    fn parse_nm_version(version: &str) -> Option<(u32, u32)> {
        let mut parts = version.split('.').map(|part| part.parse().ok());

        let major_version: u32 = parts.next()??;
        let minor_version: u32 = parts.next()??;
        Some((major_version, minor_version))
    }

    fn add_connection_2(
        &self,
        settings_map: &DeviceConfig,
    ) -> Result<(dbus::Path<'static>, DeviceConfig)> {
        let args: VariantMap = HashMap::new();

        Proxy::new(NM_BUS, NM_SETTINGS_PATH, RPC_TIMEOUT, &*self.connection)
            .method_call(
                NM_SETTINGS_INTERFACE,
                "AddConnection2",
                (settings_map, NM_ADD_CONNECTION_VOLATILE, args),
            )
            .map_err(Error::Dbus)
    }

    fn add_connection_unsaved(
        &self,
        settings_map: &DeviceConfig,
    ) -> Result<(dbus::Path<'static>,)> {
        Proxy::new(NM_BUS, NM_SETTINGS_PATH, RPC_TIMEOUT, &*self.connection)
            .method_call(
                NM_SETTINGS_INTERFACE,
                "AddConnectionUnsaved",
                (settings_map,),
            )
            .map_err(Error::Dbus)
    }

    fn wait_until_device_is_ready(&self, device: &dbus::Path<'_>) -> Result<()> {
        let device_state = self.get_device_state(device)?;

        if !device_is_ready(device_state) {
            let deadline = Instant::now() + DEVICE_READY_TIMEOUT;

            let mut match_rule = MatchRule::new_signal(NM_DEVICE, NM_DEVICE_STATE_CHANGED);

            match_rule.path = Some(device.clone().into_static());
            let device_state = Arc::new(AtomicU32::new(device_state));

            {
                let shared_device_state = device_state.clone();
                let device_matcher = self
                    .connection
                    .add_match(
                        match_rule,
                        move |state_change: DeviceStateChange, _connection, _message| {
                            log::debug!("Received new tunnel state change: {:?}", state_change);
                            let new_state = state_change.new_state;
                            shared_device_state.store(new_state, Ordering::Release);
                            true
                        },
                    )
                    .map_err(Error::Dbus)?;
                while Instant::now() < deadline
                    && !device_is_ready(device_state.load(Ordering::Acquire))
                {
                    if let Err(err) = self.connection.process(RPC_TIMEOUT) {
                        log::error!(
                            "DBus connection failed while waiting for device to be ready: {}",
                            err
                        );
                    }
                }

                if let Err(err) = self.connection.remove_match(device_matcher) {
                    log::error!("Failed to remove match from DBus connection: {}", err);
                }
            }

            let final_device_state = device_state.load(Ordering::Acquire);
            if !device_is_ready(final_device_state) {
                return Err(Error::DeviceNotReady(final_device_state));
            }
        }
        Ok(())
    }

    pub fn remove_tunnel(&self, tunnel: WireguardTunnel) -> Result<()> {
        let deactivation_result: Result<()> = self
            .nm_manager()
            .method_call(
                NM_MANAGER,
                "DeactivateConnection",
                (&tunnel.connection_path,),
            )
            .map_err(Error::Dbus);

        let config_result: Result<()> = tunnel
            .config_proxy(&self.connection)
            .method_call(NM_SETTINGS_CONNECTION_INTERFACE, "Delete", ())
            .map_err(Error::Dbus);
        deactivation_result?;
        config_result?;
        Ok(())
    }

    /// Ensures NetworkManager's connectivity check is disabled and returns the connectivity check
    /// previous state. Returns true only if the connectivity check was enabled and is now
    /// disabled. Disabling the connectivity check should be done before a firewall is applied
    /// due to the fact that blocking DNS requests can make it hang:
    /// <https://gitlab.freedesktop.org/NetworkManager/NetworkManager/-/issues/404>
    pub fn disable_connectivity_check(&self) -> Option<bool> {
        let nm_manager = self.nm_manager();
        match nm_manager.get(NM_MANAGER, CONNECTIVITY_CHECK_KEY) {
            Ok(true) => {
                if let Err(err) = nm_manager.set(NM_MANAGER, CONNECTIVITY_CHECK_KEY, false) {
                    log::error!(
                        "Failed to disable NetworkManager connectivity check: {}",
                        err
                    );
                    Some(false)
                } else {
                    Some(true)
                }
            }
            Ok(false) => Some(false),
            Err(_) => None,
        }
    }

    /// Enabled NetworkManager's connectivity check. Fails silently.
    pub fn enable_connectivity_check(&self) {
        if let Err(err) = self
            .nm_manager()
            .set(NM_MANAGER, CONNECTIVITY_CHECK_KEY, true)
        {
            log::error!("Failed to reset NetworkManager connectivity check: {}", err);
        }
    }

    fn nm_manager(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(NM_BUS, NM_MANAGER_PATH, RPC_TIMEOUT, &*self.connection)
    }

    pub fn ensure_network_manager_exists(&self) -> Result<()> {
        match self
            .as_manager()
            .get::<Box<dyn RefArg>>(NM_MANAGER, "Version")
        {
            Ok(_) => Ok(()),
            Err(err) => {
                log::error!("Failed to read version of NetworkManager {}", err);
                Err(Error::NetworkManagerNotDetected)
            }
        }
    }

    pub fn ensure_can_be_used_to_manage_dns(&self) -> Result<()> {
        self.ensure_resolv_conf_is_managed()?;
        self.ensure_network_manager_exists()?;
        self.nm_version_dns_works()?;
        Ok(())
    }
    pub fn ensure_resolv_conf_is_managed(&self) -> Result<()> {
        // check if NM is set to manage resolv.conf
        let management_mode: String = self
            .as_dns_manager()
            .get(NM_DNS_MANAGER, RC_MANAGEMENT_MODE_KEY)?;
        if management_mode == "unmanaged" {
            return Err(Error::NetworkManagerNotManagingDns);
        }

        if management_mode == "systemd-resolved" {
            return match systemd_resolved::SystemdResolved::new() {
                Ok(_) => Ok(()),
                Err(err) => Err(Error::SystemdResolvedNotManagingResolvconf(err)),
            };
        }

        let dns_mode: String = self
            .as_dns_manager()
            .get(NM_DNS_MANAGER, DNS_MODE_KEY)
            .map_err(Error::Dbus)?;

        match dns_mode.as_ref() {
            // NM can't setup config for multiple interfaces with dnsmasq
            "dnsmasq" => return Err(Error::UsingDnsmasq),
            // If NetworkManager isn't managing DNS for us, it's useless.
            "none" => return Err(Error::NetworkManagerNotManagingDns),
            _ => (),
        };

        if !verify_etc_resolv_conf_contents() {
            log::debug!(
                "/etc/resolv.conf differs from reference resolv.conf, therefore NM is not managing DNS"
            );
            return Err(Error::NetworkManagerNotManagingDns);
        }

        Ok(())
    }

    fn as_manager(&'_ self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(NM_BUS, NM_MANAGER_PATH, RPC_TIMEOUT, &*self.connection)
    }

    fn as_dns_manager(&'_ self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(NM_BUS, NM_DNS_MANAGER_PATH, RPC_TIMEOUT, &*self.connection)
    }

    fn as_path<'a>(&'a self, device: &'a dbus::Path<'a>) -> Proxy<'a, &'a SyncConnection> {
        Proxy::new(NM_BUS, device, RPC_TIMEOUT, &*self.connection)
    }

    pub fn set_dns(&mut self, interface_name: &str, servers: &[IpAddr]) -> Result<DeviceConfig> {
        let device_path = self.fetch_device(interface_name)?;
        self.wait_until_device_is_ready(&device_path)?;

        let device = self.as_path(&device_path);
        // Get the last applied connection
        let (mut settings, version_id): (NetworkSettings<'_>, u64) =
            device.method_call(NM_DEVICE, "GetAppliedConnection", (0u32,))?;

        // Keep changed routes.
        // These routes were modified outside NM, likely by RouteManager.
        if let Some(ipv4_settings) = settings.get_mut("ipv4") {
            let device_ip4_config: dbus::Path<'_> =
                device.get(NM_DEVICE, "Ip4Config").map_err(Error::Dbus)?;

            let device_routes: Vec<Vec<u32>> = self
                .as_path(&device_ip4_config)
                .get(NM_IP4_CONFIG, "Routes")
                .map_err(Error::Dbus)?;

            let device_route_data: Vec<HashMap<String, Variant<Box<dyn RefArg>>>> = self
                .as_path(&device_ip4_config)
                .get(NM_IP4_CONFIG, "RouteData")
                .map_err(Error::Dbus)?;

            ipv4_settings.insert("route-metric".to_string(), Variant(Box::new(0u32)));
            ipv4_settings.insert("routes".to_string(), Variant(Box::new(device_routes)));
            ipv4_settings.insert(
                "route-data".to_string(),
                Variant(Box::new(device_route_data)),
            );
        }

        if let Some(ipv6_settings) = settings.get_mut("ipv6") {
            let device_ip6_config: dbus::Path<'_> =
                device.get(NM_DEVICE, "Ip6Config").map_err(Error::Dbus)?;

            let device_addresses6: Vec<(Vec<u8>, u32, Vec<u8>)> = self
                .as_path(&device_ip6_config)
                .get(NM_IP6_CONFIG, "Addresses")
                .map_err(Error::Dbus)?;

            let device_routes6: Vec<(Vec<u8>, u32, Vec<u8>, u32)> = self
                .as_path(&device_ip6_config)
                .get(NM_IP6_CONFIG, "Routes")
                .map_err(Error::Dbus)?;

            let device_route6_data: Vec<HashMap<String, Variant<Box<dyn RefArg>>>> = self
                .as_path(&device_ip6_config)
                .get(NM_IP6_CONFIG, "RouteData")
                .map_err(Error::Dbus)?;

            ipv6_settings.insert("route-metric".to_string(), Variant(Box::new(0u32)));
            ipv6_settings.insert("routes".to_string(), Variant(Box::new(device_routes6)));
            ipv6_settings.insert(
                "route-data".to_string(),
                Variant(Box::new(device_route6_data)),
            );
            // if the link contains link local addresses, addresses shouldn't be reset
            if ipv6_settings
                .get("method")
                .map(|method| {
                    // if IPv6 isn't enabled, IPv6 method will be set to "ignore", in which case we
                    // shouldn't reapply any config for ipv6
                    method.as_str() != Some("link-local") && method.as_str() != Some("ignore")
                })
                .unwrap_or(true)
            {
                ipv6_settings.insert(
                    "addresses".to_string(),
                    Variant(Box::new(device_addresses6)),
                );
            }
        }

        let mut settings_backup =
            HashMap::<String, HashMap<String, Variant<Box<dyn RefArg>>>>::new();
        for (top_key, map) in settings.iter() {
            let mut inner_dict = HashMap::<String, Variant<Box<dyn RefArg>>>::new();
            for (key, variant) in map.iter() {
                inner_dict.insert(key.clone(), Variant(variant.0.box_clone()));
            }
            settings_backup.insert(top_key.clone(), inner_dict);
        }

        // Update the DNS config
        let v4_dns: Vec<u32> = servers
            .iter()
            .filter_map(|server| {
                match server {
                    // Network-byte order
                    IpAddr::V4(server) => Some(u32::to_be((*server).into())),
                    IpAddr::V6(_) => None,
                }
            })
            .collect();
        if !v4_dns.is_empty() {
            Self::update_dns_config(&mut settings, "ipv4", v4_dns);
        }

        let v6_dns: Vec<Vec<u8>> = servers
            .iter()
            .filter_map(|server| match server {
                IpAddr::V4(_) => None,
                IpAddr::V6(server) => Some(server.octets().to_vec()),
            })
            .collect();
        if !v6_dns.is_empty() {
            Self::update_dns_config(&mut settings, "ipv6", v6_dns);
        }

        if let Some(wg_config) = settings.get_mut("wireguard")
            && !wg_config.contains_key("fwmark")
        {
            log::error!("WireGuard config doesn't contain the firewall mark");
        }

        self.reapply_settings(&device_path, settings, version_id)?;
        Ok(settings_backup)
    }

    pub fn reapply_settings<Settings: arg::Append>(
        &self,
        device: &dbus::Path<'_>,
        settings: Settings,
        version_id: u64,
    ) -> Result<()> {
        self.as_path(device).method_call::<(), _, _, _>(
            NM_DEVICE,
            "Reapply",
            (settings, version_id, 0u32),
        )?;
        Ok(())
    }

    fn update_dns_config<'a, T>(
        settings: &mut NetworkSettings<'a>,
        ip_protocol: &'static str,
        servers: T,
    ) where
        T: RefArg + 'a,
    {
        let settings = match settings.get_mut(ip_protocol) {
            Some(ip_protocol) => ip_protocol,
            None => {
                settings.insert(ip_protocol.to_string(), HashMap::new());
                settings.get_mut(ip_protocol).unwrap()
            }
        };

        settings.insert(
            "method".to_string(),
            Variant(Box::new("manual".to_string())),
        );
        settings.insert(
            "dns-priority".to_string(),
            Variant(Box::new(DNS_FIRST_PRIORITY)),
        );
        settings.insert("dns".to_string(), Variant(Box::new(servers)));
        settings.insert(
            "dns-search".to_string(),
            Variant(Box::new(vec!["~.".to_string()])),
        );
    }

    pub fn fetch_device(&self, interface_name: &str) -> Result<dbus::Path<'static>> {
        let devices: Box<dyn RefArg> = self
            .as_manager()
            .get(NM_MANAGER, "Devices")
            .map_err(Error::Dbus)?;
        let iter = devices
            .as_iter()
            .ok_or(Error::ObtainDevices)?
            .map(|device| device.box_clone());

        for device_item in iter {
            // Copy due to lifetime weirdness
            let device_path = device_item
                .as_any()
                .downcast_ref::<dbus::Path<'_>>()
                .ok_or(Error::ObtainDevices)?;

            let device_name: String = self
                .as_path(device_path)
                .get(NM_DEVICE, "Interface")
                .map_err(Error::Dbus)?;

            if device_name != interface_name {
                continue;
            }

            return Ok(device_path.clone());
        }
        Err(Error::DeviceNotFound)
    }

    pub fn convert_address_to_dbus(address: &IpAddr) -> VariantMap {
        let mut map: VariantMap = HashMap::new();
        map.insert(
            "address".to_string(),
            Variant(Box::new(address.to_string())),
        );
        let prefix: u32 = if address.is_ipv4() { 32 } else { 128 };
        map.insert("prefix".to_string(), Variant(Box::new(prefix)));

        map
    }
}

#[derive(Debug)]
struct DeviceStateChange {
    new_state: u32,
    _old_state: u32,
    _reason: u32,
}

impl arg::ReadAll for DeviceStateChange {
    fn read(i: &mut arg::Iter<'_>) -> std::result::Result<Self, arg::TypeMismatchError> {
        Ok(DeviceStateChange {
            new_state: i.read()?,
            _old_state: i.read()?,
            _reason: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for DeviceStateChange {
    const NAME: &'static str = NM_DEVICE_STATE_CHANGED;
    const INTERFACE: &'static str = NM_DEVICE;
}

#[derive(Debug)]
pub struct WireguardTunnel {
    config_path: dbus::Path<'static>,
    connection_path: dbus::Path<'static>,
    device_path: dbus::Path<'static>,
}

impl WireguardTunnel {
    fn device_proxy<'a>(&'a self, connection: &'a SyncConnection) -> Proxy<'a, &'a SyncConnection> {
        Proxy::new(NM_BUS, &self.device_path, RPC_TIMEOUT, connection)
    }

    fn config_proxy<'a>(&'a self, connection: &'a SyncConnection) -> Proxy<'a, &'a SyncConnection> {
        Proxy::new(NM_BUS, &self.config_path, RPC_TIMEOUT, connection)
    }
}

/// The per-family IP settings carrying [`VpnIpPolicy`]. `auto` is the
/// method a VPN connection must use: the plugin is what supplies the
/// addresses, and every other knob here tells NetworkManager to keep its
/// hands off what the plugin reports.
fn ip_settings(policy: &VpnIpPolicy) -> VariantMap {
    let mut settings = VariantMap::new();
    settings.insert("method".to_string(), Variant(Box::new("auto".to_string())));
    settings.insert(
        "never-default".to_string(),
        Variant(Box::new(policy.never_default)),
    );
    settings.insert(
        "ignore-auto-dns".to_string(),
        Variant(Box::new(policy.ignore_auto_dns)),
    );
    settings.insert(
        "ignore-auto-routes".to_string(),
        Variant(Box::new(policy.ignore_auto_routes)),
    );
    if let Some(ternary) = policy.auto_route_ext_gw {
        settings.insert("auto-route-ext-gw".to_string(), Variant(Box::new(ternary)));
    }
    settings
}

pub fn device_is_ready(device_state: u32) -> bool {
    /// Any state above `NM_DEVICE_STATE_IP_CONFIG` is considered to be an OK state to change the
    /// DNS config. For the enums, see https://developer.gnome.org/NetworkManager/stable/nm-dbus-types.html#NMDeviceState
    const READY_STATES: [u32; 3] = [
        NM_DEVICE_STATE_IP_CHECK,
        NM_DEVICE_STATE_SECONDARY,
        NM_DEVICE_STATE_ACTIVATED,
    ];
    READY_STATES.contains(&device_state)
}

// Verify that the contents of /etc/resolv.conf match what NM expects them to be.
fn verify_etc_resolv_conf_contents() -> bool {
    let expected_resolv_conf = "/var/run/NetworkManager/resolv.conf";
    let actual_resolv_conf = "/etc/resolv.conf";
    eq_file_content(&expected_resolv_conf, &actual_resolv_conf)
}

fn eq_file_content<P: AsRef<Path>>(a: &P, b: &P) -> bool {
    let file_a = match File::open(a).map(BufReader::new) {
        Ok(file) => file,
        Err(e) => {
            log::debug!("Failed to open file {}: {}", a.as_ref().display(), e);
            return false;
        }
    };
    let file_b = match File::open(b).map(BufReader::new) {
        Ok(file) => file,
        Err(e) => {
            log::debug!("Failed to open file {}: {}", b.as_ref().display(), e);
            return false;
        }
    };

    !file_a
        .lines()
        .zip(file_b.lines())
        .any(|(a, b)| match (a, b) {
            (Ok(a), Ok(b)) => a != b,
            _ => false,
        })
}

#[cfg(test)]
mod test {
    use super::*;

    /// `NM_VPN_CONNECTION_STATE_ACTIVATED`, the only state the desktop
    /// shows a VPN for.
    const NM_VPN_CONNECTION_STATE_ACTIVATED: u32 = 5;

    #[test]
    fn test_valid_versions() {
        NetworkManager::ensure_nm_is_new_enough_for_wireguard(1, 16).unwrap();
        NetworkManager::ensure_nm_is_old_enough_for_dns(1, 26).unwrap();
        assert!(NetworkManager::ensure_nm_is_new_enough_for_wireguard(1, 14).is_err());
        assert!(NetworkManager::ensure_nm_is_old_enough_for_dns(1, 28).is_err());
    }

    /// Whatever the NetworkManager version, the indicator connection must
    /// never be allowed to touch routing or DNS: the engine owns both, and
    /// a default route or a resolver coming from NetworkManager on top of
    /// the live tunnel is a leak.
    #[test]
    fn the_indicator_connection_never_takes_over_routing_or_dns() {
        for version in [(1, 20), (1, 41), (1, 42), (1, 46), (2, 0)] {
            let policy = vpn_ip_policy(version);
            assert!(policy.never_default, "NM {version:?} would route traffic");
            assert!(policy.ignore_auto_dns, "NM {version:?} would set DNS");
            assert!(policy.ignore_auto_routes, "NM {version:?} would add routes");
        }
    }

    /// `auto-route-ext-gw` landed in NetworkManager 1.42. Sending an
    /// unknown property makes NetworkManager reject the whole connection,
    /// so an older NetworkManager must not be offered it: it then adds its
    /// own host route to the peer, which is what the engine installs
    /// anyway.
    #[test]
    fn only_offers_auto_route_ext_gw_to_a_network_manager_that_knows_it() {
        assert_eq!(vpn_ip_policy((1, 41)).auto_route_ext_gw, None);
        assert_eq!(vpn_ip_policy((1, 42)).auto_route_ext_gw, Some(0));
        assert_eq!(vpn_ip_policy((1, 46)).auto_route_ext_gw, Some(0));
        assert_eq!(vpn_ip_policy((2, 0)).auto_route_ext_gw, Some(0));
    }

    /// The whole round trip against a real NetworkManager, which is the
    /// only place the D-Bus payload is actually judged: a dictionary NM
    /// dislikes fails here and nowhere else.
    ///
    /// Opt in, because it needs root, a running NetworkManager, the plugin
    /// installed, and an existing interface to describe. It also touches
    /// the machine's network state, so it must never run unattended:
    ///
    /// ```text
    /// ip tuntap add mode tun name warrentest0
    /// ip addr add 10.66.0.2/24 dev warrentest0 && ip link set warrentest0 up
    /// WARREN_NM_TEST_IFACE=warrentest0 cargo test -p talpid-dbus -- --ignored
    /// ```
    #[test]
    #[ignore = "needs root, NetworkManager, the installed plugin and a test interface"]
    fn publishes_then_withdraws_a_vpn_connection_on_a_real_network_manager() {
        let Ok(interface) = std::env::var("WARREN_NM_TEST_IFACE") else {
            panic!("set WARREN_NM_TEST_IFACE to the interface to describe");
        };
        let manager = NetworkManager::new().expect("a system bus");
        let service = std::env::var("WARREN_NM_TEST_SERVICE")
            .unwrap_or_else(|_| "org.freedesktop.NetworkManager.warren".to_owned());

        let indicator = manager
            .publish_vpn_indicator(
                &service,
                "Warren VPN test",
                &interface,
                "203.0.113.7".parse().unwrap(),
            )
            .expect("NetworkManager accepted the VPN connection");

        // Activation is asynchronous, so wait for the state rather than
        // read it immediately and conclude from a value that is still
        // settling.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut state = 0u32;
        while Instant::now() < deadline {
            state = Proxy::new(
                NM_BUS,
                &indicator.connection_path,
                RPC_TIMEOUT,
                &*manager.connection,
            )
            .get("org.freedesktop.NetworkManager.VPN.Connection", "VpnState")
            .unwrap_or(0);
            if state == NM_VPN_CONNECTION_STATE_ACTIVATED {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let published = state;

        manager
            .withdraw_vpn_indicator(indicator)
            .expect("NetworkManager took the VPN connection down");

        assert_eq!(
            published, NM_VPN_CONNECTION_STATE_ACTIVATED,
            "the desktop only shows a VPN for an activated connection"
        );
    }
}
