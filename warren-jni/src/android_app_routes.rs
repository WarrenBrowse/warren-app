//! The JNI side of "Country per app" (`docs/app-routing.md` section 3.5):
//! the platform lookup Kotlin registers, the exits it sets, and the route
//! statuses it reads on every status wake.

#![cfg(all(target_os = "android", feature = "tunnel"))]

use std::{net::SocketAddr, sync::Arc};

use jnix::{
    FromJava, JnixEnv,
    jni::{
        JNIEnv, JavaVM,
        objects::{GlobalRef, JClass, JObject, JString, JValue},
        sys::{jobjectArray, jstring},
    },
};
use parking_lot::Mutex;

use crate::flow_owner::OwnerLookup;

/// The Kotlin `FlowOwnerResolver` and the VM that calls it, once registered.
struct Registered {
    vm: JavaVM,
    resolver: GlobalRef,
}

static RESOLVER: Mutex<Option<Arc<Registered>>> = Mutex::new(None);

/// What Kotlin reads through `getAppRoutesStatus`: no route while no tunnel
/// runs, which it shows as waiting for the VPN.
const NO_ROUTES: &str = "{\"routes\":[]}";

static STATUS: Mutex<String> = Mutex::new(String::new());

/// Publishes the route statuses and wakes the Kotlin status waiter.
pub(crate) fn publish_status(json: String) {
    *STATUS.lock() = json;
    crate::android_jni::bump_status();
}

/// Forgets the statuses of a tunnel that ended.
pub(crate) fn reset_status() {
    publish_status(NO_ROUTES.to_owned());
}

/// Whether Kotlin registered its lookup: without it no flow can be
/// attributed, so no app can be routed.
pub(crate) fn lookup_available() -> bool {
    RESOLVER.lock().is_some()
}

/// The owner lookup through the Kotlin `FlowOwnerResolver`, which calls
/// `ConnectivityManager.getConnectionOwnerUid` and
/// `PackageManager.getPackagesForUid`.
pub(crate) struct JniOwnerLookup;

impl JniOwnerLookup {
    fn registered() -> Option<Arc<Registered>> {
        RESOLVER.lock().clone()
    }
}

fn ip_octets(addr: SocketAddr) -> Vec<u8> {
    match addr.ip().to_canonical() {
        std::net::IpAddr::V4(ip) => ip.octets().to_vec(),
        std::net::IpAddr::V6(ip) => ip.octets().to_vec(),
    }
}

impl OwnerLookup for JniOwnerLookup {
    fn owner_uid(&self, protocol: i32, local: SocketAddr, remote: SocketAddr) -> i32 {
        let Some(registered) = Self::registered() else {
            return -1;
        };
        // The packet path's threads are the runtime's workers, which live as
        // long as the process: attaching them once for good avoids an attach
        // and a detach per new flow.
        let Ok(env) = registered.vm.attach_current_thread_permanently() else {
            return -1;
        };
        let mut uid = -1;
        // A permanently attached thread never frees its local references by
        // itself: the frame does.
        let _ = env.with_local_frame(4, || {
            let local_ip = env.byte_array_from_slice(&ip_octets(local))?;
            let remote_ip = env.byte_array_from_slice(&ip_octets(remote))?;
            uid = env
                .call_method(
                    registered.resolver.as_obj(),
                    "ownerUid",
                    "(I[BI[BI)I",
                    &[
                        JValue::Int(protocol),
                        JValue::Object(JObject::from(local_ip)),
                        JValue::Int(i32::from(local.port())),
                        JValue::Object(JObject::from(remote_ip)),
                        JValue::Int(i32::from(remote.port())),
                    ],
                )?
                .i()?;
            Ok(JObject::null())
        });
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return -1;
        }
        uid
    }

    fn packages_for_uid(&self, uid: u32) -> Vec<String> {
        let Some(registered) = Self::registered() else {
            return Vec::new();
        };
        let Ok(env) = registered.vm.attach_current_thread_permanently() else {
            return Vec::new();
        };
        let Ok(uid) = i32::try_from(uid) else {
            return Vec::new();
        };
        let mut packages = Vec::new();
        let _ = env.with_local_frame(16, || {
            let array = env
                .call_method(
                    registered.resolver.as_obj(),
                    "packagesForUid",
                    "(I)[Ljava/lang/String;",
                    &[JValue::Int(uid)],
                )?
                .l()?;
            if array.is_null() {
                return Ok(JObject::null());
            }
            let array = array.into_inner() as jobjectArray;
            for index in 0..env.get_array_length(array)? {
                let element = env.get_object_array_element(array, index)?;
                if element.is_null() {
                    continue;
                }
                let name: String = env.get_string(JString::from(element))?.into();
                packages.push(name);
                env.delete_local_ref(element)?;
            }
            Ok(JObject::null())
        });
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return Vec::new();
        }
        packages
    }
}

/// Registers the Kotlin `FlowOwnerResolver` the owner lookup calls, or
/// clears it when `resolver` is null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_setFlowOwnerResolver<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    resolver: JObject<'local>,
) {
    if resolver.is_null() {
        *RESOLVER.lock() = None;
        return;
    }
    let (Ok(vm), Ok(resolver)) = (env.get_java_vm(), env.new_global_ref(resolver)) else {
        log::warn!("App routing: the flow owner lookup could not be registered");
        return;
    };
    *RESOLVER.lock() = Some(Arc::new(Registered { vm, resolver }));
}

/// Replaces the exits in force with the JSON array Kotlin sends
/// (`[{"app":..,"country":..,"city":..}]`), followed live by a running
/// tunnel without a reconnect of its main session.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_setAppRoutes<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    json: JString<'local>,
) {
    let env = JnixEnv::from(env);
    let json = String::from_java(&env, json);
    crate::app_routes_session::set_app_exits_json(&json);
}

/// The route statuses (`{"routes":[..]}`), read by Kotlin on every status
/// wake.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_getAppRoutesStatus<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let status = {
        let status = STATUS.lock();
        if status.is_empty() {
            NO_ROUTES.to_owned()
        } else {
            status.clone()
        }
    };
    // Kotlin declares the answer non-null: fall back to no route.
    match env
        .new_string(status)
        .or_else(|_| env.new_string(NO_ROUTES))
    {
        Ok(s) => s.into_inner(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Tells the owner lookup that the installed packages changed, so a uid that
/// now names another package does not keep the route of the one before.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_notifyPackagesChanged<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) {
    crate::flow_owner::packages_changed();
}

/// Threads looking up the owners of new flows, so a burst of new flows is
/// answered in parallel rather than one Binder call after the other.
const OWNER_LOOKUP_THREADS: usize = 4;

/// Types of one tunnel's route side on Android.
type Resolver = crate::flow_owner::AndroidOwnerResolver<JniOwnerLookup>;
type Table = warren_app_routes::RoutingTable<Resolver>;

/// What [`AppRoutes::start`] needs from the tunnel.
pub(crate) struct AppRoutesSetup<'a> {
    pub tun: warrenguard_transport::AndroidTun,
    pub main: crate::app_routes_session::MainCircuit,
    pub wants_ipv6: bool,
    pub enable_daita: bool,
    pub tokens: &'a crate::token_provider::SessionTokens,
    pub anchor: Option<warrenguard_transport::route_anchor::RouteAnchorHandle>,
    /// The main supervisor's session watch: route sessions start once the
    /// main session is up, and the statuses read "waiting for the VPN" while
    /// it is not.
    pub sessions: warrenguard_transport::supervisor::ClientWatch,
}

/// The route side of one tunnel attempt: the router over the TUN, the route
/// controller and its sessions, and the planner that feeds them.
pub(crate) struct AppRoutes {
    table: Arc<Table>,
    tun: warrenguard_transport::AndroidTun,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    controller: Option<tokio::task::JoinHandle<()>>,
    _planner: crate::supervised_session::AbortOnDrop,
    _connected: crate::supervised_session::AbortOnDrop,
}

impl AppRoutes {
    /// Plans the exits in force and installs their policy at once, then
    /// starts the route sessions once the main session is up.
    pub(crate) fn start(setup: AppRoutesSetup<'_>) -> Self {
        use warren_app_routes::{
            AppRouteObserver, RelaySink, RouteController, RouteSessionConfig,
            SupervisorRouteSessions,
        };

        use crate::app_routes_session::{Planner, PlannerEvent, app_exits, run_planner};

        let main_exit = setup.main.main_exit;
        // Read and followed through one receiver, so a change landing
        // between the first plan and the planner's start is not missed.
        let mut exits = app_exits();
        let initial = exits.borrow_and_update().clone();
        let (planner, plan) = if lookup_available() {
            Planner::new(setup.main, initial)
        } else {
            Planner::without_attribution(setup.main, initial)
        };
        let (events_tx, events_rx) = tokio::sync::mpsc::unbounded_channel();
        let planner = crate::supervised_session::AbortOnDrop::spawn(run_planner(
            planner,
            exits,
            events_rx,
            publish_status,
        ));

        // `getConnectionOwnerUid` is a Binder call: it runs on the table's
        // own threads, so a new flow waits for its owner without holding
        // every other packet of the tunnel (docs/app-routing.md 2.1).
        let table = warren_app_routes::RoutingTable::with_owner_workers(
            crate::flow_owner::AndroidOwnerResolver::new(JniOwnerLookup),
            OWNER_LOOKUP_THREADS,
            || {
                let mut resolver = crate::flow_owner::AndroidOwnerResolver::new(JniOwnerLookup);
                move |flow: &talpid_app_routing::flow::FlowKey, under_way: bool| {
                    use talpid_app_routing::owner::{OwnerResolver, SocketOwner};
                    if under_way {
                        resolver
                            .socket_owner(flow)
                            .map_or(SocketOwner::Unknown, SocketOwner::Process)
                    } else {
                        resolver.owner(flow)
                    }
                }
            },
        );
        let mut config = RouteSessionConfig::new(Some(Arc::clone(&setup.tokens.source)));
        config.anchor = setup.anchor.clone();
        config.route_admission = Some(Arc::clone(&setup.tokens.route_admission));
        config.wants_ipv6 = setup.wants_ipv6;
        config.enable_daita = setup.enable_daita;
        let draining = events_tx.clone();
        config.on_exit_draining = Some(Arc::new(move |exit| {
            let _ = draining.send(PlannerEvent::Draining(exit));
        }));
        let reports = events_tx.clone();
        let observer: AppRouteObserver = Arc::new(move |routes| {
            let _ = reports.send(PlannerEvent::Reports(routes));
        });
        // VpnService.protect keeps every carrier socket of the process out
        // of the VPN, so there is no firewall to name a route's relay to.
        let name_relays: RelaySink = Arc::new(|_relays| Box::pin(async {}));
        let mut controller = RouteController::new(
            Arc::clone(&table),
            setup.tun.clone(),
            SupervisorRouteSessions::new(tokio::runtime::Handle::current(), config),
            talpid_app_routing::router::SessionAddresses {
                v4: Some(crate::tunnel::LOCAL_TUN_IPV4),
                v6: setup.wants_ipv6.then_some(crate::tunnel::LOCAL_TUN_IPV6),
            },
            name_relays,
            Some(observer),
        );
        controller.seed(&plan.borrow(), Some(main_exit));

        let mut connected = setup.sessions.clone();
        let connected = crate::supervised_session::AbortOnDrop::spawn(async move {
            loop {
                let up = connected.borrow_and_update().is_some();
                if events_tx.send(PlannerEvent::Connected(up)).is_err()
                    || connected.changed().await.is_err()
                {
                    break;
                }
            }
        });

        let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
        let mut main_up = setup.sessions;
        let anchor = setup.anchor.as_ref().map(|anchor| anchor.state());
        let controller = tokio::spawn(async move {
            let (_main_exit_tx, main_exit_rx) = tokio::sync::watch::channel(Some(main_exit));
            // Route sessions start once the main tunnel is up, as on desktop;
            // until then their apps are already blocked by the seeded policy.
            tokio::select! {
                up = main_up.wait_for(Option::is_some) => {
                    if up.is_err() {
                        return;
                    }
                }
                _ = &mut stopped => return,
            }
            controller
                .run(plan, main_exit_rx, anchor, async move {
                    let _ = stopped.await;
                })
                .await;
        });
        Self {
            table,
            tun: setup.tun,
            stop: Some(stop),
            controller: Some(controller),
            _planner: planner,
            _connected: connected,
        }
    }

    /// The main session's view of the TUN: a routed app's packets are taken
    /// out of it.
    pub(crate) fn main_device(
        &self,
    ) -> warren_app_routes::RoutedTun<warrenguard_transport::AndroidTun, Resolver> {
        warren_app_routes::RoutedTun::new(self.tun.clone(), Arc::clone(&self.table))
    }

    /// Stops every route session and waits until they released what they
    /// held.
    pub(crate) async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(controller) = self.controller.take() {
            let _ = controller.await;
        }
    }
}

impl Drop for AppRoutes {
    fn drop(&mut self) {
        if let Some(controller) = &self.controller {
            controller.abort();
        }
    }
}
