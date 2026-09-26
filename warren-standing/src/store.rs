//! One process's holder of the wallet's standing on the mobile clients
//! (warren-core doc 105 §5.3, §5.4), shared by `warren-jni` and `warren-ios`
//! so the two cannot drift on the envelope the app renders or on when the
//! tunnel blocks: the mobile twin of the desktop daemon's
//! `mullvad-daemon::warren_account_standing`.
//!
//! Three inputs feed one [`StandingTracker`]:
//! - the app's poll of the signed `GET /v1/account/standing`, whose JSON
//!   envelope ([`StandingStore::on_poll`]) is what the app renders;
//! - the ban refusal the token and entitlement issuers answer, reported by
//!   their background refresh tasks ([`StandingStore::on_refresh_error`]). A
//!   v7 client never presents its wallet to an exit, so this is how it learns
//!   of a ban at all, and it arrives before any dial;
//! - an exit's `RejectedBanned` answer on the v6 path
//!   ([`StandingStore::on_exit_ban`]).
//!
//! The tunnel reads [`StandingStore::ban_in_force`] before it dials and
//! watches [`StandingStore::subscribe`] while a session runs, so a ban learned
//! mid-session ends it the way the desktop daemon blocks its tunnel.
//!
//! Which strikes this device already announced survives a process death in
//! [`LEDGER_FILE`] in the app's private storage, which holds SHA-256 digests
//! only, never a case reference. The wallet is keyed by its public key in
//! memory and never logged.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use warren_api::{AccountStandingResponse, ClientError, TokenClientError};

use crate::{Ban, NewStrike, Standing, StandingTracker, StrikeLedger};

/// File name of the strike ledger in the app files directory.
pub const LEDGER_FILE: &str = "warren-strike-ledger.json";

/// The wallet key the tracker compares: the public key in hex. Compared,
/// never logged.
fn wallet_key(wallet_pubkey: &[u8; 32]) -> String {
    hex::encode(wallet_pubkey)
}

struct Inner {
    tracker: StandingTracker,
    /// The wallet the tracker last heard about, so a poll that failed for
    /// another wallet never answers the previous one's standing.
    wallet: Option<String>,
    /// Wallets that left this device. Their token and entitlement refresh
    /// loops outlive them, and a ban refusal one of those loops reports says
    /// nothing about the wallet now installed: taken as news, it would switch
    /// the tracker back to the departed wallet and drop the current one's ban.
    departed: std::collections::HashSet<String>,
}

/// Process-lived holder of the wallet's standing.
pub struct StandingStore {
    inner: Mutex<Inner>,
    ledger_path: Option<PathBuf>,
    /// Bumped on every change, for the tunnel to notice a ban mid-session.
    changed: tokio::sync::watch::Sender<u64>,
}

impl StandingStore {
    /// A store that remembers the strikes the ledger at `ledger_path` lists as
    /// announced. `None` keeps the ledger in memory only.
    #[must_use]
    pub fn new(ledger_path: Option<PathBuf>) -> Self {
        let ledger = ledger_path.as_deref().map(load_ledger).unwrap_or_default();
        Self {
            inner: Mutex::new(Inner {
                tracker: StandingTracker::new(ledger),
                wallet: None,
                departed: std::collections::HashSet::new(),
            }),
            ledger_path,
            changed: tokio::sync::watch::Sender::new(0),
        }
    }

    /// A token or entitlement refresh of `wallet_pubkey` failed with `error`.
    /// Records the ban it carries, if it is the issuer's ban refusal, and
    /// answers whether it was one.
    pub fn on_refresh_error(
        &self,
        wallet_pubkey: &[u8; 32],
        error: &TokenClientError,
        now_unix_secs: u64,
    ) -> bool {
        let Some(ban) = Ban::from_refresh_error(error) else {
            return false;
        };
        self.record_ban(wallet_pubkey, ban, now_unix_secs);
        true
    }

    /// A call that credits time (a voucher redemption, a store payment call)
    /// of `wallet_pubkey` failed with `error`. Records the ban it carries, if
    /// it is the ban refusal, and answers the ban in force then: the one the
    /// standing already knew when it holds, since the refusal of the unsigned
    /// `/v1/register` may not say when it lapses.
    pub fn on_ban_refusal(
        &self,
        wallet_pubkey: &[u8; 32],
        error: &ClientError,
        now_unix_secs: u64,
    ) -> Option<Ban> {
        let ban = Ban::from_client_error(error)?;
        self.record_ban(wallet_pubkey, ban, now_unix_secs);
        Some(
            self.ban_in_force(wallet_pubkey, now_unix_secs)
                .unwrap_or(ban),
        )
    }

    /// An exit refused `wallet_pubkey` as banned with the opaque reason
    /// `code`. Answers the ban the session ends on: the one the standing or an
    /// issuer reported when it still holds, since that one knows its lapse.
    ///
    /// The exit's answer is not recorded as a block for the next dial, the
    /// desktop daemon's rule: it carries no lapse, so nothing but a standing
    /// answer could ever clear it, and the next dial is refused by the exit
    /// anyway while the ban holds.
    #[must_use]
    pub fn on_exit_ban(&self, wallet_pubkey: &[u8; 32], code: u8, now_unix_secs: u64) -> Ban {
        self.ban_in_force(wallet_pubkey, now_unix_secs)
            .unwrap_or_else(|| Ban::from_exit_rejection(code))
    }

    /// The app's poll of the standing endpoint for `wallet_pubkey` answered
    /// `result`. Answers the JSON envelope the app renders:
    ///
    /// `{"ok":bool,"reported":bool,"standing":null|{..},"new_strikes":[..]}`
    ///
    /// `reported` is false on a `404`, an API that does not serve the
    /// standing yet: nothing is wrong and nothing is shown for it. `ok` is
    /// false on any other failure, which the app retries. `standing` carries
    /// what is known either way, so a ban learned from an issuer shows even
    /// when the endpoint is absent.
    pub fn on_poll(
        &self,
        wallet_pubkey: &[u8; 32],
        result: Result<AccountStandingResponse, ClientError>,
        now_unix_secs: u64,
    ) -> String {
        let wallet = wallet_key(wallet_pubkey);
        let mut to_persist = None;
        let mut inner = self.lock();
        let (ok, reported, new_strikes) = match result {
            Ok(response) => {
                let before = inner.tracker.ledger().clone();
                inner.departed.remove(&wallet);
                inner.wallet = Some(wallet.clone());
                let update = inner.tracker.on_standing(&wallet, response);
                if *inner.tracker.ledger() != before {
                    to_persist = Some(inner.tracker.ledger().clone());
                }
                if update.is_some() {
                    self.changed.send_modify(|n| *n = n.wrapping_add(1));
                }
                (
                    true,
                    true,
                    update.map(|u| u.new_strikes).unwrap_or_default(),
                )
            }
            Err(ClientError::ServerStatus { status: 404, .. }) => (true, false, Vec::new()),
            Err(_) => (false, true, Vec::new()),
        };
        let standing = (inner.wallet.as_deref() == Some(wallet.as_str()))
            .then(|| inner.tracker.standing().cloned())
            .flatten();
        drop(inner);
        // Written with the lock released: the tunnel reads the store from its
        // runtime workers, and a slow disk must not stall them.
        if let Some(ledger) = to_persist {
            self.persist(&ledger);
        }
        envelope(ok, reported, standing.as_ref(), &new_strikes, now_unix_secs)
    }

    /// The ban holding `wallet_pubkey` at `now_unix_secs`, if any.
    pub fn ban_in_force(&self, wallet_pubkey: &[u8; 32], now_unix_secs: u64) -> Option<Ban> {
        let inner = self.lock();
        if inner.wallet.as_deref() != Some(wallet_key(wallet_pubkey).as_str()) {
            return None;
        }
        inner.tracker.ban_in_force(now_unix_secs)
    }

    /// The wallet left this device: its standing and which of its strikes
    /// were announced are forgotten, on disk too.
    pub fn forget(&self) {
        let mut inner = self.lock();
        if let Some(departed) = inner.wallet.take() {
            inner.departed.insert(departed);
        }
        let had_ledger = *inner.tracker.ledger() != StrikeLedger::new();
        if inner.tracker.on_no_wallet().is_some() {
            self.changed.send_modify(|n| *n = n.wrapping_add(1));
        }
        drop(inner);
        if had_ledger {
            self.persist(&StrikeLedger::new());
        }
    }

    /// A receiver that wakes on every change of the standing.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changed.subscribe()
    }

    /// The ban on `wallet_pubkey`, once one holds at `now()`: at once when
    /// one already does, else when the standing learns of one (an issuer's
    /// refusal from a background refresh, a standing answer). Pending forever
    /// when none comes. A tunnel selects on it to end a live session on a ban.
    pub async fn ban_arrives(&self, wallet_pubkey: [u8; 32], now: impl Fn() -> u64) -> Ban {
        // Subscribed before the first look, so a ban recorded in between
        // still wakes the loop.
        let mut changes = self.subscribe();
        loop {
            if let Some(ban) = self.ban_in_force(&wallet_pubkey, now()) {
                return ban;
            }
            if changes.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic under the lock leaves a tracker that is still coherent (each
        // method mutates it in one call), so the next caller carries on.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn record_ban(&self, wallet_pubkey: &[u8; 32], ban: Ban, now_unix_secs: u64) {
        let wallet = wallet_key(wallet_pubkey);
        let mut inner = self.lock();
        if inner.departed.contains(&wallet) {
            return;
        }
        inner.wallet = Some(wallet.clone());
        if inner
            .tracker
            .on_issuance_ban(&wallet, ban, now_unix_secs)
            .is_some()
        {
            self.changed.send_modify(|n| *n = n.wrapping_add(1));
        }
    }

    fn persist(&self, ledger: &StrikeLedger) {
        let Some(path) = self.ledger_path.as_deref() else {
            return;
        };
        if let Err(error) = write_private(path, ledger.to_json().as_bytes()) {
            log::warn!("Could not save the Warren strike ledger: {}", error.kind());
        }
    }
}

/// The ban a blocked session names to the app (`getBanVerdict` on Android):
/// `{"reason":"[BANNED_PORT_FORWARDING] ...","lapses_at_unix_secs":N|null}`,
/// or `{}` when the session was not blocked for a ban.
#[must_use]
pub fn ban_verdict_json(ban: Option<&Ban>) -> String {
    match ban {
        Some(ban) => serde_json::json!({
            "reason": ban.auth_failed_reason(),
            "lapses_at_unix_secs": ban.lapses_at_unix_secs,
        })
        .to_string(),
        None => "{}".to_owned(),
    }
}

fn envelope(
    ok: bool,
    reported: bool,
    standing: Option<&Standing>,
    new_strikes: &[NewStrike],
    now_unix_secs: u64,
) -> String {
    let standing = standing.map(|standing| {
        serde_json::json!({
            "strikes": standing.strikes,
            "threshold": standing.threshold,
            "window_days": standing.window_days,
            "ban": standing.ban.map(|ban| serde_json::json!({
                "reason": ban.reason,
                "banned_at_unix_secs": ban.banned_at_unix_secs,
                "lapses_at_unix_secs": ban.lapses_at_unix_secs,
                "in_force": ban.in_force(now_unix_secs),
                "block_reason": ban.auth_failed_reason(),
            })),
        })
    });
    serde_json::json!({
        "ok": ok,
        "reported": reported,
        "standing": standing,
        "new_strikes": new_strikes,
    })
    .to_string()
}

fn load_ledger(path: &Path) -> StrikeLedger {
    match std::fs::read_to_string(path) {
        Ok(json) => StrikeLedger::from_json(&json).unwrap_or_else(|_| {
            // Starting over costs one repeated warning per live strike, which
            // beats never warning again.
            log::warn!("Ignoring an unreadable Warren strike ledger");
            StrikeLedger::new()
        }),
        Err(_) => StrikeLedger::new(),
    }
}

/// Writes `bytes` to `path` through a private temporary file and a rename, so
/// a process death mid-write never leaves a torn ledger.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let written = std::fs::write(&tmp, bytes).and_then(|()| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, path)
    });
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use warren_api::{
        AccountBan, AccountStandingResponse, BanReasonCode, ClientError, TokenClientError,
    };

    use super::*;

    const WALLET: [u8; 32] = [7; 32];
    const OTHER: [u8; 32] = [9; 32];
    const NOW: u64 = 1_800_000_000;

    fn strike(reference: &str, port: u16) -> Value {
        serde_json::json!({
            "day_unix_secs": 1_790_000_000u64,
            "category": "copyright",
            "exit_country": "FI",
            "port": port,
            "case_reference": reference,
        })
    }

    fn answer(strikes: &[(&str, u16)], ban: Option<AccountBan>) -> AccountStandingResponse {
        AccountStandingResponse {
            strikes: strikes
                .iter()
                .map(|(r, p)| serde_json::from_value(strike(r, *p)).unwrap())
                .collect(),
            threshold: 3,
            window_days: 90,
            ban,
        }
    }

    fn account_ban(lapses_at: u64) -> AccountBan {
        AccountBan {
            banned_at_unix_secs: NOW - 10,
            lapses_at_unix_secs: Some(lapses_at),
            reason_code: BanReasonCode::PortForwardingAbuse,
        }
    }

    fn banned_refusal() -> TokenClientError {
        TokenClientError::Api(ClientError::Banned {
            reason_code: BanReasonCode::PortForwardingAbuse,
            lapses_at_unix_secs: Some(NOW + 100),
        })
    }

    fn parse(json: &str) -> Value {
        serde_json::from_str(json).expect("the envelope is JSON")
    }

    #[test]
    fn a_standing_answer_publishes_the_strikes_and_warns_about_each_new_one_once() {
        let store = StandingStore::new(None);

        let first = parse(&store.on_poll(&WALLET, Ok(answer(&[("PF-1", 50000)], None)), NOW));
        let again = parse(&store.on_poll(&WALLET, Ok(answer(&[("PF-1", 50000)], None)), NOW));

        assert_eq!(first["ok"], true);
        assert_eq!(first["reported"], true);
        assert_eq!(first["standing"]["strikes"][0]["port"], 50000);
        assert_eq!(first["standing"]["strikes"][0]["case_reference"], "PF-1");
        assert_eq!(first["standing"]["threshold"], 3);
        assert_eq!(first["standing"]["ban"], Value::Null);
        assert_eq!(first["new_strikes"][0]["ordinal"], 1);
        assert_eq!(first["new_strikes"][0]["threshold"], 3);
        assert_eq!(first["new_strikes"][0]["strike"]["case_reference"], "PF-1");
        assert_eq!(again["new_strikes"], serde_json::json!([]));
        assert_eq!(again["standing"]["strikes"][0]["port"], 50000);
    }

    #[test]
    fn a_ban_in_the_answer_carries_its_lapse_and_the_token_the_tunnel_blocks_with() {
        let store = StandingStore::new(None);

        let json =
            parse(&store.on_poll(&WALLET, Ok(answer(&[], Some(account_ban(NOW + 60)))), NOW));

        let ban = &json["standing"]["ban"];
        assert_eq!(ban["reason"], "port_forwarding_abuse");
        assert_eq!(ban["lapses_at_unix_secs"], NOW + 60);
        assert_eq!(ban["in_force"], true);
        assert!(
            ban["block_reason"]
                .as_str()
                .unwrap()
                .starts_with("[BANNED_PORT_FORWARDING] ")
        );
        assert!(store.ban_in_force(&WALLET, NOW).is_some());
    }

    #[test]
    fn an_api_without_the_endpoint_is_not_reported_and_is_not_a_failure() {
        let store = StandingStore::new(None);

        let json = parse(&store.on_poll(
            &WALLET,
            Err(ClientError::ServerStatus {
                status: 404,
                body: String::new(),
            }),
            NOW,
        ));

        assert_eq!(json["ok"], true);
        assert_eq!(json["reported"], false);
        assert_eq!(json["standing"], Value::Null);
    }

    #[test]
    fn any_other_failure_is_not_ok_and_keeps_what_was_known() {
        let store = StandingStore::new(None);
        store.on_poll(&WALLET, Ok(answer(&[("PF-1", 50000)], None)), NOW);

        let json = parse(&store.on_poll(
            &WALLET,
            Err(ClientError::ServerStatus {
                status: 503,
                body: "wallet 5Grw... echoed".to_owned(),
            }),
            NOW,
        ));

        assert_eq!(json["ok"], false);
        assert_eq!(json["standing"]["strikes"][0]["case_reference"], "PF-1");
        assert!(!json.to_string().contains("echoed"), "{json}");
    }

    #[test]
    fn an_issuers_ban_refusal_blocks_before_any_standing_answer_and_shows_in_the_next_poll() {
        let store = StandingStore::new(None);

        assert!(store.on_refresh_error(&WALLET, &banned_refusal(), NOW));
        let json = parse(&store.on_poll(
            &WALLET,
            Err(ClientError::ServerStatus {
                status: 404,
                body: String::new(),
            }),
            NOW,
        ));

        assert_eq!(
            store
                .ban_in_force(&WALLET, NOW)
                .map(|b| b.lapses_at_unix_secs),
            Some(Some(NOW + 100))
        );
        assert_eq!(json["standing"]["ban"]["in_force"], true);
    }

    fn credit_refusal(lapses_at_unix_secs: Option<u64>) -> ClientError {
        ClientError::Banned {
            reason_code: BanReasonCode::PortForwardingAbuse,
            lapses_at_unix_secs,
        }
    }

    #[test]
    fn a_credit_refusal_blocks_and_answers_the_ban() {
        let store = StandingStore::new(None);

        let ban = store.on_ban_refusal(&WALLET, &credit_refusal(None), NOW);

        assert_eq!(
            ban.map(|b| b.reason),
            Some(BanReasonCode::PortForwardingAbuse)
        );
        assert!(store.ban_in_force(&WALLET, NOW).is_some());
    }

    #[test]
    fn a_credit_refusal_without_a_lapse_answers_the_lapse_the_standing_knows() {
        let store = StandingStore::new(None);
        store.on_poll(&WALLET, Ok(answer(&[], Some(account_ban(NOW + 60)))), NOW);

        let ban = store.on_ban_refusal(&WALLET, &credit_refusal(None), NOW);

        assert_eq!(ban.and_then(|b| b.lapses_at_unix_secs), Some(NOW + 60));
    }

    #[test]
    fn a_credit_failure_that_is_not_a_ban_records_nothing() {
        let store = StandingStore::new(None);
        let spent = ClientError::ServerStatus {
            status: 409,
            body: "voucher already redeemed".to_owned(),
        };

        assert_eq!(store.on_ban_refusal(&WALLET, &spent, NOW), None);
        assert_eq!(store.ban_in_force(&WALLET, NOW), None);
    }

    #[test]
    fn a_refresh_failure_that_is_not_a_ban_changes_nothing() {
        let store = StandingStore::new(None);

        assert!(!store.on_refresh_error(&WALLET, &TokenClientError::BadAttributionKey, NOW));
        assert_eq!(store.ban_in_force(&WALLET, NOW), None);
    }

    #[test]
    fn one_wallets_ban_never_blocks_another() {
        let store = StandingStore::new(None);
        store.on_refresh_error(&WALLET, &banned_refusal(), NOW);

        assert_eq!(store.ban_in_force(&OTHER, NOW), None);
        let json = parse(&store.on_poll(
            &OTHER,
            Err(ClientError::ServerStatus {
                status: 503,
                body: String::new(),
            }),
            NOW,
        ));
        assert_eq!(json["standing"], Value::Null);
    }

    #[test]
    fn a_departed_wallets_refusal_leaves_the_current_wallets_ban_alone() {
        let store = StandingStore::new(None);
        store.on_poll(&OTHER, Ok(answer(&[], None)), NOW);
        store.forget();
        store.on_poll(&WALLET, Ok(answer(&[], Some(account_ban(NOW + 60)))), NOW);

        store.on_refresh_error(&OTHER, &banned_refusal(), NOW);

        assert_eq!(
            store
                .ban_in_force(&WALLET, NOW)
                .and_then(|ban| ban.lapses_at_unix_secs),
            Some(NOW + 60)
        );
    }

    #[test]
    fn a_wallet_that_comes_back_is_heard_again() {
        let store = StandingStore::new(None);
        store.on_poll(&WALLET, Ok(answer(&[], None)), NOW);
        store.forget();
        store.on_poll(&WALLET, Ok(answer(&[], None)), NOW);

        store.on_refresh_error(&WALLET, &banned_refusal(), NOW);

        assert!(store.ban_in_force(&WALLET, NOW).is_some());
    }

    #[test]
    fn an_exit_ban_keeps_the_lapse_the_standing_answered() {
        let store = StandingStore::new(None);
        store.on_poll(&WALLET, Ok(answer(&[], Some(account_ban(NOW + 60)))), NOW);

        let ban = store.on_exit_ban(&WALLET, 1, NOW);

        assert_eq!(ban.lapses_at_unix_secs, Some(NOW + 60));
    }

    #[test]
    fn an_exit_ban_alone_names_the_ban_without_blocking_the_next_dial() {
        let store = StandingStore::new(None);

        let ban = store.on_exit_ban(&WALLET, 0, NOW);

        assert_eq!(ban.reason, BanReasonCode::Other);
        assert_eq!(ban.lapses_at_unix_secs, None);
        assert_eq!(store.ban_in_force(&WALLET, NOW), None);
    }

    #[tokio::test]
    async fn a_ban_learned_during_a_session_ends_the_wait() {
        let store = std::sync::Arc::new(StandingStore::new(None));
        let waiting = {
            let store = store.clone();
            tokio::spawn(async move { store.ban_arrives(WALLET, || NOW).await })
        };
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished(), "no ban yet");

        store.on_refresh_error(&WALLET, &banned_refusal(), NOW);

        let ban = tokio::time::timeout(std::time::Duration::from_secs(5), waiting)
            .await
            .expect("the ban ends the wait")
            .unwrap();
        assert_eq!(ban.lapses_at_unix_secs, Some(NOW + 100));
    }

    #[tokio::test]
    async fn another_wallets_ban_does_not_end_the_wait() {
        let store = StandingStore::new(None);
        store.on_refresh_error(&OTHER, &banned_refusal(), NOW);

        let waited = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            store.ban_arrives(WALLET, || NOW),
        )
        .await;

        assert!(
            waited.is_err(),
            "another wallet's ban never ends this wallet's session"
        );
    }

    #[test]
    fn a_ban_wakes_a_subscriber() {
        let store = StandingStore::new(None);
        let changes = store.subscribe();

        store.on_refresh_error(&WALLET, &banned_refusal(), NOW);

        assert!(changes.has_changed().unwrap());
    }

    #[test]
    fn a_restarted_process_does_not_warn_twice_and_the_file_names_no_case() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LEDGER_FILE);
        StandingStore::new(Some(path.clone())).on_poll(
            &WALLET,
            Ok(answer(&[("PF-2026-0001", 50000)], None)),
            NOW,
        );

        let restarted = StandingStore::new(Some(path.clone()));
        let json =
            parse(&restarted.on_poll(&WALLET, Ok(answer(&[("PF-2026-0001", 50000)], None)), NOW));

        assert_eq!(json["new_strikes"], serde_json::json!([]));
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("PF-2026-0001"), "{on_disk}");
    }

    #[test]
    fn forgetting_the_wallet_clears_the_ban_and_the_ledger_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LEDGER_FILE);
        let store = StandingStore::new(Some(path.clone()));
        store.on_poll(
            &WALLET,
            Ok(answer(&[("PF-1", 50000)], Some(account_ban(NOW + 60)))),
            NOW,
        );

        store.forget();

        assert_eq!(store.ban_in_force(&WALLET, NOW), None);
        let restarted = StandingStore::new(Some(path));
        let json = parse(&restarted.on_poll(&WALLET, Ok(answer(&[("PF-1", 50000)], None)), NOW));
        assert_eq!(
            json["new_strikes"][0]["ordinal"], 1,
            "the strike is announced again"
        );
    }

    #[test]
    fn the_verdict_names_the_block_token_and_the_lapse() {
        let ban = Ban::from_exit_rejection(1);

        let json = parse(&ban_verdict_json(Some(&ban)));

        assert!(
            json["reason"]
                .as_str()
                .unwrap()
                .starts_with("[BANNED_PORT_FORWARDING] ")
        );
        assert_eq!(json["lapses_at_unix_secs"], Value::Null);
        assert_eq!(parse(&ban_verdict_json(None)), serde_json::json!({}));
    }
}
