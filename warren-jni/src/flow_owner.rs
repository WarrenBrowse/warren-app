//! Which app owns a flow the Android TUN carries, for "Country per app"
//! (`docs/app-routing.md` sections 2.1 and 3.5).
//!
//! The desktop resolvers read the host's socket tables and name a process by
//! its executable. Android names an app by its uid instead:
//! `ConnectivityManager.getConnectionOwnerUid` (API 29+, callable by the
//! active VPN app) answers the uid owning the socket of a TCP or UDP 5-tuple
//! seen on the TUN, and `PackageManager.getPackagesForUid` names the packages
//! of that uid. Both are Binder calls, so the router's own rules keep them off
//! the hot path: one owner lookup per new flow, one package lookup per uid
//! (the router caches a decision per [`ProcessKey`]), never one per packet.
//!
//! The lookup is live, so there is no snapshot to take and no older view a
//! reused port could fool: [`OwnerResolver::refresh`] does nothing. A flow
//! whose owner the platform cannot name goes through the main session, as on
//! desktop (the router decides that, not this module).
//!
//! A uid may carry several packages (a shared user id). The packets of such a
//! uid cannot be told apart, so the uid takes the route of one of its
//! packages that has a country, the first by name: routing it is the side
//! that keeps a routed app's packets off the main session.

use std::{
    collections::HashMap,
    ffi::OsStr,
    net::SocketAddr,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use talpid_app_routing::{
    app::{AppMatcher, ProcessKey},
    flow::{FlowKey, Transport},
    owner::{OwnerError, OwnerResolver},
};

/// `IPPROTO_TCP` and `IPPROTO_UDP`, as `getConnectionOwnerUid` takes them.
pub(crate) const IPPROTO_TCP: i32 = 6;
pub(crate) const IPPROTO_UDP: i32 = 17;

/// The first app id of an app (`Process.FIRST_APPLICATION_UID`), in every
/// user and profile: below it are
/// the system's own uids, which never run an app the user picks.
const FIRST_APPLICATION_UID: u32 = 10_000;

/// The uids of one Android user or profile (`UserHandle.PER_USER_RANGE`): a
/// uid is the user id times this, plus the app id.
const PER_USER_RANGE: u32 = 100_000;

/// Advanced whenever the installed packages change, so a uid that now names
/// another package does not keep the decision taken for the one before.
static PACKAGE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Tells every resolver that the installed packages changed.
pub(crate) fn packages_changed() {
    PACKAGE_GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// The platform calls behind a lookup: the system boundary, faked in tests.
pub(crate) trait OwnerLookup: Send + 'static {
    /// The uid owning the socket of this flow, as the platform answers it:
    /// negative when it knows none.
    fn owner_uid(&self, protocol: i32, local: SocketAddr, remote: SocketAddr) -> i32;

    /// The packages of `uid`, empty when it has none the app may see.
    fn packages_for_uid(&self, uid: u32) -> Vec<String>;
}

/// An [`OwnerResolver`] over the Android platform calls. The "pid" the router
/// handles is the owning uid, and the "executable" of a uid is the package
/// name the policy matches.
pub(crate) struct AndroidOwnerResolver<L> {
    lookup: L,
    /// The packages of each uid asked about, for the generation they were
    /// read in.
    packages: HashMap<u32, Vec<String>>,
    generation: u64,
    /// The packages the policy names, which decide the package a shared uid
    /// is named after.
    watched: AppMatcher<()>,
}

impl<L: OwnerLookup> AndroidOwnerResolver<L> {
    pub(crate) fn new(lookup: L) -> Self {
        Self {
            lookup,
            packages: HashMap::new(),
            generation: PACKAGE_GENERATION.load(Ordering::Relaxed),
            watched: AppMatcher::new(
                talpid_app_routing::app::PathFlavor::Linux,
                Vec::<(&str, ())>::new(),
            ),
        }
    }

    /// Forgets the packages read before the installed packages changed.
    fn follow_package_changes(&mut self) {
        let generation = PACKAGE_GENERATION.load(Ordering::Relaxed);
        if generation != self.generation {
            self.generation = generation;
            self.packages.clear();
        }
    }
}

impl<L: OwnerLookup> OwnerResolver for AndroidOwnerResolver<L> {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        let protocol = match flow.transport {
            Transport::Tcp => IPPROTO_TCP,
            Transport::Udp => IPPROTO_UDP,
            // No platform call names the owner of an echo: it goes through
            // the main session, as on desktop.
            Transport::IcmpEcho => return None,
        };
        let uid = self.lookup.owner_uid(protocol, flow.local, flow.remote);
        u32::try_from(uid)
            .ok()
            .filter(|uid| uid % PER_USER_RANGE >= FIRST_APPLICATION_UID)
    }

    fn watch_programs<V: Copy>(&mut self, programs: &AppMatcher<V>) {
        self.watched = programs.apps_only();
    }

    fn refresh(&mut self) -> Result<(), OwnerError> {
        Ok(())
    }

    fn process_key(&mut self, uid: u32) -> Option<ProcessKey> {
        Some(ProcessKey {
            pid: uid,
            start_time: 0,
            image: PACKAGE_GENERATION.load(Ordering::Relaxed),
        })
    }

    fn executable(&mut self, uid: u32) -> Option<PathBuf> {
        self.follow_package_changes();
        let lookup = &self.lookup;
        let packages = self.packages.entry(uid).or_insert_with(|| {
            let mut packages = lookup.packages_for_uid(uid);
            packages.sort();
            packages
        });
        let watched = &self.watched;
        let chosen = packages
            .iter()
            .find(|package| watched.lookup(OsStr::new(package.as_str())).is_some())
            .or_else(|| packages.first())?;
        Some(PathBuf::from(chosen))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        sync::{Arc, Mutex},
    };

    use super::*;

    /// A pretend platform: which uid owns which local port, and which
    /// packages each uid carries. Counts the calls made to it.
    #[derive(Clone, Default)]
    struct FakePlatform {
        owners: Arc<Mutex<HashMap<(i32, u16), i32>>>,
        packages: Arc<Mutex<HashMap<u32, Vec<String>>>>,
        owner_calls: Arc<Mutex<Vec<(i32, SocketAddr, SocketAddr)>>>,
        package_calls: Arc<Mutex<u32>>,
    }

    impl FakePlatform {
        fn owns(&self, protocol: i32, port: u16, uid: i32) {
            self.owners.lock().unwrap().insert((protocol, port), uid);
        }

        fn app(&self, uid: u32, packages: &[&str]) {
            self.packages.lock().unwrap().insert(
                uid,
                packages
                    .iter()
                    .map(|package| (*package).to_owned())
                    .collect(),
            );
        }
    }

    impl OwnerLookup for FakePlatform {
        fn owner_uid(&self, protocol: i32, local: SocketAddr, remote: SocketAddr) -> i32 {
            self.owner_calls
                .lock()
                .unwrap()
                .push((protocol, local, remote));
            *self
                .owners
                .lock()
                .unwrap()
                .get(&(protocol, local.port()))
                .unwrap_or(&-1)
        }

        fn packages_for_uid(&self, uid: u32) -> Vec<String> {
            *self.package_calls.lock().unwrap() += 1;
            self.packages
                .lock()
                .unwrap()
                .get(&uid)
                .cloned()
                .unwrap_or_default()
        }
    }

    const TUN: Ipv4Addr = Ipv4Addr::new(10, 64, 0, 1);
    const REMOTE: Ipv4Addr = Ipv4Addr::new(198, 51, 100, 9);

    fn flow(transport: Transport, port: u16) -> FlowKey {
        FlowKey {
            transport,
            local: SocketAddr::new(IpAddr::V4(TUN), port),
            remote: SocketAddr::new(IpAddr::V4(REMOTE), 443),
        }
    }

    fn watching(resolver: &mut AndroidOwnerResolver<FakePlatform>, apps: &[&str]) {
        let matcher = AppMatcher::new(
            talpid_app_routing::app::PathFlavor::Linux,
            apps.iter().map(|app| (*app, 1u8)),
        );
        resolver.watch_programs(&matcher);
    }

    #[test]
    fn asks_the_platform_for_the_uid_owning_the_flow_as_the_tun_carries_it() {
        let platform = FakePlatform::default();
        platform.owns(IPPROTO_TCP, 40_001, 10_123);
        platform.owns(IPPROTO_UDP, 40_002, 10_124);
        let mut resolver = AndroidOwnerResolver::new(platform.clone());

        let tcp = resolver.socket_owner(&flow(Transport::Tcp, 40_001));
        let udp = resolver.socket_owner(&flow(Transport::Udp, 40_002));

        assert_eq!((tcp, udp), (Some(10_123), Some(10_124)));
        let calls = platform.owner_calls.lock().unwrap().clone();
        assert_eq!(
            calls[0],
            (
                IPPROTO_TCP,
                SocketAddr::new(IpAddr::V4(TUN), 40_001),
                SocketAddr::new(IpAddr::V4(REMOTE), 443)
            )
        );
    }

    #[test]
    fn an_unknown_owner_a_system_uid_and_an_echo_have_no_owner() {
        let platform = FakePlatform::default();
        platform.owns(IPPROTO_TCP, 40_003, 1_000);
        platform.owns(IPPROTO_TCP, 40_004, 1_001_000);
        let mut resolver = AndroidOwnerResolver::new(platform.clone());

        assert_eq!(resolver.socket_owner(&flow(Transport::Tcp, 40_009)), None);
        assert_eq!(resolver.socket_owner(&flow(Transport::Tcp, 40_003)), None);
        assert_eq!(
            resolver.socket_owner(&flow(Transport::Tcp, 40_004)),
            None,
            "the system uid of a work profile"
        );
        assert_eq!(resolver.socket_owner(&flow(Transport::IcmpEcho, 7)), None);
        assert_eq!(
            platform.owner_calls.lock().unwrap().len(),
            3,
            "an echo never reaches the platform"
        );
    }

    #[test]
    fn a_uid_is_named_after_its_package() {
        let platform = FakePlatform::default();
        platform.app(10_123, &["org.browser"]);
        let mut resolver = AndroidOwnerResolver::new(platform);

        assert_eq!(
            resolver.executable(10_123),
            Some(PathBuf::from("org.browser"))
        );
        assert_eq!(resolver.executable(10_999), None, "a uid with no package");
    }

    #[test]
    fn a_shared_uid_takes_the_name_of_its_package_with_a_country() {
        let platform = FakePlatform::default();
        platform.app(
            10_200,
            &["org.suite.mail", "org.suite.chat", "org.suite.core"],
        );
        let mut resolver = AndroidOwnerResolver::new(platform);

        let before = resolver.executable(10_200);
        watching(&mut resolver, &["org.suite.mail"]);
        let after = resolver.executable(10_200);

        assert_eq!(
            before,
            Some(PathBuf::from("org.suite.chat")),
            "first by name"
        );
        assert_eq!(after, Some(PathBuf::from("org.suite.mail")));
    }

    #[test]
    fn the_packages_of_a_uid_are_read_once_until_the_installed_packages_change() {
        let platform = FakePlatform::default();
        platform.app(10_300, &["org.old"]);
        let mut resolver = AndroidOwnerResolver::new(platform.clone());
        let before = resolver.process_key(10_300);

        let _ = resolver.executable(10_300);
        let _ = resolver.executable(10_300);
        platform.app(10_300, &["org.new"]);
        packages_changed();
        let renamed = resolver.executable(10_300);
        let after = resolver.process_key(10_300);

        assert_eq!(*platform.package_calls.lock().unwrap(), 2);
        assert_eq!(renamed, Some(PathBuf::from("org.new")));
        assert_ne!(before, after, "a decision for the old package is not kept");
    }

    #[test]
    fn the_live_lookup_needs_no_snapshot() {
        let mut resolver = AndroidOwnerResolver::new(FakePlatform::default());

        assert!(resolver.refresh().is_ok());
    }
}
