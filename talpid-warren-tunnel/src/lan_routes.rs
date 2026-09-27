//! Local network sharing on the tunnel's routes.
//!
//! The Linux split-default sends every destination to the tunnel's table
//! ahead of `main`, so without an exception a shared local network (the LAN
//! gateway, a printer, a custom network) is routed into the tunnel even
//! though the firewall lets it out: it never answers, and an SSH session
//! opened from the LAN loses its replies. Each shared network is therefore
//! looked up in `main` ahead of the tunnel, in both families, where only a
//! route more specific than the default one takes it (the link itself, or a
//! static route): a shared network reached through the default gateway stays
//! in the tunnel, as with the plain split on macOS and Windows. The tunnel's
//! own address pool is kept in the tunnel ahead of every shared network, so
//! sharing `10.0.0.0/8` never hands the tunnel gateway to `main`, where a
//! route pushed by a hostile DHCP server would take it. The list is followed
//! live: turning sharing on or off, or editing the networks, never needs a
//! reconnect.
//!
//! macOS and Windows install the split as two `/1` routes in the one global
//! table, so a directly connected network, whose route is more specific,
//! already stays on its interface there.

use std::net::IpAddr;
use std::sync::Arc;

use ipnetwork::IpNetwork;
use tokio::sync::{Mutex, watch};
use warrenguard_route_split::bypass_cidr::BypassNetwork;
use warrenguard_route_split::default_route_split::{BypassNetworkRules, IpRules, SystemIp};

/// The networks to route outside the tunnel, as the tunnel state machine
/// publishes them: the shared networks while sharing is on, none while it
/// is off.
pub type LanNetworks = watch::Receiver<Vec<IpNetwork>>;

/// `networks` as routing exceptions. A network no exception can express (a
/// `/0`, which the settings refuse anyway) is left to the tunnel.
fn bypass_networks(networks: &[IpNetwork]) -> Vec<BypassNetwork> {
    networks
        .iter()
        .filter_map(|network| BypassNetwork::new(network.network(), network.prefix()).ok())
        .collect()
}

/// The tunnel's own address pools, in both families: its addresses, its
/// gateway and its resolver live there.
fn tunnel_networks() -> Vec<BypassNetwork> {
    [
        (
            IpAddr::V4(warrenguard_config::TUNNEL_POOL_NETWORK),
            warrenguard_config::TUNNEL_POOL_PREFIX,
        ),
        (
            IpAddr::V6(warrenguard_config::TUNNEL_POOL_NETWORK_V6),
            warrenguard_config::TUNNEL_POOL_PREFIX_V6,
        ),
    ]
    .into_iter()
    .filter_map(|(address, prefix)| BypassNetwork::new(address, prefix).ok())
    .collect()
}

/// The shared networks' routes for the life of one tunnel.
pub(crate) struct LanRoutes<R: IpRules + 'static = SystemIp> {
    rules: Arc<Mutex<BypassNetworkRules<R>>>,
    follower: Option<tokio::task::JoinHandle<()>>,
}

#[cfg_attr(not(target_os = "linux"), expect(dead_code))]
impl LanRoutes<SystemIp> {
    /// Routes the networks `lan` names outside the tunnel now, and follows
    /// every later change of the list until [`Self::stop`]. Installed before
    /// the split-default, so a shared network is never routed into the
    /// tunnel, not even while the tunnel comes up.
    pub(crate) async fn start(lan: LanNetworks) -> Self {
        Self::start_with(lan, BypassNetworkRules::new(tunnel_networks())).await
    }
}

impl<R: IpRules + 'static> LanRoutes<R> {
    async fn start_with(mut lan: LanNetworks, rules: BypassNetworkRules<R>) -> Self {
        let rules = Arc::new(Mutex::new(rules));
        let wanted = bypass_networks(&lan.borrow_and_update());
        apply(&rules, &wanted).await;
        let follower = tokio::spawn({
            let rules = Arc::clone(&rules);
            async move {
                while lan.changed().await.is_ok() {
                    let wanted = bypass_networks(&lan.borrow_and_update());
                    apply(&rules, &wanted).await;
                }
            }
        });
        Self {
            rules,
            follower: Some(follower),
        }
    }

    /// Stops following the list and removes every route it installed.
    #[cfg_attr(not(any(target_os = "linux", test)), expect(dead_code))]
    pub(crate) async fn stop(mut self) {
        let rules = Arc::clone(&self.rules);
        // Taken first: an update in flight finishes, so every rule it adds is
        // known and removed below.
        let mut rules = rules.lock().await;
        if let Some(follower) = self.follower.take() {
            follower.abort();
            let _ = follower.await;
        }
        rules.clear().await;
    }
}

impl<R: IpRules + 'static> Drop for LanRoutes<R> {
    // Without `stop` (a start that failed later on): the follower ends, and
    // the rules, once no task holds them, remove their routes on drop.
    fn drop(&mut self) {
        if let Some(follower) = self.follower.take() {
            follower.abort();
        }
    }
}

async fn apply<R: IpRules>(rules: &Mutex<BypassNetworkRules<R>>, wanted: &[BypassNetwork]) {
    let mut rules = rules.lock().await;
    match rules.apply(wanted).await {
        Ok(()) => log::info!(
            "Warren local network sharing: {} network(s) routed outside the tunnel",
            rules.installed().len()
        ),
        Err(error) => log::warn!(
            "Warren local network sharing: {} of {} network(s) routed outside the tunnel: {error}",
            rules.installed().len(),
            wanted.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use warrenguard_route_split::default_route_split::BypassChange;

    use super::*;

    fn net(s: &str) -> IpNetwork {
        s.parse().expect("network literal")
    }

    fn bypass(s: &str) -> BypassNetwork {
        let network = net(s);
        BypassNetwork::new(network.network(), network.prefix()).expect("valid network")
    }

    #[test]
    fn every_shared_network_of_either_family_becomes_an_exception() {
        let exceptions = bypass_networks(&[net("192.168.1.0/24"), net("fd00::/8")]);

        let rendered: Vec<String> = exceptions.iter().map(ToString::to_string).collect();
        assert_eq!(rendered, ["192.168.1.0/24", "fd00::/8"]);
    }

    #[test]
    fn a_whole_family_is_never_routed_outside_the_tunnel() {
        assert!(bypass_networks(&[net("0.0.0.0/0"), net("::/0")]).is_empty());
    }

    #[test]
    fn the_tunnel_pools_of_both_families_are_kept_in_the_tunnel() {
        assert_eq!(
            tunnel_networks(),
            [bypass("10.66.0.0/16"), bypass("fdcc:f:1::/64")]
        );
    }

    /// Records the `ip` commands the routes run.
    #[derive(Clone, Default)]
    struct Recorder(Arc<StdMutex<Vec<Vec<String>>>>);

    impl Recorder {
        fn take(&self) -> Vec<Vec<String>> {
            std::mem::take(&mut *self.0.lock().expect("lock"))
        }
    }

    impl IpRules for Recorder {
        async fn run(&self, args: Vec<String>) -> anyhow::Result<()> {
            self.0.lock().expect("lock").push(args);
            Ok(())
        }

        fn run_blocking(&self, args: &[String]) {
            self.0.lock().expect("lock").push(args.to_vec());
        }
    }

    fn commands(changes: &[BypassChange]) -> Vec<Vec<String>> {
        changes.iter().map(BypassChange::command).collect()
    }

    async fn started(lan: LanNetworks) -> (LanRoutes<Recorder>, Recorder) {
        let recorder = Recorder::default();
        let rules = BypassNetworkRules::with_runner(tunnel_networks(), recorder.clone());
        (LanRoutes::start_with(lan, rules).await, recorder)
    }

    #[tokio::test]
    async fn the_tunnel_pools_go_in_before_the_first_shared_network() {
        let (_sharing, lan) = watch::channel(vec![net("10.0.0.0/8")]);

        let (_routes, ran) = started(lan).await;

        assert_eq!(
            ran.take(),
            commands(&[
                BypassChange::KeepInTunnel(bypass("10.66.0.0/16")),
                BypassChange::KeepInTunnel(bypass("fdcc:f:1::/64")),
                BypassChange::Add(bypass("10.0.0.0/8")),
            ])
        );
    }

    #[tokio::test]
    async fn a_change_of_the_list_is_followed_while_the_tunnel_runs() {
        let (sharing, lan) = watch::channel(vec![net("10.0.0.0/8")]);
        let (_routes, ran) = started(lan).await;
        let _ = ran.take();

        sharing.send_replace(vec![net("198.18.0.0/15")]);

        let expected = commands(&[
            BypassChange::Add(bypass("198.18.0.0/15")),
            BypassChange::Remove(bypass("10.0.0.0/8")),
        ]);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut seen = Vec::new();
            while seen.len() < expected.len() {
                seen.extend(ran.take());
                tokio::task::yield_now().await;
            }
            assert_eq!(seen, expected);
        })
        .await
        .expect("the follower applies the new list");
    }

    #[tokio::test]
    async fn stopping_removes_every_rule_it_installed() {
        let (_sharing, lan) = watch::channel(vec![net("10.0.0.0/8"), net("fc00::/7")]);
        let (routes, ran) = started(lan).await;
        let _ = ran.take();

        routes.stop().await;

        assert_eq!(
            ran.take(),
            commands(&[
                BypassChange::Remove(bypass("10.0.0.0/8")),
                BypassChange::Remove(bypass("fc00::/7")),
                BypassChange::StopKeeping(bypass("10.66.0.0/16")),
                BypassChange::StopKeeping(bypass("fdcc:f:1::/64")),
            ])
        );
    }
}
