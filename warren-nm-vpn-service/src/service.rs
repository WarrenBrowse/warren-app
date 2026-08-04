//! The D-Bus side: speak `org.freedesktop.NetworkManager.VPN.Plugin`.
//!
//! NetworkManager spawns this process, calls `Connect`, and expects the
//! plugin to answer with the tunnel's shape. We answer for a tunnel the
//! daemon already built, then watch it: NetworkManager keeps a VPN
//! connection activated even after its interface is gone (measured), so
//! without this watchdog a daemon that dies leaves the desktop claiming a
//! VPN that no longer carries anything.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dbus::arg::{RefArg, Variant};
use dbus::blocking::LocalConnection;
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::{MatchRule, Message};

use crate::config::{ConfigPlan, ConfigValue, ConnectRequest, Family, TunnelIdentity};
use crate::iface;

const OBJECT_PATH: &str = "/org/freedesktop/NetworkManager/VPN/Plugin";
const INTERFACE: &str = "org.freedesktop.NetworkManager.VPN.Plugin";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// `NMVpnServiceState`.
const STATE_INIT: u32 = 1;
const STATE_STARTING: u32 = 3;
const STATE_STARTED: u32 = 4;
const STATE_STOPPING: u32 = 5;
const STATE_STOPPED: u32 = 6;

/// How often the message loop wakes up, which is also how fast a dead
/// tunnel is noticed. A stale indicator is a correctness problem, not a
/// cosmetic one, so this stays short enough to be imperceptible while
/// costing one `if_nametoindex` per tick.
const TICK: Duration = Duration::from_millis(500);

/// A plugin NetworkManager spawned but never drove has nothing to do, and
/// staying resident would leak a process per attempt.
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Default)]
struct Plugin {
    watch: Option<TunnelIdentity>,
    quit: bool,
}

impl Plugin {
    /// True once the interface we described is no longer the one we watch.
    fn tunnel_is_gone(&self) -> bool {
        self.watch
            .as_ref()
            .is_some_and(|watch| watch.is_stale(iface::index_of(&watch.tundev).ok()))
    }
}

fn emit(connection: &LocalConnection, member: &str, message: Message) {
    if connection.send(message).is_err() {
        log::warn!("Failed to emit {member} to NetworkManager");
    }
}

fn signal(member: &str) -> Message {
    Message::new_signal(OBJECT_PATH, INTERFACE, member)
        .expect("the plugin's own object path and interface are well formed")
}

fn emit_state(connection: &LocalConnection, state: u32) {
    log::debug!("Reporting VPN service state {state}");
    emit(
        connection,
        "StateChanged",
        signal("StateChanged").append1(state),
    );
}

type VariantMap = HashMap<String, Variant<Box<dyn RefArg>>>;

fn boxed<T: RefArg + 'static>(value: T) -> Variant<Box<dyn RefArg>> {
    Variant(Box::new(value))
}

/// Turn the decided dictionary into the D-Bus shapes NetworkManager reads.
fn variant_map(entries: Vec<(&'static str, ConfigValue)>) -> VariantMap {
    entries
        .into_iter()
        .map(|(key, value)| {
            let variant = match value {
                ConfigValue::Text(text) => boxed(text),
                ConfigValue::Flag(flag) => boxed(flag),
                ConfigValue::Number(number) => boxed(number),
                ConfigValue::Bytes(bytes) => boxed(bytes),
            };
            (key.to_owned(), variant)
        })
        .collect()
}

fn emit_config(connection: &LocalConnection, plan: &ConfigPlan) {
    emit(
        connection,
        "Config",
        signal("Config").append1(variant_map(plan.config_entries())),
    );

    for (family, member) in [(Family::V4, "Ip4Config"), (Family::V6, "Ip6Config")] {
        if let Some(entries) = plan.ip_entries(family) {
            emit(
                connection,
                member,
                signal(member).append1(variant_map(entries)),
            );
        }
    }
}

/// Pull `vpn.data` out of the connection dictionary NetworkManager passes
/// to `Connect`. It arrives as a variant holding `a{ss}`, which dbus-rs
/// exposes as a flat key, value, key, value iterator.
fn vpn_data(connection: &HashMap<String, VariantMap>) -> HashMap<String, String> {
    let mut data = HashMap::new();
    let Some(vpn) = connection.get("vpn") else {
        return data;
    };
    let Some(entries) = vpn.get("data").and_then(|value| value.0.as_iter()) else {
        return data;
    };
    let mut entries = entries.filter_map(|entry| entry.as_str().map(str::to_owned));
    while let (Some(key), Some(value)) = (entries.next(), entries.next()) {
        data.insert(key, value);
    }
    data
}

fn describe_tunnel(
    connection_settings: &HashMap<String, VariantMap>,
) -> Result<(ConfigPlan, u32), String> {
    let request = ConnectRequest::from_vpn_data(&vpn_data(connection_settings))
        .map_err(|error| error.to_string())?;
    let ifindex = iface::index_of(&request.tundev).map_err(|error| error.to_string())?;
    let addresses = iface::addresses_of(&request.tundev).map_err(|error| error.to_string())?;
    let plan = ConfigPlan::build(request, addresses).map_err(|error| error.to_string())?;
    Ok((plan, ifindex))
}

fn handle_connect(plugin: &Rc<RefCell<Plugin>>, connection: &LocalConnection, message: &Message) {
    let settings: HashMap<String, VariantMap> = match message.read1() {
        Ok(settings) => settings,
        Err(error) => {
            log::error!("NetworkManager sent a connection we cannot read: {error}");
            let _ = connection.send(message.method_return());
            emit_state(connection, STATE_STOPPED);
            plugin.borrow_mut().quit = true;
            return;
        }
    };
    let _ = connection.send(message.method_return());
    emit_state(connection, STATE_STARTING);

    match describe_tunnel(&settings) {
        Ok((plan, ifindex)) => {
            log::info!("Describing tunnel on {} to NetworkManager", plan.tundev);
            emit_config(connection, &plan);
            plugin.borrow_mut().watch = Some(TunnelIdentity {
                tundev: plan.tundev.clone(),
                ifindex,
            });
            emit_state(connection, STATE_STARTED);
        }
        Err(reason) => {
            log::error!("Refusing to describe the tunnel: {reason}");
            emit(connection, "Failure", signal("Failure").append1(0u32));
            emit_state(connection, STATE_STOPPED);
            plugin.borrow_mut().quit = true;
        }
    }
}

fn handle_disconnect(
    plugin: &Rc<RefCell<Plugin>>,
    connection: &LocalConnection,
    message: &Message,
) {
    let _ = connection.send(message.method_return());
    emit_state(connection, STATE_STOPPING);
    emit_state(connection, STATE_STOPPED);
    let mut plugin = plugin.borrow_mut();
    plugin.watch = None;
    plugin.quit = true;
}

fn handle_properties(connection: &LocalConnection, message: &Message) {
    let member = message.member().map(|member| member.to_string());
    match member.as_deref() {
        Some("Get") => {
            let _ = connection.send(message.method_return().append1(boxed(STATE_INIT)));
        }
        Some("GetAll") => {
            let mut all: VariantMap = HashMap::new();
            all.insert("State".into(), boxed(STATE_INIT));
            let _ = connection.send(message.method_return().append1(all));
        }
        _ => {
            let _ = connection.send(message.method_return());
        }
    }
}

pub fn main() {
    crate::logging::init();

    let bus_name = warren_product_env::NM_VPN_SERVICE;
    let connection = match LocalConnection::new_system() {
        Ok(connection) => connection,
        Err(error) => {
            log::error!("Cannot reach the system bus: {error}");
            std::process::exit(1);
        }
    };
    // Do not queue: a second instance means NetworkManager already has a
    // live plugin, and silently waiting behind it would answer nothing.
    if let Err(error) = connection.request_name(bus_name, false, true, true) {
        log::error!("Cannot own {bus_name}: {error}");
        std::process::exit(1);
    }

    let plugin = Rc::new(RefCell::new(Plugin::default()));
    let dispatch = plugin.clone();
    connection.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |message, connection| {
            let interface = message.interface().map(|interface| interface.to_string());
            let member = message.member().map(|member| member.to_string());
            match (interface.as_deref(), member.as_deref()) {
                (Some(INTERFACE), Some("Connect" | "ConnectInteractive")) => {
                    handle_connect(&dispatch, connection, &message);
                }
                (Some(INTERFACE), Some("Disconnect")) => {
                    handle_disconnect(&dispatch, connection, &message);
                }
                (Some(INTERFACE), Some("NeedSecrets")) => {
                    // The tunnel is already up; nothing here is a secret.
                    let _ = connection.send(message.method_return().append1(""));
                }
                (Some(PROPERTIES_INTERFACE), _) => handle_properties(connection, &message),
                _ => {
                    let _ = connection.send(message.method_return());
                }
            }
            true
        }),
    );

    let started = Instant::now();
    loop {
        if let Err(error) = connection.process(TICK) {
            log::error!("System bus went away: {error}");
            break;
        }
        let idle = {
            let plugin = plugin.borrow();
            if plugin.quit {
                break;
            }
            if plugin.tunnel_is_gone() {
                log::info!("Tunnel interface is gone, retracting the VPN connection");
                emit_state(&connection, STATE_STOPPED);
                break;
            }
            plugin.watch.is_none()
        };
        if idle && started.elapsed() > IDLE_TIMEOUT {
            log::info!("NetworkManager never asked us to connect, exiting");
            break;
        }
    }
}
