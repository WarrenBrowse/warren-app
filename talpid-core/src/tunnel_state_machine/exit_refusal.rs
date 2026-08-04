//! Whether an exit refusal is worth waiting for.
//!
//! An exit refuses a session in two situations that are indistinguishable on
//! the wire (the close code is opaque by design): the wallet HAS a
//! subscription the exit has not synced yet, and the refusal clears at its
//! next allowlist poll; or the wallet has none, and no amount of waiting
//! changes that. The client used to tell them apart with a stopwatch, which
//! meant assuming the exit's poll cadence and telling a paying user "account
//! is out of time" whenever an exit polled a few seconds late.
//!
//! The account API answers the question outright, and it is reachable exactly
//! when the client needs it: from inside the blocking state, by the address
//! the firewall already admits. So the refusal is classified from that answer,
//! and the timed guess survives only for the case where nothing answers.
//!
//! This is the one place that decides. The connecting state records how each
//! attempt ended and asks here before dialing again; nothing else times a
//! refusal.

use std::future::Future;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use talpid_types::tunnel::ErrorStateCause;

/// What the account API says about the wallet the tunnel authenticates with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    /// There is a subscription and it has not expired. An exit refusing this
    /// wallet is simply behind: its next allowlist poll admits the session.
    Active,
    /// The API answered and there is nothing for an exit to sync: no
    /// subscription, or one that has run out.
    Inactive,
    /// Nothing conclusive. The API was unreachable, answered an error, or no
    /// wallet is configured, so the refusal has to be judged on time alone.
    Unknown,
}

/// Answers whether the wallet the tunnel authenticates with has an active
/// subscription right now. Implemented by the daemon over the signed account
/// API; the state machine asks only after an exit has refused a session, so a
/// connect that works never costs a request.
pub trait SubscriptionStatusProvider: Send + Sync + 'static {
    /// The account API's current answer. Implementations must never fail the
    /// caller: anything they cannot establish is [`SubscriptionStatus::Unknown`].
    fn subscription_status(&self) -> Pin<Box<dyn Future<Output = SubscriptionStatus>>>;
}

/// How long a refusal is retried when the API could not say whether the
/// subscription exists.
///
/// This is the stopwatch, kept for the ambiguous case only. The value is the
/// one the client used for every refusal before it could ask: long enough to
/// outlast an exit's allowlist sync, short enough that a wallet with nothing
/// to sync gets an answer rather than a dark host. Every other case is decided
/// by the API and never looks at a clock.
const UNVERIFIED_PATIENCE: Duration = Duration::from_secs(45);

/// How long one answer is reused before the API is asked again.
///
/// A refusal loop can run for a minute, and one query per dial would turn a
/// self-healing refusal into a request flood. Re-asking matters mostly for
/// [`SubscriptionStatus::Unknown`]: it gives the API a few chances to come
/// back before the fallback patience runs out.
const STATUS_TTL: Duration = Duration::from_secs(15);

/// How long the connecting state waits for the API before falling back to the
/// timed guess. An answer that has not arrived by then would not save this
/// attempt anyway, and the run re-asks one [`STATUS_TTL`] later.
pub(super) const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// The floor between two dials once the API has confirmed the refusal will
/// clear.
///
/// Half the default anti-spin floor, and it applies to nothing else. The
/// client cannot know WHEN the exit will admit the wallet, so its whole
/// contribution to the wait is how late it dials after the admission lands:
/// at most one floor. Measured on the beta fleet: a wallet registered two
/// seconds before connecting was admitted between the second and third dial
/// in 6 runs out of 6, so the client was up to 1.01 s late for nothing.
/// Halving the floor halves that lateness and takes a refusal that never
/// clears from 9 to 10 dials per 30 s, which is what the shorter floor costs
/// the exit.
const CONFIRMED_TRANSIENT_FLOOR: Duration = Duration::from_millis(500);

/// Permission to pace the next dial by [`CONFIRMED_TRANSIENT_FLOOR`] instead
/// of the caller's anti-spin default.
///
/// The field is private to this module and no constructor hands out a `true`,
/// so [`lap_pacing`] is the only way to obtain that permission: no other
/// failure path in the state machine can dial twice as fast by asserting a
/// confidence it did not establish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LapPacing {
    confirmed_transient: bool,
}

impl LapPacing {
    /// What every lap gets unless the refusal policy says otherwise.
    pub(super) const ANTI_SPIN: Self = Self {
        confirmed_transient: false,
    };

    /// The permission itself. Private, so [`Refusals::pacing`] is the only
    /// code that can decide a refusal has earned it.
    const CONFIRMED: Self = Self {
        confirmed_transient: true,
    };

    /// Only for tests outside this module, which cannot build the permission
    /// and must still be able to pin what it does to the schedule.
    #[cfg(test)]
    pub(super) const CONFIRMED_TRANSIENT: Self = Self::CONFIRMED;

    /// The floor for the lap that just failed: `default` unless a refusal the
    /// API confirmed is waiting on nothing but the exit's next poll.
    pub(super) fn floor(self, default: Duration) -> Duration {
        if self.confirmed_transient {
            CONFIRMED_TRANSIENT_FLOOR
        } else {
            default
        }
    }
}

/// A run of consecutive exit refusals, and what the API said about it.
#[derive(Debug)]
struct Run {
    /// When the run opened, i.e. the first refusal that was not preceded by
    /// another refusal.
    started: Instant,
    /// The exit's own rejection message, carried so a surfaced refusal keeps
    /// the auth-failed token the app already localizes. No identity material:
    /// the exit's reason text is a product code, never an address or a key.
    reason: String,
    /// Last answer and when it was obtained, so the API is asked at most once
    /// per [`STATUS_TTL`] for the whole run.
    answer: Option<(SubscriptionStatus, Instant)>,
}

/// The refusal run in progress, if any.
///
/// Owns no policy of its own: every decision below is a method on this type,
/// which is why the policy is testable without driving a tunnel.
#[derive(Debug, Default)]
struct Refusals {
    run: Option<Run>,
}

impl Refusals {
    /// Record how one connect attempt ended: `Some(reason)` for an exit
    /// refusal, `None` for anything else. Any other failure ends the run, so
    /// patience is only ever spent on consecutive refusals.
    fn note_attempt(&mut self, rejection: Option<&str>, now: Instant) {
        match rejection {
            Some(reason) => {
                if let Some(run) = self.run.as_mut() {
                    run.reason = reason.to_owned();
                } else {
                    self.run = Some(Run {
                        started: now,
                        reason: reason.to_owned(),
                        answer: None,
                    });
                }
            }
            None => self.run = None,
        }
    }

    /// What the caller must supply before the next dial can be judged:
    /// `None` when no refusal run is open (dial, ask nothing), `Some(None)`
    /// when the API must be asked, `Some(Some(status))` when a fresh enough
    /// answer is already in hand.
    fn pending(&self, now: Instant) -> Option<Option<SubscriptionStatus>> {
        let run = self.run.as_ref()?;
        Some(run.answer.and_then(|(status, at)| {
            (now.saturating_duration_since(at) < STATUS_TTL).then_some(status)
        }))
    }

    /// Store an answer that was just obtained from the API, with the time it
    /// was obtained. Only a fresh answer is stamped: stamping a reused one
    /// would push the TTL forward on every lap and the run would never ask
    /// again, which is how an API that came back mid-run went unnoticed.
    fn record_answer(&mut self, status: SubscriptionStatus, now: Instant) {
        if let Some(run) = self.run.as_mut() {
            run.answer = Some((status, now));
        }
    }

    /// Report the state to park on, if any. A surfaced refusal ends the run, so
    /// a later attempt starts from a full budget and asks the API again.
    fn decide(&mut self, status: SubscriptionStatus, now: Instant) -> Option<ErrorStateCause> {
        let run = self.run.as_mut()?;
        match verdict(status, now.saturating_duration_since(run.started)) {
            Verdict::Retry => None,
            Verdict::Surface => {
                let run = self.run.take().expect("the run was borrowed just above");
                Some(ErrorStateCause::AuthFailed(Some(run.reason)))
            }
        }
    }

    /// The pacing the lap that just failed earns. A refusal only dials sooner
    /// once the API has answered `Active` for THIS run: the first refusal of a
    /// run has no answer yet, so the first two dials of any refusal stay a
    /// full default floor apart.
    ///
    /// The answer must still be fresh, by the same [`STATUS_TTL`] rule that
    /// governs reusing it for the verdict. A lap slow enough to outlive the
    /// answer that authorized it (a stalled dial, a long handshake before the
    /// refusal) is exactly a lap that should not be shortened on the strength
    /// of it.
    fn pacing(&self, now: Instant) -> LapPacing {
        match self.run.as_ref().and_then(|run| run.answer) {
            Some((SubscriptionStatus::Active, at))
                if now.saturating_duration_since(at) < STATUS_TTL =>
            {
                LapPacing::CONFIRMED
            }
            _ => LapPacing::ANTI_SPIN,
        }
    }

    fn forget(&mut self) {
        self.run = None;
    }
}

/// What to do with a refusal run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Keep dialing. The refusal is known to clear, or not yet known to be
    /// permanent.
    Retry,
    /// Stop dialing in silence and park on a readable, cancelable state.
    Surface,
}

/// The whole policy, in one pure function.
///
/// An active subscription puts no deadline on the wait: the refusal WILL clear
/// once the exit polls, the retry interval already keeps the cost down, and a
/// paying user must never be told their account has run out. Nothing to sync
/// is surfaced at the first refusal, because that is what is true. An
/// unanswered query is the only case still judged on time.
fn verdict(status: SubscriptionStatus, refusing_for: Duration) -> Verdict {
    match status {
        SubscriptionStatus::Active => Verdict::Retry,
        SubscriptionStatus::Inactive => Verdict::Surface,
        SubscriptionStatus::Unknown if refusing_for >= UNVERIFIED_PATIENCE => Verdict::Surface,
        SubscriptionStatus::Unknown => Verdict::Retry,
    }
}

/// The run in progress. Process-global because a run spans several state
/// objects: every lap builds a fresh `ConnectingState`.
static REFUSALS: LazyLock<Mutex<Refusals>> = LazyLock::new(|| Mutex::new(Refusals::default()));

fn refusals() -> std::sync::MutexGuard<'static, Refusals> {
    REFUSALS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Record how one connect attempt ended. See [`Refusals::note_attempt`].
pub(super) fn note_attempt(rejection: Option<&str>, now: Instant) {
    refusals().note_attempt(rejection, now);
}

/// Forget the run, so the next refusal starts from a full budget and a fresh
/// question to the API. Called on a user-initiated connect.
pub(super) fn forget_run() {
    refusals().forget();
}

/// The pacing the lap that just failed earns. See [`Refusals::pacing`].
pub(super) fn lap_pacing(now: Instant) -> LapPacing {
    refusals().pacing(now)
}

/// Whether the last connect attempt ended in an exit refusal.
///
/// The connecting state asks so it can keep a refusal loop out of the flap
/// detector: the two bound the same loop, and the flap detector would stop it
/// after six laps regardless of what the API said.
pub(super) fn run_is_open() -> bool {
    refusals().run.is_some()
}

/// The state to park on instead of dialing again, or `None` to dial.
///
/// `ask` is only called when a refusal run is open and no fresh answer is in
/// hand, so a connect that never gets refused costs no request, and a refusal
/// loop costs at most one per [`STATUS_TTL`]. It runs with no lock held: it
/// reaches the network, and nothing else may wait on that.
pub(super) fn block_reason_before_dialing(
    now: Instant,
    ask: impl FnOnce() -> SubscriptionStatus,
) -> Option<ErrorStateCause> {
    let answer = refusals().pending(now)?;
    let status = match answer {
        Some(status) => status,
        None => {
            let status = ask();
            refusals().record_answer(status, now);
            status
        }
    };
    let block_reason = refusals().decide(status, now);
    if block_reason.is_some() {
        log::error!(
            "The exit refused the session and the client has no reason to expect it \
             to clear (account API: {status:?}). Surfacing it instead of dialing in \
             silence."
        );
    }
    block_reason
}

#[cfg(test)]
mod tests {
    use super::*;

    const REASON: &str = "[EXPIRED_ACCOUNT] exit rejected the session";

    fn refused(refusals: &mut Refusals, at: Instant) {
        refusals.note_attempt(Some(REASON), at);
    }

    #[test]
    fn nothing_is_decided_while_no_exit_has_refused() {
        let refusals = Refusals::default();
        assert!(
            refusals.pending(Instant::now()).is_none(),
            "a connect that was never refused must not cost an API request"
        );
    }

    #[test]
    fn an_active_subscription_is_waited_out_however_late_the_exit_polls() {
        // The whole point: patience is no longer a margin over an assumed poll
        // cadence, so an exit that is slow can never produce an account
        // verdict against a paying user.
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);

        for elapsed in [
            Duration::from_secs(1),
            UNVERIFIED_PATIENCE,
            Duration::from_secs(3600),
        ] {
            assert!(
                refusals
                    .decide(SubscriptionStatus::Active, started + elapsed)
                    .is_none(),
                "a refusal against an active subscription must be retried, not \
                 surfaced ({elapsed:?})"
            );
        }
    }

    #[test]
    fn no_subscription_is_surfaced_at_the_very_first_refusal() {
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);

        match refusals.decide(SubscriptionStatus::Inactive, started) {
            Some(ErrorStateCause::AuthFailed(Some(reason))) => assert_eq!(
                reason, REASON,
                "the exit's own reason must reach the app, so it keeps its localized copy"
            ),
            other => panic!("a refusal with nothing to sync must surface at once, got {other:?}"),
        }
    }

    #[test]
    fn an_unanswered_query_falls_back_to_the_timed_patience() {
        let started = Instant::now();

        let mut early = Refusals::default();
        refused(&mut early, started);
        assert!(
            early
                .decide(
                    SubscriptionStatus::Unknown,
                    started + UNVERIFIED_PATIENCE - Duration::from_millis(1)
                )
                .is_none(),
            "an unverified refusal must still be retried inside the fallback patience"
        );

        let mut late = Refusals::default();
        refused(&mut late, started);
        assert!(
            late.decide(SubscriptionStatus::Unknown, started + UNVERIFIED_PATIENCE)
                .is_some(),
            "an unverified refusal must be surfaced once the fallback patience is spent"
        );
    }

    #[test]
    fn one_answer_serves_a_whole_ttl_so_a_refusal_loop_is_not_a_request_flood() {
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);
        refusals.record_answer(SubscriptionStatus::Active, started);

        assert_eq!(
            refusals.pending(started + STATUS_TTL - Duration::from_millis(1)),
            Some(Some(SubscriptionStatus::Active)),
            "an answer inside its TTL must be reused rather than re-asked"
        );
        assert_eq!(
            refusals.pending(started + STATUS_TTL),
            Some(None),
            "a stale answer must be re-asked, or an unreachable API would never \
             get a second chance"
        );
    }

    #[test]
    fn reusing_an_answer_does_not_extend_its_life() {
        // Measured on the fleet: with the answer re-stamped on every lap, a
        // 51 s refusal run asked the API exactly once, so an API that came
        // back mid-run was never noticed.
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);
        refusals.record_answer(SubscriptionStatus::Unknown, started);

        for lap in 1..STATUS_TTL.as_secs() {
            let now = started + Duration::from_secs(lap);
            let status = refusals.pending(now).expect("the run is open").unwrap();
            refusals.decide(status, now);
        }

        assert_eq!(
            refusals.pending(started + STATUS_TTL),
            Some(None),
            "the TTL runs from the answer, not from the last lap that reused it"
        );
    }

    #[test]
    fn any_other_failure_ends_the_run_so_patience_starts_over() {
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);
        refusals.decide(SubscriptionStatus::Unknown, started);

        refusals.note_attempt(None, started + Duration::from_secs(1));
        assert!(
            refusals.pending(started + Duration::from_secs(1)).is_none(),
            "a lap that failed for another reason must close the refusal run"
        );

        refused(&mut refusals, started + Duration::from_secs(2));
        assert!(
            refusals
                .decide(
                    SubscriptionStatus::Unknown,
                    started + Duration::from_secs(2) + UNVERIFIED_PATIENCE
                        - Duration::from_millis(1)
                )
                .is_none(),
            "the new run must be timed from its own first refusal"
        );
    }

    #[test]
    fn a_surfaced_refusal_ends_the_run() {
        // The daemon reconnects out of the blocked state on its own timer; that
        // attempt must ask the API again rather than resurface the old verdict.
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);

        assert!(
            refusals
                .decide(SubscriptionStatus::Inactive, started)
                .is_some()
        );
        assert!(
            refusals.pending(started).is_none(),
            "a surfaced refusal must leave no run behind"
        );
    }

    #[test]
    fn a_run_is_open_only_between_a_refusal_and_the_next_other_outcome() {
        // What the flap detector reads to leave a refusal loop alone.
        let mut refusals = Refusals::default();
        let started = Instant::now();
        assert!(refusals.run.is_none());

        refused(&mut refusals, started);
        assert!(refusals.run.is_some(), "a refusal must open a run");

        refusals.note_attempt(None, started);
        assert!(
            refusals.run.is_none(),
            "any other outcome must close it, so ordinary flapping is still caught"
        );
    }

    #[test]
    fn the_shorter_floor_is_earned_only_by_a_refusal_the_api_confirmed() {
        let mut refusals = Refusals::default();
        let started = Instant::now();
        assert_eq!(
            refusals.pacing(started),
            LapPacing::ANTI_SPIN,
            "with no refusal at all, the anti-spin floor is the only one"
        );

        refused(&mut refusals, started);
        assert_eq!(
            refusals.pacing(started),
            LapPacing::ANTI_SPIN,
            "the first refusal of a run has no answer yet, so the first two \
             dials stay a full floor apart"
        );

        refusals.record_answer(SubscriptionStatus::Active, started);
        assert!(
            refusals.pacing(started).floor(Duration::from_secs(1)) == CONFIRMED_TRANSIENT_FLOOR,
            "a refusal against an active subscription is the one case that dials sooner"
        );

        for unconfirmed in [SubscriptionStatus::Unknown, SubscriptionStatus::Inactive] {
            refusals.record_answer(unconfirmed, started);
            assert_eq!(
                refusals.pacing(started),
                LapPacing::ANTI_SPIN,
                "{unconfirmed:?} confirms nothing, so the floor stays where it was"
            );
        }

        refusals.record_answer(SubscriptionStatus::Active, started);
        refusals.note_attempt(None, started);
        assert_eq!(
            refusals.pacing(started),
            LapPacing::ANTI_SPIN,
            "a lap that failed for another reason must not inherit the refusal's pacing"
        );
    }

    #[test]
    fn an_answer_too_old_to_reuse_is_too_old_to_dial_faster_on() {
        // The pacing is read at the END of a lap, the answer was stamped at its
        // start, so a lap slow enough to outlive its own answer must fall back
        // to the anti-spin floor rather than shorten itself on a confirmation
        // this module would already refuse to reuse for the verdict.
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);
        refusals.record_answer(SubscriptionStatus::Active, started);

        assert_eq!(
            refusals.pacing(started + STATUS_TTL - Duration::from_millis(1)),
            LapPacing::CONFIRMED,
            "a fresh confirmation still earns the shorter floor"
        );
        assert_eq!(
            refusals.pacing(started + STATUS_TTL),
            LapPacing::ANTI_SPIN,
            "past the TTL the confirmation is stale, and so is the permission"
        );
    }

    #[test]
    fn a_user_initiated_connect_forgets_the_run() {
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);

        refusals.forget();

        assert!(
            refusals.pending(started).is_none(),
            "pressing Connect must give the exit a full budget again"
        );
    }

    #[test]
    fn the_latest_refusal_reason_is_the_one_surfaced() {
        // The exit may sharpen its reason mid-run; the user must read the last
        // one, not the first.
        let mut refusals = Refusals::default();
        let started = Instant::now();
        refused(&mut refusals, started);
        refusals.note_attempt(Some("[EXPIRED_ACCOUNT] later reason"), started);

        match refusals.decide(SubscriptionStatus::Inactive, started) {
            Some(ErrorStateCause::AuthFailed(Some(reason))) => {
                assert_eq!(reason, "[EXPIRED_ACCOUNT] later reason");
            }
            other => panic!("expected a surfaced refusal, got {other:?}"),
        }
    }
}
