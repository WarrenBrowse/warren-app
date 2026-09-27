//! A main session that started without what its anchor needs gets it from
//! the first credentials refresh that brings it, without a reconnect.
//!
//! The anchor needs the route admission key from the token directory, and a
//! main session admitted on a token: it is sealed to that token's serial. A
//! fresh daemon has neither when its first tunnel starts, since both come
//! from the wallet's first refresh, which runs as that tunnel connects. So
//! the tunnel builds its anchor without a key when the directory has not
//! offered one yet ([`RouteAnchorHandle::awaiting_key`]), and on each refresh:
//!
//! - hands the anchor the key once the directory offers it; the engine then
//!   anchors the live main session at once when it was admitted on a token;
//! - sets the main session up again when it runs on the wallet and the
//!   refresh brought tokens, make before break (the engine's overlap
//!   reconnect), so the new session is admitted on a token and anchors. The
//!   exit places that session on another inner address than the wallet
//!   session's, which the desktop tunnel adopts by rebuilding itself and the
//!   Android one follows in place; the first tunnel of a run therefore waits a
//!   moment for the first tokens ([`first_refresh`]), so this is the
//!   exception.

use std::sync::Arc;

use tokio::sync::watch;
use warrenguard_transport::route_anchor::{RouteAnchorConfig, RouteAnchorHandle};

use crate::{Credentials, RouteAdmissionSource};

/// How long the first tunnel of a daemon run waits for the wallet's tokens of
/// the current epoch (or the end of its first refresh round) before its main
/// session is set up. The refresh starts as that tunnel connects and mints
/// the current epoch first; without those tokens the main session logs in
/// with the wallet, cannot anchor, and has to be set up again on a token once
/// they land, which moves it to another inner address and rebuilds the
/// tunnel. A refresh that does not come (the API blocked on this network)
/// costs the connection this much, once.
pub const FIRST_REFRESH_WAIT: std::time::Duration = std::time::Duration::from_secs(4);

/// The anchor of a main session whose tunnel runs per-app routes: with the
/// key the token directory offers, or waiting for one when it has offered
/// none yet (a first run), which [`follow_credentials`] hands over later.
#[must_use]
pub fn anchor_for(admission: &dyn RouteAdmissionSource) -> RouteAnchorHandle {
    match admission.kem() {
        Some(kem) => RouteAnchorHandle::new(RouteAnchorConfig { kem }),
        None => RouteAnchorHandle::awaiting_key(),
    }
}

/// Resolves once the wallet holds tokens for the current epoch or the
/// daemon finished its first credentials refresh round, at once when either
/// already holds (a later tunnel of the run, or a first round that failed),
/// and after `bound` at most.
pub async fn first_refresh(
    mut credentials: watch::Receiver<Credentials>,
    bound: std::time::Duration,
) {
    let refreshed = credentials.wait_for(|announced| announced.has_tokens || announced.rounds > 0);
    let _ = tokio::time::timeout(bound, refreshed).await;
}

/// Takes up what the credentials bring, starting with what is already there,
/// until the daemon stops announcing refreshes or the main session's
/// supervisor ends. `token_admission` is the engine's word on the live main
/// session (`MultiHopSupervisor::token_admission_rx`); `set_up_again` asks the
/// engine for a make-before-break setup of the main session, at most once per
/// tunnel: a setup that could not leave the wallet (every serial held
/// elsewhere) is not retried at every refresh.
pub async fn follow_credentials(
    mut credentials: watch::Receiver<Credentials>,
    mut token_admission: watch::Receiver<Option<bool>>,
    anchor: RouteAnchorHandle,
    admission: Arc<dyn RouteAdmissionSource>,
    set_up_again: impl Fn() + Send,
) {
    let mut asked = false;
    loop {
        let has_tokens = credentials.borrow_and_update().has_tokens;
        let on_wallet = *token_admission.borrow_and_update() == Some(false);
        if take_up(has_tokens, on_wallet && !asked, &anchor, admission.as_ref()) {
            log::info!(
                "App routing: tokens arrived; the main session is set up again on a token to anchor"
            );
            asked = true;
            set_up_again();
        }
        tokio::select! {
            changed = credentials.changed() => if changed.is_err() { return },
            changed = token_admission.changed() => if changed.is_err() { return },
        }
    }
}

/// Hands the anchor the key once the directory offers one, and says whether
/// the main session, `on_wallet`, should now be set up again on a token.
fn take_up(
    has_tokens: bool,
    on_wallet: bool,
    anchor: &RouteAnchorHandle,
    admission: &dyn RouteAdmissionSource,
) -> bool {
    if !anchor.has_key()
        && let Some(kem) = admission.kem()
        && anchor.provide_key(kem)
    {
        log::info!("App routing: the route admission key arrived; the main session anchors");
    }
    anchor.has_key() && has_tokens && on_wallet
}

#[cfg(test)]
mod tests {
    #[tokio::test(start_paused = true)]
    async fn a_first_tunnel_waits_for_the_first_refresh_and_no_longer() {
        let (credentials, followed) = watch::channel(Credentials::default());
        let started = tokio::time::Instant::now();
        let refresh = async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            credentials.send_replace(Credentials {
                rounds: 1,
                has_tokens: true,
            });
            std::future::pending::<()>().await;
        };

        tokio::select! {
            () = first_refresh(followed.clone(), FIRST_REFRESH_WAIT) => {}
            () = refresh => unreachable!(),
        }
        assert_eq!(started.elapsed(), std::time::Duration::from_secs(1));

        let again = tokio::time::Instant::now();
        first_refresh(followed, FIRST_REFRESH_WAIT).await;
        assert_eq!(
            again.elapsed(),
            std::time::Duration::ZERO,
            "a later tunnel does not wait"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_first_tunnel_goes_on_as_soon_as_the_first_tokens_are_minted() {
        let (credentials, followed) = watch::channel(Credentials::default());
        let started = tokio::time::Instant::now();
        let minted = async {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            credentials.send_modify(|announced| announced.has_tokens = true);
            std::future::pending::<()>().await;
        };

        tokio::select! {
            () = first_refresh(followed, FIRST_REFRESH_WAIT) => {}
            () = minted => unreachable!(),
        }

        assert_eq!(
            started.elapsed(),
            std::time::Duration::from_millis(700),
            "the rest of the round (later epochs) is not waited for"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_refresh_that_does_not_come_holds_the_tunnel_only_for_the_bound() {
        let (_credentials, followed) = watch::channel(Credentials::default());
        let started = tokio::time::Instant::now();

        first_refresh(followed, FIRST_REFRESH_WAIT).await;

        assert_eq!(started.elapsed(), FIRST_REFRESH_WAIT);
    }

    use std::sync::atomic::{AtomicUsize, Ordering};

    use warrenguard_multihop::{RouteKemPublicKey, RouteKemSecretKey};
    use warrenguard_transport::route_anchor::AnchorState;

    use super::*;

    /// A token directory whose route admission key can appear later.
    struct Directory(std::sync::Mutex<Option<RouteKemPublicKey>>);

    impl RouteAdmissionSource for Directory {
        fn kem(&self) -> Option<RouteKemPublicKey> {
            self.0.lock().unwrap().clone()
        }

        fn offers_routes(&self, _exit_id: &[u8; 16]) -> bool {
            false
        }
    }

    fn directory(offering: bool) -> Arc<Directory> {
        let kem = RouteKemSecretKey::derive(&[0x71; 32], 1)
            .unwrap()
            .public_key()
            .clone();
        Arc::new(Directory(std::sync::Mutex::new(offering.then_some(kem))))
    }

    #[test]
    fn a_main_anchor_takes_the_offered_key_or_waits_for_one() {
        assert!(anchor_for(directory(true).as_ref()).has_key());

        let waiting = anchor_for(directory(false).as_ref());

        assert!(!waiting.has_key());
        assert_eq!(waiting.current_state(), AnchorState::Unavailable);
    }

    #[test]
    fn a_refresh_that_brings_the_key_hands_it_to_the_anchor() {
        let anchor = RouteAnchorHandle::awaiting_key();

        let set_up_again = take_up(true, false, &anchor, directory(true).as_ref());

        assert!(anchor.has_key());
        assert!(
            !set_up_again,
            "a main session on a token anchors where it is"
        );
    }

    #[test]
    fn a_wallet_session_is_set_up_again_once_tokens_and_the_key_are_there() {
        let anchor = RouteAnchorHandle::awaiting_key();

        assert!(
            !take_up(false, true, &anchor, directory(true).as_ref()),
            "no token yet"
        );
        assert!(take_up(true, true, &anchor, directory(true).as_ref()));
    }

    #[test]
    fn without_a_key_on_offer_the_main_session_is_left_as_it_is() {
        let anchor = RouteAnchorHandle::awaiting_key();

        assert!(!take_up(true, true, &anchor, directory(false).as_ref()));
        assert!(!anchor.has_key());
        assert_eq!(anchor.current_state(), AnchorState::Unavailable);
    }

    struct Follower {
        credentials: watch::Sender<Credentials>,
        admission: watch::Sender<Option<bool>>,
        anchor: RouteAnchorHandle,
        setups: Arc<AtomicUsize>,
        task: tokio::task::JoinHandle<()>,
    }

    fn follower() -> Follower {
        let (credentials, credentials_rx) = watch::channel(Credentials::default());
        let (admission, admission_rx) = watch::channel(None);
        let anchor = RouteAnchorHandle::awaiting_key();
        let setups = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&setups);
        let task = tokio::spawn(follow_credentials(
            credentials_rx,
            admission_rx,
            anchor.clone(),
            directory(true),
            move || {
                counted.fetch_add(1, Ordering::SeqCst);
            },
        ));
        Follower {
            credentials,
            admission,
            anchor,
            setups,
            task,
        }
    }

    const REFRESHED: Credentials = Credentials {
        rounds: 1,
        has_tokens: true,
    };

    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn a_wallet_session_that_comes_up_after_the_refresh_is_set_up_again() {
        let follower = follower();
        follower.credentials.send_replace(REFRESHED);
        settle().await;
        assert!(follower.anchor.has_key());
        assert_eq!(follower.setups.load(Ordering::SeqCst), 0, "no session yet");

        follower.admission.send_replace(Some(false));
        settle().await;

        assert_eq!(follower.setups.load(Ordering::SeqCst), 1);
        follower.task.abort();
    }

    #[tokio::test]
    async fn a_wallet_session_is_set_up_again_once_per_tunnel() {
        let follower = follower();
        follower.admission.send_replace(Some(false));

        for round in 1..=3 {
            follower.credentials.send_replace(Credentials {
                rounds: round,
                has_tokens: true,
            });
            settle().await;
        }

        assert_eq!(follower.setups.load(Ordering::SeqCst), 1);
        follower.task.abort();
    }

    #[tokio::test]
    async fn a_session_on_a_token_is_never_set_up_again() {
        let follower = follower();
        follower.admission.send_replace(Some(true));

        follower.credentials.send_replace(REFRESHED);
        settle().await;

        assert!(follower.anchor.has_key());
        assert_eq!(follower.setups.load(Ordering::SeqCst), 0);
        follower.task.abort();
    }

    #[tokio::test]
    async fn the_follower_ends_with_the_main_session() {
        let follower = follower();

        drop(follower.admission);

        tokio::time::timeout(std::time::Duration::from_secs(5), follower.task)
            .await
            .expect("ends")
            .expect("not aborted");
    }
}
