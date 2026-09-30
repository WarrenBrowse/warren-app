//! Re-applies the firewall when a Tailscale interface appears, goes away or changes name while
//! the tunnel is up.
//!
//! The firewall reads the host's interfaces each time it applies a policy, so a Tailscale that
//! starts after Warren connected stays blocked until something applies the policy again. The
//! routing socket announces every address and interface change; this task turns that stream into
//! one command per change of the SET of coexisting tailnet interfaces.

use std::collections::BTreeSet;
use std::sync::Weak;
use std::time::Duration;

use futures::{Stream, StreamExt, channel::mpsc};
use tokio::time::{Instant, timeout};

use super::TunnelCommand;
use crate::firewall::tailnet::SetTracker;

/// How long the host must be quiet before its interfaces are read. Tailscale brings its interface
/// up, then assigns its addresses, then adds routes, each announced separately.
pub(super) const SETTLE: Duration = Duration::from_millis(500);

/// How often Linux and Windows look at their interfaces, for want of an event source here.
#[cfg(any(target_os = "linux", windows))]
pub(super) const INTERFACE_POLL: Duration = Duration::from_secs(2);

/// The longest a steady stream of events can postpone a read.
const LONGEST_SETTLE: Duration = Duration::from_secs(5);

/// Sends [`TunnelCommand::TailnetInterfacesChanged`] each time the set `read_set` returns differs
/// from the previous one, after `events` (any interface or address change) went quiet for
/// `settle`. Ends when `events` ends or the state machine is gone.
pub(super) async fn run_tailnet_watch<E>(
    mut events: E,
    mut read_set: impl FnMut() -> BTreeSet<String>,
    command_tx: Weak<mpsc::UnboundedSender<TunnelCommand>>,
    settle: Duration,
) where
    E: Stream + Unpin,
{
    let mut tracker = SetTracker::new(read_set());
    while events.next().await.is_some() {
        let burst_start = Instant::now();
        loop {
            if burst_start.elapsed() >= LONGEST_SETTLE {
                break;
            }
            match timeout(settle, events.next()).await {
                Ok(Some(_)) => {}
                Ok(None) => return,
                Err(_quiet) => break,
            }
        }

        if tracker.observe(read_set()) {
            log::debug!("The set of coexisting tailnet interfaces changed");
            let Some(tx) = command_tx.upgrade() else {
                return;
            };
            let _ = tx.unbounded_send(TunnelCommand::TailnetInterfacesChanged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    struct Fixture {
        events: mpsc::UnboundedSender<()>,
        commands: mpsc::UnboundedReceiver<TunnelCommand>,
        host: Arc<Mutex<BTreeSet<String>>>,
        _command_tx: Arc<mpsc::UnboundedSender<TunnelCommand>>,
    }

    impl Fixture {
        /// Starts the watch and lets it take its initial reading of `initial`.
        async fn start(initial: BTreeSet<String>) -> Self {
            let (events, events_rx) = mpsc::unbounded();
            let (command_tx, commands) = mpsc::unbounded();
            let command_tx = Arc::new(command_tx);
            let host = Arc::new(Mutex::new(initial));
            let reader = host.clone();
            tokio::spawn(run_tailnet_watch(
                events_rx,
                move || reader.lock().unwrap().clone(),
                Arc::downgrade(&command_tx),
                SETTLE,
            ));
            tokio::task::yield_now().await;
            Self {
                events,
                commands,
                host,
                _command_tx: command_tx,
            }
        }

        async fn settle(&self) {
            tokio::task::yield_now().await;
            tokio::time::advance(SETTLE + Duration::from_millis(1)).await;
            tokio::task::yield_now().await;
        }

        fn notifications(&mut self) -> usize {
            let mut count = 0;
            while let Ok(command) = self.commands.try_recv() {
                assert!(matches!(command, TunnelCommand::TailnetInterfacesChanged));
                count += 1;
            }
            count
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_burst_that_adds_an_interface_notifies_once_after_it_settles() {
        let mut fixture = Fixture::start(set(&[])).await;
        *fixture.host.lock().unwrap() = set(&["utun13"]);

        for _ in 0..5 {
            fixture.events.unbounded_send(()).unwrap();
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_millis(100)).await;
        }
        assert_eq!(fixture.notifications(), 0, "still inside the burst");

        fixture.settle().await;
        assert_eq!(fixture.notifications(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn route_churn_that_leaves_the_set_alone_notifies_nothing() {
        let mut fixture = Fixture::start(set(&["utun13"])).await;

        for _ in 0..3 {
            fixture.events.unbounded_send(()).unwrap();
            fixture.settle().await;
        }

        assert_eq!(fixture.notifications(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn tailscale_stopping_after_it_started_notifies_each_time() {
        let mut fixture = Fixture::start(set(&[])).await;

        *fixture.host.lock().unwrap() = set(&["utun13"]);
        fixture.events.unbounded_send(()).unwrap();
        fixture.settle().await;
        assert_eq!(fixture.notifications(), 1);

        *fixture.host.lock().unwrap() = set(&[]);
        fixture.events.unbounded_send(()).unwrap();
        fixture.settle().await;
        assert_eq!(fixture.notifications(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_never_ending_stream_of_events_still_gets_read() {
        let mut fixture = Fixture::start(set(&[])).await;
        *fixture.host.lock().unwrap() = set(&["utun13"]);

        // An event every 100 ms never lets the host go quiet for `SETTLE`.
        for _ in 0..80 {
            fixture.events.unbounded_send(()).unwrap();
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }

        assert!(fixture.notifications() >= 1);
    }
}
