#![allow(clippy::undocumented_unsafe_blocks)] // Remove me if you dare.

use parking_lot::Mutex;
use std::{
    collections::{BTreeSet, HashMap},
    fmt, mem,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    sync::{Arc, RwLock, mpsc as sync_mpsc},
    thread,
    time::Duration,
};
use system_configuration::{
    core_foundation::{
        array::CFArray,
        base::{CFType, TCFType, ToVoid},
        dictionary::{CFDictionary, CFMutableDictionary},
        number::CFNumber,
        propertylist::CFPropertyList,
        runloop::{CFRunLoop, kCFRunLoopCommonModes},
        string::CFString,
    },
    dynamic_store::{SCDynamicStore, SCDynamicStoreBuilder, SCDynamicStoreCallBackContext},
    sys::schema_definitions::{
        kSCPropNetDNSServerAddresses, kSCPropNetDNSServerPort, kSCPropNetInterfaceDeviceName,
    },
};
use talpid_routing::debounce::BurstGuard;

use super::ResolvedDnsConfig;

pub type Result<T> = std::result::Result<T, Error>;

const DNS_PORT: u16 = 53;

/// Errors that can happen when setting/monitoring DNS on macOS.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error while setting DNS servers
    #[error("Error while setting DNS servers")]
    SettingDnsFailed,

    /// Failed to initialize dynamic store
    #[error("Failed to initialize dynamic store")]
    DynamicStoreInitError,

    /// Failed to load interface config
    #[error("Failed to load interface config at path {0}")]
    LoadInterfaceConfigError(String),

    /// Failed to load DNS config
    #[error("Failed to load DNS config at path {0}")]
    LoadDnsConfigError(String),
}

const STATE_PATH_PATTERN: &str = "State:/Network/Service/.*/DNS";
const SETUP_PATH_PATTERN: &str = "Setup:/Network/Service/.*/DNS";

const BURST_BUFFER_PERIOD: Duration = Duration::from_millis(500);
const BURST_LONGEST_BUFFER_PERIOD: Duration = Duration::from_secs(5);

type ServicePath = String;
type DnsServer = String;

/// One write the monitor decided to make to a DNS key.
#[derive(Debug, PartialEq, Eq)]
enum StoreAction {
    Write(DnsSettings),
    Remove,
}

struct State {
    /// The settings this monitor is currently enforcing as active settings.
    dns_settings: Option<DnsSettings>,
    /// The backup of all DNS settings. These are being applied back on reset.
    backup: HashMap<ServicePath, Option<DnsSettings>>,
}

impl State {
    fn new() -> Self {
        Self {
            dns_settings: None,
            backup: HashMap::new(),
        }
    }

    /// Construct [`DnsSettings`] from the arguments and apply the desired addresses to all network services.
    fn apply_new_config(
        &mut self,
        store: &SCDynamicStore,
        interface: &str,
        servers: &[IpAddr],
        port: u16,
    ) -> Result<()> {
        talpid_types::detect_flood!();

        let servers: Vec<DnsServer> = servers.iter().map(|ip| ip.to_string()).collect();
        let new_settings =
            DnsSettings::from_server_addresses(&servers, interface.to_string(), port);
        match &self.dns_settings {
            None => {
                self.dns_settings = Some(new_settings);
                self.update_and_apply_state(store);
            }
            Some(old_settings) => {
                if new_settings.server_addresses() != old_settings.server_addresses() {
                    let orphans = self.orphans_in(&HashMap::new(), |key| key_exists(store, key));
                    for service_path in self.backup.keys() {
                        let Some(action) = plan_apply(service_path, None, &new_settings, &orphans)
                        else {
                            continue;
                        };
                        perform(store, service_path, action)?;
                    }
                    self.dns_settings = Some(new_settings);
                }
            }
        };

        Ok(())
    }

    /// Store changes to the DNS config, ignoring any changes that we have applied. Then apply our
    /// desired state to any services to which it has not already been applied.
    fn update_and_apply_state(&mut self, store: &SCDynamicStore) {
        let actual_state = read_all_dns(store);
        let orphans = self.orphans_in(&actual_state, |key| key_exists(store, key));
        self.update_backup_state(&actual_state, &orphans);
        self.apply_desired_state(store, &actual_state, &orphans);
    }

    /// The orphan `State:` keys among the current snapshot and the backup.
    /// The backup is included because a key retained after it vanished (see
    /// `merge_states`) is exactly the one `reset()` must not write back once
    /// its service is down.
    fn orphans_in(
        &self,
        actual_state: &HashMap<ServicePath, Option<DnsSettings>>,
        key_exists: impl Fn(&str) -> bool,
    ) -> BTreeSet<ServicePath> {
        let candidates: BTreeSet<&str> = actual_state
            .keys()
            .chain(self.backup.keys())
            .map(String::as_str)
            .collect();
        orphan_state_keys(candidates, key_exists)
    }

    /// Store changes to the DNS config, ignoring any changes that we have applied. The operation is
    /// idempotent.
    fn update_backup_state(
        &mut self,
        actual_state: &HashMap<ServicePath, Option<DnsSettings>>,
        orphans: &BTreeSet<ServicePath>,
    ) {
        let Some(ref desired_settings) = self.dns_settings else {
            return;
        };

        let prev_state = mem::take(&mut self.backup);
        let desired_set = desired_settings.server_addresses();

        self.backup = Self::merge_states(actual_state, prev_state, desired_set, orphans);
    }

    /// Merge `new_state` set by the OS with a previous `prev_state`, but ignore any service whose
    /// addresses are `ignore_addresses`.
    ///
    /// A captured `Some` original is never downgraded by a `None`/absent
    /// reading: configd makes the per-service DNS keys flicker while it
    /// recomputes the network state (utun removal, primary-service flap), and
    /// `reset()` re-merges one last snapshot right in that window. Trusting
    /// the flicker would clobber the primary interface's backup, so the
    /// restore would delete its real DNS and leave the host resolver-less until
    /// the next DHCP lease/RA rewrote it (minutes on Ethernet, ~30 s on WiFi).
    /// A retained-but-stale entry is the lesser evil: `reset()` decides by the
    /// service's liveness whether to write it back (see `plan_restore`).
    fn merge_states(
        new_state: &HashMap<ServicePath, Option<DnsSettings>>,
        mut prev_state: HashMap<ServicePath, Option<DnsSettings>>,
        ignore_addresses: BTreeSet<SocketAddr>,
        orphans: &BTreeSet<ServicePath>,
    ) -> HashMap<ServicePath, Option<DnsSettings>> {
        let mut modified_state = HashMap::new();

        for (path, settings) in new_state {
            let old_entry = prev_state.remove(path);
            // A resolver override is not a DNS configuration anyone can restore,
            // so it never enters the backup (see `is_resolver_override`), and
            // neither does an orphan `State:` key as a new original (see
            // `orphan_state_keys`; one captured while its service was up is
            // retained by the arm below, and `plan_restore` decides at reset).
            // Reading them as "no original" also makes the arm below keep a
            // real original that a foreign daemon has since overwritten.
            let settings = settings
                .as_ref()
                .filter(|s| !is_resolver_override(s) && !orphans.contains(path));
            match settings {
                // If the service is using the desired addresses, don't save changes
                Some(settings) if settings.server_addresses() == ignore_addresses => {
                    let settings = old_entry.unwrap_or_else(|| Some(settings.to_owned()));
                    modified_state.insert(path.to_owned(), settings);
                }
                // A None reading while a Some original is on file: keep the
                // original (see the function doc; this is the teardown
                // flicker that broke DNS restore).
                None if matches!(old_entry, Some(Some(_))) => {
                    let original = old_entry.expect("matched Some above");
                    modified_state.insert(path.to_owned(), original);
                }
                // Otherwise, save the new settings
                settings => {
                    let servers = settings
                        .map(|settings| settings.format_addresses())
                        .unwrap_or_default();
                    log::debug!("Saving DNS settings [{}] for {}", servers, path);
                    modified_state.insert(path.to_owned(), settings.cloned());
                }
            }
        }

        // Services whose keys vanished from this snapshot: retain their Some
        // backups instead of dropping them (same flicker as above). Whether
        // the retained original is written back is decided at reset, by the
        // service's liveness: an orphan written back becomes the host's
        // resolver (see `orphan_state_keys`).
        for (path, settings) in prev_state {
            if settings.is_some() {
                log::debug!("Retaining DNS backup for vanished service {path}");
                modified_state.insert(path, settings);
            } else {
                log::debug!("DNS removed for {path}");
            }
        }

        modified_state
    }

    /// Apply the desired addresses to all network services. The operation is idempotent.
    fn apply_desired_state(
        &mut self,
        store: &SCDynamicStore,
        actual_state: &HashMap<ServicePath, Option<DnsSettings>>,
        orphans: &BTreeSet<ServicePath>,
    ) {
        let Some(ref desired_settings) = self.dns_settings else {
            return;
        };

        for (path, settings) in actual_state {
            let Some(action) = plan_apply(path, settings.as_ref(), desired_settings, orphans)
            else {
                continue;
            };
            if let Err(e) = perform(store, path, action) {
                log::error!("Failed changing DNS for {}: {}", path, e);
            }
        }
    }

    fn reset(&mut self, store: &SCDynamicStore) -> Result<()> {
        log::trace!("Restoring DNS settings to: {:#?}", self.backup);

        let actual_state = read_all_dns(store);
        let orphans = self.orphans_in(&actual_state, |key| key_exists(store, key));
        self.update_backup_state(&actual_state, &orphans);
        self.dns_settings.take();

        let old_backup = std::mem::take(&mut self.backup);

        // One service failing must not cost the others their restore: the
        // invariant of this whole file is that the host is never left without
        // a resolver, so every entry is attempted and the first error reported.
        let mut first_error = None;
        for (service_path, settings) in old_backup {
            let action = plan_restore(&service_path, settings, &orphans);
            if let Err(e) = perform(store, &service_path, action) {
                log::error!("Failed restoring DNS for {service_path}: {e}");
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

/// What to write to `path` while the tunnel is up, given its current reading.
///
/// An orphan is removed rather than overwritten with our resolver, even when
/// it already carries our addresses: a write keeps alive a key whose owner
/// abandoned it, and a leftover one (a `SearchOrder`-ranked MagicDNS entry
/// left by a stopped Tailscale, or our own override from an older build) would
/// otherwise stay the host's first resolver.
fn plan_apply(
    path: &str,
    reading: Option<&DnsSettings>,
    desired: &DnsSettings,
    orphans: &BTreeSet<ServicePath>,
) -> Option<StoreAction> {
    if orphans.contains(path) {
        return Some(StoreAction::Remove);
    }
    match reading {
        Some(settings) if settings.server_addresses() == desired.server_addresses() => None,
        _ => Some(StoreAction::Write(desired.clone())),
    }
}

/// What to write to `path` at reset, given the original captured for it.
///
/// A `State:` key is the live state its owner publishes, so an original is
/// written back only while that owner still stands behind the key. Once the
/// service is down, the original names a resolver nothing routes to, and a
/// `SearchOrder` in it outranks the primary service: written back, it becomes
/// the only resolver the host has, which is a total loss of name resolution
/// on every disconnect. `Setup:` keys mirror the preferences and restore
/// unconditionally.
fn plan_restore(
    path: &str,
    original: Option<DnsSettings>,
    orphans: &BTreeSet<ServicePath>,
) -> StoreAction {
    match original {
        Some(settings) if !orphans.contains(path) => StoreAction::Write(settings),
        Some(_) => {
            log::warn!(
                "Not restoring DNS for {path}: its service publishes no address, \
                 so nothing routes to the resolver it names"
            );
            StoreAction::Remove
        }
        None => StoreAction::Remove,
    }
}

fn key_exists(store: &SCDynamicStore, key: &str) -> bool {
    store.get(CFString::new(key)).is_some()
}

fn perform(store: &SCDynamicStore, path: &str, action: StoreAction) -> Result<()> {
    match action {
        StoreAction::Write(settings) => settings.save(store, path),
        StoreAction::Remove => remove_dns_key(store, path),
    }
}

/// Remove `path` from the store. A key that is already absent is the desired
/// end state, never an error: a retained backup of a vanished service has no
/// key to remove.
fn remove_dns_key(store: &SCDynamicStore, path: &str) -> Result<()> {
    let key = CFString::new(path);
    if store.get(key.clone()).is_none() {
        return Ok(());
    }
    log::debug!("Removing DNS for {}", path);
    if store.remove(key) {
        Ok(())
    } else {
        Err(Error::SettingDnsFailed)
    }
}

/// Holds the configuration for one service.
#[derive(Debug, Eq, PartialEq, Clone)]
struct DnsSettings {
    dict: CFDictionary,
    name: String,
}

unsafe impl Send for DnsSettings {}

impl DnsSettings {
    pub fn from_server_addresses(server_addresses: &[DnsServer], name: String, port: u16) -> Self {
        let mut mut_dict = CFMutableDictionary::new();
        if !server_addresses.is_empty() {
            let cf_string_servers: Vec<CFString> =
                server_addresses.iter().map(|s| CFString::new(s)).collect();
            let server_addresses_value = CFArray::from_CFTypes(&cf_string_servers).into_untyped();
            let server_addresses_key =
                unsafe { CFString::wrap_under_get_rule(kSCPropNetDNSServerAddresses) };
            mut_dict.add(
                &server_addresses_key.to_void(),
                &server_addresses_value.to_void(),
            );

            // Set port if non-standard
            if port != DNS_PORT {
                let server_port_key =
                    unsafe { CFString::wrap_under_get_rule(kSCPropNetDNSServerPort) };
                let server_port_value = CFNumber::from(i32::from(port));
                mut_dict.add(&server_port_key.to_void(), &server_port_value.to_void());
            }
        }
        let dict = mut_dict.to_immutable();
        DnsSettings { dict, name }
    }

    /// Get DNS settings for a given service path. Returns `None` If the path does not exist.
    ///
    /// The interface name is best-effort: a service that has none still yields
    /// its DNS settings. Failing the whole load on the name is what made us
    /// unable to read our own write back on a NetworkExtension VPN service, so
    /// `apply_desired_state` rewrote that service every burst period for as
    /// long as the tunnel was up (see `InterfaceSettings::device_name`).
    pub fn load<S: Into<CFString>>(store: &SCDynamicStore, path: S) -> Result<Self> {
        let cf_path = path.into();

        let dict = store
            .get(cf_path.clone())
            .and_then(CFPropertyList::downcast_into::<CFDictionary>)
            .ok_or(Error::LoadDnsConfigError(cf_path.to_string()))?;

        let name = InterfaceSettings::load_from_dns_key(store, cf_path.to_string())
            .ok()
            .and_then(|interface| interface.device_name())
            .unwrap_or_default();

        Ok(DnsSettings { dict, name })
    }

    /// Set the dynamic store entry at `path` to a dictionary these DNS settings.
    pub fn save<S: Into<CFString> + fmt::Display>(
        &self,
        store: &SCDynamicStore,
        path: S,
    ) -> Result<()> {
        log::trace!("Setting DNS to [{}] for {}", self.format_addresses(), path);
        if store.set(path, self.dict.clone()) {
            Ok(())
        } else {
            Err(Error::SettingDnsFailed)
        }
    }

    pub fn server_addresses(&self) -> BTreeSet<SocketAddr> {
        let port = self
            .dict
            .find(unsafe { kSCPropNetDNSServerPort }.to_void())
            .map(|ptr| unsafe { CFType::wrap_under_get_rule(*ptr) })
            .and_then(|port| port.downcast::<CFNumber>())
            .and_then(|port| port.to_i32())
            .and_then(|port| u16::try_from(port).ok())
            .unwrap_or(DNS_PORT);

        self.dict
            .find(unsafe { kSCPropNetDNSServerAddresses }.to_void())
            .map(|array_ptr| unsafe { CFType::wrap_under_get_rule(*array_ptr) })
            .and_then(|array| array.downcast::<CFArray>())
            .and_then(Self::parse_cf_array_to_strings)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|addr| addr.parse::<IpAddr>())
            .map(|ip| SocketAddr::new(ip, port))
            .collect()
    }

    fn format_addresses(&self) -> String {
        let mut s = String::new();
        for addr in self.server_addresses() {
            if !s.is_empty() {
                s.push_str(", ");
            }
            s.push_str(&addr.to_string());
        }
        s
    }

    /// Parses a CFArray into a Rust vector of Rust strings, if the array contains CFString
    /// instances only, otherwise `None` is returned.
    fn parse_cf_array_to_strings(array: CFArray) -> Option<Vec<String>> {
        let mut strings = Vec::new();
        for item_ptr in array.iter() {
            let item = unsafe { CFType::wrap_under_get_rule(*item_ptr) };
            if let Some(string) = item.downcast::<CFString>() {
                strings.push(string.to_string());
            } else {
                log::error!("DNS server entry is not a string: {:?}", item);
                return None;
            };
        }
        Some(strings)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct InterfaceSettings(CFDictionary);

impl InterfaceSettings {
    /// Get network interface settings for the given path
    pub fn load_from_dns_key(store: &SCDynamicStore, dns_path: String) -> Result<Self> {
        // remove the "DNS" part of the path
        let path = match dns_path.strip_prefix("State") {
            Some(service_path) => "Setup".to_owned() + service_path,
            None => dns_path.clone(),
        };
        let interface_path = path.replace("/DNS", "/Interface");

        Ok(Self(
            store
                .get(CFString::from(interface_path.as_str()))
                .and_then(CFPropertyList::downcast_into::<CFDictionary>)
                .ok_or(Error::LoadInterfaceConfigError(path))?,
        ))
    }

    /// The BSD device name of the service's interface, when it has one.
    ///
    /// A NetworkExtension VPN service (Tailscale, a coexisting VPN app)
    /// publishes `Type` and `SubType` and no `DeviceName`, so `None` is a
    /// normal reading for a service that is perfectly configured. Reporting it
    /// as an error made `DnsSettings::load` fail for that service, and the
    /// caller reads a failure as "this service has no DNS settings": we then
    /// never recognised the addresses we had just written there, and rewrote
    /// them on every store notification, forever.
    pub fn device_name(&self) -> Option<String> {
        self.0
            .find(unsafe { kSCPropNetInterfaceDeviceName }.to_void())
            .map(|str_pointer| unsafe { CFType::wrap_under_get_rule(*str_pointer) })
            .and_then(|string| string.downcast::<CFString>())
            .map(|cf_string| cf_string.to_string())
    }
}

unsafe impl Send for InterfaceSettings {}

pub struct DnsMonitor {
    /// The backing "System Configuration framework" store, which allow us to access and detect
    /// changes to the device's network configuration.
    store: SCDynamicStore,
    /// The current DNS injection state. If this is `None` it means we are not injecting any DNS.
    /// When it's `Some(state)` we are actively making sure `state.dns_settings` is configured
    /// on all network interfaces.
    state: Arc<Mutex<State>>,
}

/// SAFETY: The `SCDynamicStore` can be sent to other threads since it doesn't share mutable state
/// with anything else.
unsafe impl Send for DnsMonitor {}

impl super::DnsMonitorT for DnsMonitor {
    type Error = Error;

    /// Creates and returns a new `DnsMonitor`. This spawns a background thread that will monitor
    /// DNS settings for all network interfaces. If any changes occur it will instantly reset
    /// the DNS settings for that interface back to the last server list set to this instance
    /// with `set_dns`.
    fn new() -> Result<Self> {
        let state = Arc::new(Mutex::new(State::new()));
        Self::spawn(state.clone())?;
        let monitor = DnsMonitor {
            store: SCDynamicStoreBuilder::new("mullvad-dns").build(),
            state,
        };
        // Repair any DNS entry nothing answers behind (a dead local resolver
        // from an unclean exit, an orphan key of a stopped VPN service) before
        // we start managing DNS ourselves.
        remove_dead_dns_entries(&monitor.store);
        Ok(monitor)
    }

    /// Update the system config to use the DNS `config`.
    ///
    /// Note that the `interface` parameter does nothing on macOS. Since we can't configure DNS
    /// on the tunnel interface, we have to configure all interfaces.
    fn set(&mut self, interface: &str, config: ResolvedDnsConfig) -> Result<()> {
        let port = config.port;
        let servers: Vec<_> = config.addresses().collect();

        let result = {
            let mut state = self.state.lock();
            state.apply_new_config(&self.store, interface, &servers, port)
        };
        // Nudge the resolver whether or not the apply fully succeeded: a partial
        // apply can leave some services pointing at the new resolver while
        // mDNSResponder still caches answers from the old one, so flush
        // unconditionally (same reasoning as `reset`).
        flush_dns_cache();
        result
    }

    fn reset(&mut self) -> Result<()> {
        let result = self.state.lock().reset(&self.store);
        // Reclaim any entry nothing answers behind, whatever planted it. The
        // startup sweep runs once per daemon start, and the daemon outlives
        // every connect/disconnect cycle, so teardown is the only moment that
        // can free a host already stranded by an older build or by a second
        // VPN daemon that exited without restoring.
        remove_dead_dns_entries(&self.store);
        // Always nudge the resolver on teardown, even if restoring the store
        // entries reported an error: the goal is to leave the system with a
        // working resolver, not stranded on a dead in-tunnel one.
        flush_dns_cache();
        result
    }
}

impl DnsMonitor {
    /// Spawns the background thread running the CoreFoundation main loop and monitors the system
    /// for DNS changes.
    fn spawn(state: Arc<Mutex<State>>) -> Result<()> {
        let (result_tx, result_rx) = sync_mpsc::channel();
        thread::spawn(move || match create_dynamic_store(state) {
            Ok(store) => {
                result_tx.send(Ok(())).unwrap();
                run_dynamic_store_runloop(store);
                // the Core Foundation main loop should only exit when macOS is shut down.
                // If it exits in any other case, that would be a bug,
                // and DNS monitoring would break.
                //
                // If we start seeing this happen on a running system, we should add error
                // handling that tries to restart the main loop (or even the entire daemon).
                log::warn!("Core Foundation main loop exited! Is macOS shutting down?");
            }
            Err(e) => result_tx.send(Err(e)).unwrap(),
        });
        result_rx.recv().unwrap()
    }
}

/// Creates a `SCDynamicStore` that watches all network interfaces for changes to the DNS settings.
fn create_dynamic_store(state: Arc<Mutex<State>>) -> Result<SCDynamicStore> {
    struct StoreContainer {
        store: SCDynamicStore,
    }
    // SAFETY: The store is thread-safe
    unsafe impl Send for StoreContainer {}
    // SAFETY: The store is thread-safe
    unsafe impl Sync for StoreContainer {}

    let store_container: Arc<RwLock<Option<StoreContainer>>> = Arc::new(RwLock::new(None));
    let store_container_copy = store_container.clone();

    let update_trigger = BurstGuard::new(
        BURST_BUFFER_PERIOD,
        BURST_LONGEST_BUFFER_PERIOD,
        move || {
            if let Some(store) = &*store_container.read().unwrap() {
                state.lock().update_and_apply_state(&store.store);
            }
        },
    );

    let callback_context = SCDynamicStoreCallBackContext {
        callout: dns_change_callback,
        info: update_trigger,
    };

    let store = SCDynamicStoreBuilder::new("talpid-dns-monitor")
        .callback_context(callback_context)
        .build();

    let mut store_container = store_container_copy.write().unwrap();
    *store_container = Some(StoreContainer {
        store: store.clone(),
    });

    let watch_keys: CFArray<CFString> = CFArray::from_CFTypes(&[]);
    let watch_patterns = CFArray::from_CFTypes(&[
        CFString::new(STATE_PATH_PATTERN),
        CFString::new(SETUP_PATH_PATTERN),
    ]);

    if store.set_notification_keys(&watch_keys, &watch_patterns) {
        log::trace!("Registered for dynamic store notifications");
        Ok(store)
    } else {
        Err(Error::DynamicStoreInitError)
    }
}

fn run_dynamic_store_runloop(store: SCDynamicStore) {
    let run_loop_source = store.create_run_loop_source();
    CFRunLoop::get_current().add_source(&run_loop_source, unsafe { kCFRunLoopCommonModes });

    log::trace!("Entering DNS CFRunLoop");
    CFRunLoop::run_current();
}

/// This function is called by the Core Foundation event loop when there is a change to one or more
/// watched dynamic store values. In our case we watch all DNS settings.
fn dns_change_callback(
    _store: SCDynamicStore,
    _changed_keys: CFArray<CFString>,
    state: &mut BurstGuard,
) {
    state.trigger();
}

/// Read all existing DNS settings and return them.
fn read_all_dns(store: &SCDynamicStore) -> HashMap<ServicePath, Option<DnsSettings>> {
    let mut settings: HashMap<_, _> = HashMap::new();
    // All "state" DNS, and all corresponding "setup" DNS even if they don't exist
    if let Some(paths) = store.get_keys(STATE_PATH_PATTERN) {
        for state_path in paths.iter() {
            let state_path_str = state_path.to_string();
            let setup_path_str = state_to_setup_path(&state_path_str).unwrap();
            settings.insert(
                state_path_str,
                DnsSettings::load(store, state_path.clone()).ok(),
            );
            settings.insert(
                setup_path_str.clone(),
                DnsSettings::load(store, setup_path_str.as_ref()).ok(),
            );
        }
    }
    // All "setup" DNS not already covered
    if let Some(paths) = store.get_keys(SETUP_PATH_PATTERN) {
        for setup_path in paths.iter() {
            let setup_path_str = setup_path.to_string();
            settings
                .entry(setup_path_str)
                .or_insert_with(|| DnsSettings::load(store, setup_path.clone()).ok());
        }
    }
    settings
}

fn state_to_setup_path(state_path: &str) -> Option<String> {
    if state_path.starts_with("State:/") {
        Some(state_path.replacen("State:/", "Setup:/", 1))
    } else {
        None
    }
}

/// Repair every DNS entry nothing answers behind (a dead local resolver from
/// an unclean daemon exit, an orphan key of a stopped VPN service), without
/// constructing a monitor. The monitor runs the same repair at startup and at
/// teardown; this entry point exists for `warren-setup reset-firewall`, the
/// out-of-band rescue used when the daemon cannot come back up at all.
pub(crate) fn recover_after_crash() -> Result<()> {
    let store = SCDynamicStoreBuilder::new("warren-dns-recovery").build();
    remove_dead_dns_entries(&store);
    flush_dns_cache();
    Ok(())
}

/// Remove the DNS entries known to answer nothing: a stale loopback override
/// (below) and any orphan `State:` key (see `orphan_state_keys`), whichever
/// daemon or build left it. Runs at daemon start and at every teardown.
fn remove_dead_dns_entries(store: &SCDynamicStore) {
    let all = read_all_dns(store);
    for path in orphan_state_keys(all.keys(), |key| key_exists(store, key)) {
        log::warn!(
            "Removing orphan DNS key {path}: its service publishes no address, \
             so nothing routes to the resolver it names"
        );
        if let Err(e) = remove_dns_key(store, &path) {
            log::error!("Failed to remove orphan DNS key {path}: {e}");
        }
    }
    remove_stale_loopback_dns(store, all);
}

/// The `State:` DNS keys among `paths` behind which no live service stands.
///
/// A network service that is up publishes its addresses under
/// `State:/Network/Service/<id>/IPv4` or `/IPv6`; those keys are what configd
/// ranks, and they go with the service. Its `/DNS` key does not always go
/// with it: a stopped NetworkExtension VPN (Tailscale) leaves one behind, and
/// so did our own restore, which wrote the retained original back into it on
/// every disconnect. Such a key carries a resolver nothing routes to, and a
/// `SearchOrder` in it outranks the primary service, so configd makes it the
/// host's first resolver.
///
/// The verdict is limited to configured network services, those with a
/// `Setup:/Network/Service/<id>` entry: a `State:` DNS key with no service
/// definition behind it is a resolver published on its own (an encrypted-DNS
/// profile is the expected shape) and owns no address by construction, so it
/// is spared. Removal is destructive and the unknown reads as live. `Setup:`
/// keys mirror the preferences and are never orphans.
fn orphan_state_keys(
    paths: impl IntoIterator<Item = impl AsRef<str>>,
    key_exists: impl Fn(&str) -> bool,
) -> BTreeSet<ServicePath> {
    paths
        .into_iter()
        .filter(|path| {
            state_service_keys(path.as_ref()).is_some_and(|keys| {
                key_exists(&keys.definition) && !keys.addresses.iter().any(|key| key_exists(key))
            })
        })
        .map(|path| path.as_ref().to_owned())
        .collect()
}

/// The store keys that tell whether the service owning a `State:` DNS key is
/// configured and up, `None` for any other path.
struct ServiceKeys {
    definition: String,
    addresses: [String; 2],
}

fn state_service_keys(dns_path: &str) -> Option<ServiceKeys> {
    let id = dns_path
        .strip_prefix("State:/Network/Service/")?
        .strip_suffix("/DNS")?;
    Some(ServiceKeys {
        definition: format!("Setup:/Network/Service/{id}"),
        addresses: [
            format!("State:/Network/Service/{id}/IPv4"),
            format!("State:/Network/Service/{id}/IPv6"),
        ],
    })
}

/// Remove any DNS override left behind by a previous, unclean daemon exit
/// (SIGKILL, crash, or a `reset` that never completed).
///
/// While (re)establishing a tunnel we point the system DNS at our local
/// resolver, which binds to a random address in the `127/8` range (see
/// `talpid_core::resolver`). If the daemon dies before restoring the DNS, the
/// system is left pointing at a loopback resolver that no longer exists and
/// all name resolution breaks until a tunnel takes over again.
///
/// This runs at daemon start, before we have pointed the system DNS at our
/// own resolver, and again at every teardown, so any service whose servers
/// are *all* loopback addresses is a candidate leftover. We spare the
/// canonical `127.0.0.1`/`::1` (which a user-run resolver such as
/// dnscrypt-proxy would use; our resolver never binds those).
///
/// Crucially we only reclaim an override whose resolver is actually **dead**
/// (see [`loopback_resolver_is_live`]): a *stale* override by definition points
/// at a resolver that no longer exists. A loopback resolver that is still
/// answering belongs to a running daemon - ours from this very run, or, when a
/// second VPN daemon coexists on the host (e.g. a separately-installed Mullvad
/// app whose own resolver also binds a non-canonical `127/8` address), theirs.
/// The previous code removed *every* non-canonical loopback override blindly,
/// which wiped the other daemon's live DNS and broke all name resolution on the
/// host the moment this daemon started. Do NOT drop the liveness check.
fn remove_stale_loopback_dns(
    store: &SCDynamicStore,
    all: HashMap<ServicePath, Option<DnsSettings>>,
) {
    for (path, settings) in all {
        let Some(settings) = settings else { continue };
        if !is_resolver_override(&settings) {
            continue;
        }
        let addresses = settings.server_addresses();
        // Keep the override if ANY of its resolvers is still live: that is a
        // running resolver (ours or a coexisting daemon's), not a leftover.
        // Probe each resolver on its OWN port (parsed from the store, which can
        // carry a non-53 ServerPort), not a hardcoded 53: a coexisting resolver
        // bound on e.g. 127.x:5353 must read as live, otherwise we probe the
        // wrong port, find it free, and reclaim a live foreign override.
        if addresses
            .iter()
            .any(|sa| loopback_resolver_is_live(sa.ip(), sa.port()))
        {
            continue;
        }
        log::warn!(
            "Removing stale loopback DNS override at {path} \
             (leftover from a previous unclean daemon exit)"
        );
        if let Err(e) = remove_dns_key(store, &path) {
            log::error!("Failed to remove stale DNS override at {path}: {e}");
        }
    }
}

/// Force mDNSResponder to reload DNS configuration and drop its cache.
///
/// Rewriting the `State:/Network/Service/.../DNS` entries is not always enough
/// on disconnect: mDNSResponder can keep serving the previous resolver (a
/// now-dead in-tunnel address such as the multi-hop gateway) until it is told
/// to reload, leaving every app unable to resolve names even though the store
/// is correct. Flushing the cache and sending SIGHUP forces an immediate
/// reload, so `getaddrinfo` recovers without a manual fix. Best-effort: any
/// failure is logged, never propagated, since a missing tool must not turn a
/// teardown into a hard error.
fn flush_dns_cache() {
    for (bin, args) in [
        ("/usr/bin/dscacheutil", &["-flushcache"][..]),
        ("/usr/bin/killall", &["-HUP", "mDNSResponder"][..]),
    ] {
        match std::process::Command::new(bin).args(args).status() {
            Ok(status) if status.success() => {}
            Ok(status) => log::debug!("{bin} {args:?} exited with {status}"),
            Err(e) => log::warn!("failed to run {bin} to flush DNS cache: {e}"),
        }
    }
}

/// Whether a DNS resolver is currently listening on `addr:port`.
///
/// Probed by attempting to bind the address: a successful bind means nothing is
/// listening (the override is a dead leftover, safe to reclaim); `AddrInUse`
/// means a live resolver holds it and must be left alone. Any other bind error
/// is treated as "live" (fail-safe: never reclaim on uncertainty, so we cannot
/// clobber a working resolver). macOS routes the whole `127/8` to loopback, so
/// binding an arbitrary `127.x` address works without an interface alias.
fn loopback_resolver_is_live(addr: IpAddr, port: u16) -> bool {
    match UdpSocket::bind((addr, port)) {
        Ok(_) => false,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => true,
        Err(_) => true,
    }
}

/// Whether a DNS reading is a VPN daemon's own resolver override rather than a
/// DNS configuration that belongs to the user.
///
/// Our local resolver binds a random non-canonical `127/8` address (see
/// `talpid_core::resolver`), and so does any coexisting VPN daemon's. Such an
/// address is only reachable while the daemon that bound it is alive, so it
/// must never be captured as the "original" to restore: the daemon that owns it
/// exits, and the restore then points every network service at a corpse. That
/// is a total, permanent loss of name resolution, because the `Setup:` keys we
/// write are persistent user configuration which no DHCP lease, network change
/// or reboot ever rewrites; only an explicit reconfiguration clears it.
///
/// Treating it as "no original" is also the accurate answer: removing the key
/// returns the service to its DHCP-provided DNS, which is what the user had
/// before any VPN touched it. A daemon still running when we drop its override
/// re-applies it from its own store watcher.
fn is_resolver_override(settings: &DnsSettings) -> bool {
    let addresses = settings.server_addresses();
    !addresses.is_empty()
        && addresses
            .iter()
            .map(SocketAddr::ip)
            .all(is_reclaimable_loopback)
}

/// Whether `ip` is a loopback address our local resolver may have bound,
/// excluding the canonical localhost addresses a user-run resolver would use.
fn is_reclaimable_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
        && ip != IpAddr::V4(Ipv4Addr::LOCALHOST)
        && ip != IpAddr::V6(Ipv6Addr::LOCALHOST)
}

#[cfg(test)]
mod test {
    use super::{
        DNS_PORT, DnsSettings, InterfaceSettings, State, StoreAction, loopback_resolver_is_live,
        orphan_state_keys, plan_apply, plan_restore, remove_dns_key,
    };
    use std::{
        collections::{BTreeSet, HashMap},
        net::SocketAddr,
        net::{IpAddr, Ipv4Addr, UdpSocket},
    };
    use system_configuration::{
        core_foundation::{
            base::{TCFType, ToVoid},
            dictionary::CFMutableDictionary,
            string::CFString,
        },
        dynamic_store::SCDynamicStoreBuilder,
        sys::schema_definitions::kSCPropNetInterfaceDeviceName,
    };

    #[test]
    fn live_loopback_resolver_is_detected_and_preserved() {
        // Regression: the stale-override cleanup must NOT reclaim a loopback
        // override whose resolver is still answering - that is exactly how a
        // coexisting Mullvad app's live resolver (a non-canonical 127/8
        // address) got wiped, killing all DNS on daemon start.
        let live =
            UdpSocket::bind((IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0)).expect("bind live");
        let live_addr = live.local_addr().expect("live addr");
        assert!(
            loopback_resolver_is_live(live_addr.ip(), live_addr.port()),
            "a bound loopback resolver must read as live (preserved, never reclaimed)"
        );
    }

    #[test]
    fn dead_loopback_address_is_reclaimable() {
        // A loopback address with no listener must read as not-live, so a
        // genuine stale leftover from an unclean exit is still cleaned up.
        let probe = UdpSocket::bind((IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0)).expect("bind");
        let free = probe.local_addr().expect("addr");
        drop(probe); // release the port so nothing is listening
        assert!(
            !loopback_resolver_is_live(free.ip(), free.port()),
            "an address with no resolver must read as not-live (reclaimable)"
        );
    }

    /// The initial backup should equal whatever the first provided state is.
    #[test]
    fn test_backup_new_dns_config() {
        let prev_state = HashMap::new();

        let new_state = HashMap::from([
            ("a".to_owned(), None),
            (
                "b".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["1.2.3.4".to_owned()],
                    "iface_b".to_owned(),
                    DNS_PORT,
                )),
            ),
            // One of our states already equals the desired state. It should be stored regardless.
            (
                "c".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["10.64.0.1".to_owned()],
                    "iface_c".to_owned(),
                    DNS_PORT,
                )),
            ),
        ]);

        let desired_addresses: BTreeSet<SocketAddr> = ["10.64.0.1:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(merged_state, new_state);
    }

    /// Any changes equal to the desired state should be ignored. Other changes should be recorded.
    #[test]
    fn test_backup_ignore_desired_state() {
        let prev_state = HashMap::from([
            ("a".to_owned(), None),
            (
                "b".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["1.2.3.4".to_owned()],
                    "iface_b".to_owned(),
                    DNS_PORT,
                )),
            ),
            (
                "c".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["10.64.0.1".to_owned()],
                    "iface_c".to_owned(),
                    DNS_PORT,
                )),
            ),
            (
                "d".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["1.3.3.7".to_owned()],
                    "iface_d".to_owned(),
                    DNS_PORT,
                )),
            ),
        ]);
        let new_state = HashMap::from([
            // This change should be ignored
            (
                "a".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["10.64.0.1".to_owned()],
                    "iface_a".to_owned(),
                    DNS_PORT,
                )),
            ),
            // This change should be ignored
            (
                "b".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["10.64.0.1".to_owned()],
                    "iface_b".to_owned(),
                    DNS_PORT,
                )),
            ),
            // This change should be ignored
            (
                "c".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["4.3.2.1".to_owned()],
                    "iface_c".to_owned(),
                    DNS_PORT,
                )),
            ),
            // This change should NOT be ignored
            (
                "d".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["4.3.2.1".to_owned()],
                    "iface_d".to_owned(),
                    DNS_PORT,
                )),
            ),
        ]);
        let expect_state = HashMap::from([
            ("a".to_owned(), None),
            (
                "b".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["1.2.3.4".to_owned()],
                    "iface_b".to_owned(),
                    DNS_PORT,
                )),
            ),
            (
                "c".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["4.3.2.1".to_owned()],
                    "iface_c".to_owned(),
                    DNS_PORT,
                )),
            ),
            (
                "d".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["4.3.2.1".to_owned()],
                    "iface_d".to_owned(),
                    DNS_PORT,
                )),
            ),
        ]);

        let desired_addresses: BTreeSet<SocketAddr> = ["10.64.0.1:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(merged_state, expect_state);
    }

    /// Services with a `Some` backup that vanish from a snapshot are RETAINED
    /// (configd key flicker at teardown must not clobber the backup); only
    /// `None` entries are dropped.
    #[test]
    fn test_backup_retains_vanished_services() {
        let prev_state = HashMap::from([
            (
                "a".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["10.64.0.1".to_owned()],
                    "iface_a".to_owned(),
                    DNS_PORT,
                )),
            ),
            (
                "b".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["1.2.3.4".to_owned()],
                    "iface_b".to_owned(),
                    DNS_PORT,
                )),
            ),
            ("c".to_owned(), None),
        ]);
        let new_state = HashMap::from([("c".to_owned(), None)]);
        let mut expected_state = prev_state.clone();
        expected_state.insert("c".to_owned(), None);

        let desired_addresses: BTreeSet<SocketAddr> = ["10.64.0.1:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(merged_state, expected_state);
    }

    /// Regression test for the teardown-flicker failure mode: at tunnel teardown,
    /// configd transiently hides the primary service's DNS key (absent from
    /// the snapshot) or reads it as `None`. The captured originals must
    /// survive that snapshot, otherwise `reset()` deletes the primary
    /// interface's real DNS and the host is left resolver-less until the next
    /// DHCP lease (minutes on Ethernet).
    #[test]
    fn test_backup_survives_teardown_flicker() {
        let en0_original = Some(DnsSettings::from_server_addresses(
            &["fd0f:ee:b0::1".to_owned()],
            "en0".to_owned(),
            DNS_PORT,
        ));
        let en1_original = Some(DnsSettings::from_server_addresses(
            &["fd0f:ee:b0::1".to_owned()],
            "en1".to_owned(),
            DNS_PORT,
        ));
        let prev_state = HashMap::from([
            (
                "State:/Network/Service/EN0/DNS".to_owned(),
                en0_original.clone(),
            ),
            (
                "State:/Network/Service/EN1/DNS".to_owned(),
                en1_original.clone(),
            ),
        ]);
        // Mid-teardown snapshot: en0's key is gone entirely, en1's reads None.
        let new_state = HashMap::from([("State:/Network/Service/EN1/DNS".to_owned(), None)]);

        let desired_addresses: BTreeSet<SocketAddr> = ["10.64.0.1:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        let expected_state = HashMap::from([
            ("State:/Network/Service/EN0/DNS".to_owned(), en0_original),
            ("State:/Network/Service/EN1/DNS".to_owned(), en1_original),
        ]);
        assert_eq!(merged_state, expected_state);
    }

    /// A reading that is only a non-canonical loopback address is some VPN
    /// daemon's resolver override, never a user's DNS. Backing one up is how a
    /// disconnect strands the host: the address dies with the daemon that owned
    /// it, and `Setup:` keys are persistent user configuration that no DHCP
    /// lease rewrites. It must be recorded as "no original" so `reset()`
    /// removes the key and the service falls back to DHCP DNS.
    #[test]
    fn test_backup_never_captures_a_resolver_override() {
        let prev_state = HashMap::new();
        let new_state = HashMap::from([
            (
                "Setup:/Network/Service/EN0/DNS".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["127.55.251.5".to_owned()],
                    "en0".to_owned(),
                    DNS_PORT,
                )),
            ),
            (
                "State:/Network/Service/EN0/DNS".to_owned(),
                Some(DnsSettings::from_server_addresses(
                    &["192.168.1.254".to_owned()],
                    "en0".to_owned(),
                    DNS_PORT,
                )),
            ),
        ]);

        let desired_addresses: BTreeSet<SocketAddr> = ["127.41.79.67:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(
            merged_state.get("Setup:/Network/Service/EN0/DNS"),
            Some(&None),
            "a loopback resolver override must never become a restorable backup"
        );
        assert_eq!(
            merged_state.get("State:/Network/Service/EN0/DNS"),
            Some(&Some(DnsSettings::from_server_addresses(
                &["192.168.1.254".to_owned()],
                "en0".to_owned(),
                DNS_PORT,
            ))),
            "a real DNS server must still be backed up"
        );
    }

    /// The canonical localhost addresses are spared: a user-run resolver
    /// (dnscrypt-proxy, Pi-hole) binds those, and our own resolver never does.
    #[test]
    fn test_backup_keeps_canonical_localhost() {
        let prev_state = HashMap::new();
        let new_state = HashMap::from([(
            "Setup:/Network/Service/EN0/DNS".to_owned(),
            Some(DnsSettings::from_server_addresses(
                &["127.0.0.1".to_owned()],
                "en0".to_owned(),
                DNS_PORT,
            )),
        )]);

        let desired_addresses: BTreeSet<SocketAddr> = ["127.41.79.67:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(
            merged_state.get("Setup:/Network/Service/EN0/DNS"),
            Some(&Some(DnsSettings::from_server_addresses(
                &["127.0.0.1".to_owned()],
                "en0".to_owned(),
                DNS_PORT,
            ))),
            "a user-run resolver on canonical localhost must be preserved"
        );
    }

    /// A resolver override appearing over an already-captured original must not
    /// replace it: the real DNS stays the value `reset()` restores.
    #[test]
    fn test_resolver_override_does_not_clobber_a_captured_original() {
        let original = Some(DnsSettings::from_server_addresses(
            &["192.168.1.254".to_owned()],
            "en0".to_owned(),
            DNS_PORT,
        ));
        let prev_state = HashMap::from([(
            "Setup:/Network/Service/EN0/DNS".to_owned(),
            original.clone(),
        )]);
        // Another daemon planted its resolver over the service we already backed up.
        let new_state = HashMap::from([(
            "Setup:/Network/Service/EN0/DNS".to_owned(),
            Some(DnsSettings::from_server_addresses(
                &["127.30.203.97".to_owned()],
                "en0".to_owned(),
                DNS_PORT,
            )),
        )]);

        let desired_addresses: BTreeSet<SocketAddr> = ["127.41.79.67:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(
            merged_state.get("Setup:/Network/Service/EN0/DNS"),
            Some(&original),
            "the captured original must survive a foreign resolver override"
        );
    }

    /// If DHCP provides an IP identical to our desired state, the tracked state will not reflect
    /// this. This is a known limitation.
    // TODO: This should actually succeed. If we happen to switch to a network whose IP equals
    //       the "desired IP", we should still back up the result.
    #[test]
    #[should_panic]
    fn test_backup_change_equals_desired_state() {
        let prev_state = HashMap::from([(
            "a".to_owned(),
            Some(DnsSettings::from_server_addresses(
                &["192.168.100.1".to_owned()],
                "iface_a".to_owned(),
                DNS_PORT,
            )),
        )]);
        let new_state = HashMap::from([(
            "a".to_owned(),
            Some(DnsSettings::from_server_addresses(
                &["192.168.1.1".to_owned()],
                "iface_a".to_owned(),
                DNS_PORT,
            )),
        )]);
        let expect_state = HashMap::from([(
            "a".to_owned(),
            Some(DnsSettings::from_server_addresses(
                &["192.168.1.1".to_owned()],
                "iface_a".to_owned(),
                DNS_PORT,
            )),
        )]);

        let desired_addresses: BTreeSet<SocketAddr> = ["192.168.1.1:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, prev_state, desired_addresses, &BTreeSet::new());

        assert_eq!(merged_state, expect_state);
    }

    /// A configured network service that is up publishes its addresses under
    /// `State:/Network/Service/<id>/IPv4` or `/IPv6`. A `State:` DNS key of a
    /// configured service with neither behind it names a resolver nothing
    /// routes to. `Setup:` keys are a mirror of the preferences and are never
    /// orphans.
    #[test]
    fn orphan_state_keys_are_those_whose_configured_service_publishes_no_address() {
        let paths = [
            "State:/Network/Service/WIFI/DNS".to_owned(),
            "State:/Network/Service/V6ONLY/DNS".to_owned(),
            "State:/Network/Service/TAILSCALE/DNS".to_owned(),
            "Setup:/Network/Service/TAILSCALE/DNS".to_owned(),
        ];
        let published = BTreeSet::from([
            "Setup:/Network/Service/WIFI",
            "State:/Network/Service/WIFI/IPv4",
            "Setup:/Network/Service/V6ONLY",
            "State:/Network/Service/V6ONLY/IPv6",
            "Setup:/Network/Service/TAILSCALE",
        ]);

        let orphans = orphan_state_keys(paths.iter(), |key| published.contains(key));

        assert_eq!(
            orphans,
            BTreeSet::from(["State:/Network/Service/TAILSCALE/DNS".to_owned()]),
            "only the State key of a configured service with no IPv4 and no IPv6 state is an orphan"
        );
    }

    /// A `State:` DNS key with no service definition behind it is a resolver
    /// published on its own (an encrypted-DNS profile has that shape) and owns
    /// no address by construction. Removing it would silently downgrade the
    /// user's resolver, so it reads as live.
    #[test]
    fn a_dns_key_without_a_service_definition_is_spared() {
        let paths = ["State:/Network/Service/DOH-PROFILE/DNS".to_owned()];

        let orphans = orphan_state_keys(paths.iter(), |_| false);

        assert!(orphans.is_empty(), "no service definition means no verdict");
    }

    /// Regression: a stopped NetworkExtension VPN (Tailscale) leaves its
    /// `State:` DNS key behind, carrying its MagicDNS resolver and a
    /// `SearchOrder` that outranks the primary service. Captured as an
    /// original and written back at `reset()`, that key made the dead resolver
    /// the host's only one after every disconnect. An orphan reading is "no
    /// original", whatever it holds.
    #[test]
    fn test_backup_never_captures_an_orphan_state_key() {
        let tailscale = "State:/Network/Service/TAILSCALE/DNS".to_owned();
        let wifi = "State:/Network/Service/WIFI/DNS".to_owned();
        let wifi_original = Some(DnsSettings::from_server_addresses(
            &["192.168.1.1".to_owned()],
            "en0".to_owned(),
            DNS_PORT,
        ));
        let new_state = HashMap::from([
            (
                tailscale.clone(),
                Some(DnsSettings::from_server_addresses(
                    &["100.100.100.100".to_owned()],
                    String::new(),
                    DNS_PORT,
                )),
            ),
            (wifi.clone(), wifi_original.clone()),
        ]);
        let orphans = BTreeSet::from([tailscale.clone()]);
        let desired_addresses: BTreeSet<SocketAddr> = ["10.66.0.1:53".parse().unwrap()].into();

        let merged_state =
            State::merge_states(&new_state, HashMap::new(), desired_addresses, &orphans);

        assert_eq!(
            merged_state.get(&tailscale),
            Some(&None),
            "an orphan State key must never become a restorable backup"
        );
        assert_eq!(
            merged_state.get(&wifi),
            Some(&wifi_original),
            "a live service's DNS is still backed up"
        );
    }

    /// While a tunnel is up, an orphan is removed rather than overwritten with
    /// our resolver: writing to it keeps a key alive that its owner already
    /// abandoned. A live key gets the usual treatment.
    #[test]
    fn apply_removes_an_orphan_state_key_instead_of_writing_to_it() {
        let tailscale = "State:/Network/Service/TAILSCALE/DNS".to_owned();
        let wifi = "State:/Network/Service/WIFI/DNS".to_owned();
        let desired = DnsSettings::from_server_addresses(
            &["10.66.0.1".to_owned()],
            "utun5".to_owned(),
            DNS_PORT,
        );
        let orphans = BTreeSet::from([tailscale.clone()]);

        assert_eq!(
            plan_apply(&tailscale, Some(&desired), &desired, &orphans),
            Some(StoreAction::Remove),
            "an orphan is removed even when it already carries our addresses"
        );
        assert_eq!(
            plan_apply(&wifi, Some(&desired), &desired, &orphans),
            None,
            "a live key already at the desired state is left alone"
        );
        assert_eq!(
            plan_apply(&wifi, None, &desired, &orphans),
            Some(StoreAction::Write(desired.clone())),
            "a live key reading None gets the desired state"
        );
    }

    /// At reset, an original captured while its service was up is not written
    /// back once the service is down: the key is removed, whatever the backup
    /// holds. `Setup:` keys and live `State:` keys restore as before.
    #[test]
    fn reset_removes_an_orphan_state_key_instead_of_restoring_it() {
        let tailscale = "State:/Network/Service/TAILSCALE/DNS".to_owned();
        let original = DnsSettings::from_server_addresses(
            &["100.100.100.100".to_owned()],
            String::new(),
            DNS_PORT,
        );
        let orphans = BTreeSet::from([tailscale.clone()]);

        assert_eq!(
            plan_restore(&tailscale, Some(original.clone()), &orphans),
            StoreAction::Remove,
            "a retained original of a service that is down is not written back"
        );
        assert_eq!(
            plan_restore(&tailscale, Some(original.clone()), &BTreeSet::new()),
            StoreAction::Write(original.clone()),
            "the same original restores while the service is up"
        );
        assert_eq!(
            plan_restore(
                "Setup:/Network/Service/TAILSCALE/DNS",
                Some(original.clone()),
                &orphans
            ),
            StoreAction::Write(original),
            "a path outside the orphan set restores"
        );
        assert_eq!(
            plan_restore(&tailscale, None, &BTreeSet::new()),
            StoreAction::Remove,
            "no original means the key is removed"
        );
    }

    /// The key that vanished mid-tunnel is retained in the backup and absent
    /// from the snapshot, so the liveness verdict must cover the backup too:
    /// that retained original is exactly the one `reset()` must not write back.
    #[test]
    fn a_retained_backup_of_a_stopped_service_is_an_orphan() {
        let tailscale = "State:/Network/Service/TAILSCALE/DNS".to_owned();
        let mut state = State::new();
        state.backup.insert(
            tailscale.clone(),
            Some(DnsSettings::from_server_addresses(
                &["100.100.100.100".to_owned()],
                String::new(),
                DNS_PORT,
            )),
        );
        let snapshot_without_it = HashMap::new();

        let configured = |key: &str| key == "Setup:/Network/Service/TAILSCALE";

        assert_eq!(
            state.orphans_in(&snapshot_without_it, configured),
            BTreeSet::from([tailscale.clone()]),
            "a backup-only path whose configured service publishes no address is an orphan"
        );
        assert!(
            state
                .orphans_in(&snapshot_without_it, |key| configured(key)
                    || key.ends_with("/IPv4"))
                .is_empty(),
            "the same path is live once its service publishes an address"
        );
    }

    /// A retained backup of a vanished service has no key to remove, and the
    /// restore must treat that as done rather than abort the remaining
    /// services on it.
    #[test]
    fn removing_an_absent_key_is_not_an_error() {
        let store = SCDynamicStoreBuilder::new("talpid-dns-test").build();
        let absent = "State:/Network/Service/TALPID-DNS-TEST-ABSENT/DNS";

        assert!(matches!(remove_dns_key(&store, absent), Ok(())));
    }

    /// Build a service `Interface` dictionary the way a NetworkExtension VPN
    /// service publishes it: a type and a subtype, and no device name.
    fn vpn_service_interface() -> InterfaceSettings {
        let mut dict = CFMutableDictionary::new();
        let type_key = CFString::new("Type");
        let type_value = CFString::new("VPN");
        dict.add(&type_key.to_void(), &type_value.to_void());
        let subtype_key = CFString::new("SubType");
        let subtype_value = CFString::new("io.tailscale.ipn.macsys");
        dict.add(&subtype_key.to_void(), &subtype_value.to_void());
        InterfaceSettings(dict.to_immutable())
    }

    /// Build a service `Interface` dictionary the way a physical service
    /// publishes it: carrying the BSD device name.
    fn physical_service_interface(device: &str) -> InterfaceSettings {
        let mut dict = CFMutableDictionary::new();
        let key = unsafe { CFString::wrap_under_get_rule(kSCPropNetInterfaceDeviceName) };
        let value = CFString::new(device);
        dict.add(&key.to_void(), &value.to_void());
        InterfaceSettings(dict.to_immutable())
    }

    /// Regression: a NetworkExtension VPN service (Tailscale, a coexisting VPN
    /// app) has no `DeviceName`, and treating that as a failure made
    /// `DnsSettings::load` return `Err` for the whole service. `read_all_dns`
    /// then read our own DNS write back as `None`, `apply_desired_state` never
    /// recognised its own value, and it rewrote the key every burst period for
    /// as long as the tunnel was up: about two writes a second, forever.
    #[test]
    fn a_vpn_service_interface_reports_no_device_name() {
        assert_eq!(
            vpn_service_interface().device_name(),
            None,
            "a service with no DeviceName must read as absent, never as an error"
        );
    }

    /// The absent name must not cost us the name we can read.
    #[test]
    fn a_physical_service_interface_reports_its_device_name() {
        assert_eq!(
            physical_service_interface("en0").device_name().as_deref(),
            Some("en0")
        );
    }
}
