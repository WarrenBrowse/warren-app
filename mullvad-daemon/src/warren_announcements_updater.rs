//! Periodic refresher for the server-signed launch announcements
//! (`GET /v1/announcements`), plus the second, wallet-signed call that
//! draws this account's campaign voucher.
//!
//! Fifth instance of the signed-artifact refresh pattern, and it keeps
//! every property the notices updater has. Each one is deliberate:
//!
//! - **a conditional GET on a five minute cadence**, the delay an
//!   operator waits for a publication or a withdrawal to reach a running
//!   client, with a **transport-failure fast retry** so a machine that
//!   just woke does not sit a full interval on a card it can no longer
//!   justify;
//! - **verification against the build-time pinned server key BEFORE any
//!   text is published**, because an announcement is operator-authored
//!   text rendered verbatim on every home screen next to a clickable
//!   link, which is a ready-made phishing surface;
//! - **an anti-rollback refusal on a lower generation**, so a captured
//!   older envelope cannot put a withdrawn card back on every screen;
//! - **re-application of the signed expiry on a 304**, so a set that
//!   stops being displayable while unchanged still clears;
//! - **no on-disk cache**, so a withdrawn announcement cannot come back
//!   on a client that has lost the API;
//! - **never a fall back to an unverified body**, which is the whole
//!   reason the envelope is signed.
//!
//! Filtering (the envelope expiry, the per-announcement expiry and the
//! declared client-version range) happens here through the contract's
//! `active_for`, so the renderer displays what it receives, verbatim.
//!
//! The announcement itself rides a broadcast document byte-identical for
//! every caller, which is what keeps the server from learning who asks
//! about what. A per-account value cannot ride that document, so a
//! `voucher_campaign_id` sends this updater to
//! `GET /v1/campaign/{id}/voucher` over the SIGNED client, and the code
//! is published beside its announcement.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use warren_api::ClientError;
use warren_discovery_core::{
    AnnouncementsError, VerifiedAnnouncements, verify_signed_announcements_any,
};

use crate::warren_artifact_refresh::{
    FETCH_TIMEOUT, FetchResponse, TransportRetryBackoff, conditional_get, now_unix, split_pins,
};
use crate::warren_notices_updater::NoticeLevel;

/// How often the announcements are re-fetched. Same cadence and the same
/// reasoning as the notices: short enough that a withdrawal reaches a
/// running client in minutes, long enough that the poll the API can see
/// stays a poll rather than a stream. The request is a conditional GET,
/// so an unchanged set costs one header round trip.
const CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Fast retry window after a transport failure, so a client that just
/// woke or just regained a network does not sit a full interval showing
/// nothing.
const RETRY_MIN: Duration = Duration::from_secs(20);
/// Ceiling for the fast retry, kept under [`CHECK_INTERVAL`] so a long
/// outage degrades into the normal cadence rather than into silence.
const RETRY_MAX: Duration = Duration::from_secs(240);

/// A boxed, owned future, the shape the daemon already uses to keep an
/// async trait object-safe (`device::account_backend::BoxFut`).
type BoxFut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

/// One announcement as handed to the UI: already filtered for expiry and
/// client version, with an unsafe call to action already withheld and
/// this account's code already drawn, so the renderer displays the list
/// verbatim.
#[derive(Clone, PartialEq, Eq)]
pub struct DisplayAnnouncement {
    /// Server-assigned id, forwarded so the UI can persist a dismissal.
    pub id: String,
    /// One-line title, rendered as plain text (never as markup).
    pub headline: String,
    /// Body text, rendered as plain text (never as markup).
    pub body: String,
    /// Severity, driving the card's indicator colour.
    pub level: NoticeLevel,
    /// Call to action, present only when its URL passed the contract's
    /// own check. Withholding the button and never the announcement is
    /// the deliberate split: the operator's text still reaches the
    /// reader, an unsafe link never becomes clickable.
    pub cta: Option<DisplayCta>,
    /// The voucher code granted to THIS account for the announcement's
    /// campaign, `None` when the announcement carries no offer or when
    /// this account is outside the cohort.
    pub voucher_code: Option<String>,
}

impl std::fmt::Debug for DisplayAnnouncement {
    /// Renders no part of the code. It is a bearer token worth a month of
    /// service, and a `{:?}` on a status snapshot is exactly how one ends
    /// up in a log or a problem report.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DisplayAnnouncement")
            .field("id", &self.id)
            .field("headline", &self.headline)
            .field("body", &self.body)
            .field("level", &self.level)
            .field("cta", &self.cta)
            .field("has_voucher_code", &self.voucher_code.is_some())
            .finish()
    }
}

/// A call to action the renderer may turn into a clickable button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayCta {
    /// Button caption, plain text.
    pub label: String,
    /// Destination opened in the system browser, `https` only.
    pub url: String,
}

/// Where a per-account campaign code comes from.
///
/// A trait rather than the concrete signed client so the publication
/// rules are testable without a network: what is under test is which
/// code the updater publishes and which identity it binds it to, not how
/// the code travels.
pub(crate) trait CampaignVoucherSource: Send + Sync {
    /// Warren SS58 address of the identity the source signs with right
    /// now. Read before every claim, because the daemon hot-swaps it.
    fn address(&self) -> String;

    /// Signed `GET /v1/campaign/{campaign_id}/voucher`. `Ok(None)` is the
    /// server's `404`: outside the cohort, a normal and quiet outcome.
    fn fetch(&self, campaign_id: String) -> BoxFut<Result<Option<String>, ClientError>>;
}

impl CampaignVoucherSource for crate::warren_sdk_client::SharedWarrenApiClient {
    fn address(&self) -> String {
        crate::warren_sdk_client::SharedWarrenApiClient::address(self)
    }

    fn fetch(&self, campaign_id: String) -> BoxFut<Result<Option<String>, ClientError>> {
        let client = self.clone();
        Box::pin(async move { client.campaign_voucher(&campaign_id).await })
    }
}

/// The codes this session holds, and the identity they were drawn for.
///
/// Held in memory only, and cleared whole on an identity change: a code
/// belongs to the wallet that asked for it, and the wallet that replaces
/// it has its own or none at all. Re-fetching is always safe because the
/// server call is a pure lookup that never mints and never assigns.
///
/// Deliberately not zeroized. The code is rendered on the user's own
/// screen and travels through the status snapshot and the gRPC stream to
/// get there, so wiping this one copy would be theatre; what actually
/// bounds the exposure is that it never reaches a log, an error or a
/// problem report.
struct SessionCodes {
    address: String,
    /// Campaign id to the code drawn for it. `None` records "outside the
    /// cohort", so a 404 is not re-asked every five minutes.
    by_campaign: HashMap<String, Option<String>>,
}

/// Build the HTTP client. Same posture as the notices fetcher: the API
/// host resolves from the daemon's address cache
/// ([`crate::warren_api_dns`]), and when an API IP is pinned and the URL
/// targets the default host, resolve without DNS and omit SNI.
fn build_http_client(api_url: &str) -> reqwest::Client {
    use mullvad_api_constants::{API_HOST_DEFAULT, API_PINNED_IP, API_PORT_DEFAULT};

    let mut builder =
        crate::warren_api_dns::with_api_resolver(reqwest::Client::builder().timeout(FETCH_TIMEOUT));
    if let Some(ip) = API_PINNED_IP
        && reqwest::Url::parse(api_url)
            .ok()
            .and_then(|u| {
                u.host_str()
                    .map(|h| h.eq_ignore_ascii_case(API_HOST_DEFAULT))
            })
            .unwrap_or(false)
    {
        builder = builder
            .resolve(
                API_HOST_DEFAULT,
                std::net::SocketAddr::new(ip, API_PORT_DEFAULT),
            )
            .tls_sni(false);
    }
    builder
        .build()
        .expect("reqwest client build failed: invalid TLS backend configuration")
}

/// Warren SS58 address of the logged-out sentinel seed.
fn sentinel_address() -> String {
    warren_identity::WarrenIdentity::from_seed(&crate::warren_sdk_client::sentinel_seed()).address()
}

/// The periodic announcements task.
pub struct WarrenAnnouncementsUpdater {
    api_url: String,
    /// Pinned server pubkey set (comma-separated for rotation). Empty
    /// means TOFU, which the daemon never configures in a real build.
    pinned_server_pubkey: Option<String>,
    /// This app's own version, used to honour an announcement's declared
    /// version range. `None` withholds every range-restricted card.
    client_version: Option<String>,
    etag: Option<String>,
    /// Highest generation ever accepted (anti-rollback high-water mark).
    highest_generation: u64,
    /// Last verified envelope, kept in memory only. Re-published on a
    /// `304` so its own expiry rules keep being applied while the set is
    /// unchanged.
    last: Option<VerifiedAnnouncements>,
    /// The signed client the campaign lookup goes through. `None` leaves
    /// every announcement published without a code, which is what a build
    /// with no wallet must do.
    voucher_source: Option<Arc<dyn CampaignVoucherSource>>,
    /// Address that must never be used to ask for a code. See
    /// [`Self::claim`].
    sentinel_address: String,
    codes: Option<SessionCodes>,
    http: reqwest::Client,
    on_update: Box<dyn Fn(Vec<DisplayAnnouncement>) + Send>,
}

impl WarrenAnnouncementsUpdater {
    /// Spawn the updater. `on_update` receives the announcements to
    /// display, including the empty vector when the last one is withdrawn
    /// or lapses, so the UI clears its card from the same signal that
    /// raised it.
    pub fn spawn(
        api_url: String,
        pinned_server_pubkey: Option<String>,
        client_version: Option<String>,
        voucher_source: Option<Arc<dyn CampaignVoucherSource>>,
        on_update: impl Fn(Vec<DisplayAnnouncement>) + Send + 'static,
    ) {
        let http = build_http_client(&api_url);
        let updater = Self {
            api_url,
            pinned_server_pubkey,
            client_version,
            etag: None,
            highest_generation: 0,
            last: None,
            voucher_source,
            sentinel_address: sentinel_address(),
            codes: None,
            http,
            on_update: Box::new(on_update),
        };
        tokio::spawn(updater.run());
    }

    async fn run(mut self) {
        let mut retry = TransportRetryBackoff::new(RETRY_MIN, RETRY_MAX);
        loop {
            match self.refresh_once().await {
                RefreshOutcome::Ok | RefreshOutcome::Rejected => retry.clear(),
                RefreshOutcome::TransportFailure => {
                    retry.on_transport_failure();
                }
            }
            let delay = retry.delay().unwrap_or(CHECK_INTERVAL);
            tokio::time::sleep(delay).await;
        }
    }

    /// One conditional fetch plus verification and publication.
    async fn refresh_once(&mut self) -> RefreshOutcome {
        let url = format!("{}/v1/announcements", self.api_url.trim_end_matches('/'));
        let response = match conditional_get(&self.http, &url, self.etag.clone()).await {
            Ok(r) => r,
            Err(e) => {
                log::debug!("Warren announcements fetch failed: {e}");
                return RefreshOutcome::TransportFailure;
            }
        };
        match response {
            // A 304 says the *set* has not changed, so the last publish
            // still stands. The envelope the client verified earlier is
            // what bounds how long it may be displayed, and it is
            // re-published below so the UI re-applies its expiry.
            FetchResponse::NotModified => {
                self.republish_last().await;
                RefreshOutcome::Ok
            }
            FetchResponse::Status(status) => {
                log::debug!("Warren announcements endpoint returned {status}");
                RefreshOutcome::Rejected
            }
            FetchResponse::Body { body, etag } => {
                let pins = split_pins(self.pinned_server_pubkey.as_deref());
                match verify_signed_announcements_any(&body, &pins) {
                    Ok(verified) => {
                        if is_rollback(verified.generation, self.highest_generation) {
                            // Rollback: an older envelope, replayed. It
                            // could hold an announcement the operator
                            // withdrew.
                            log::warn!(
                                "Warren announcements generation went backwards ({} < {}); \
                                 ignoring",
                                verified.generation,
                                self.highest_generation
                            );
                            return RefreshOutcome::Rejected;
                        }
                        self.highest_generation = verified.generation;
                        self.etag = etag;
                        self.publish(&verified).await;
                        self.last = Some(verified);
                        RefreshOutcome::Ok
                    }
                    Err(e) => {
                        // Never fall back to the body: an unverified
                        // envelope is exactly the phishing case the
                        // signature exists to stop.
                        log::error!("Warren announcements verification failed: {}", describe(&e));
                        RefreshOutcome::Rejected
                    }
                }
            }
        }
    }

    /// Re-applies the last verified envelope's own expiry rules. Called on
    /// `304`, so a set that stops being displayable while unchanged (a
    /// per-announcement TTL elapses, or the envelope lapses) still clears.
    async fn republish_last(&mut self) {
        if let Some(verified) = self.last.clone() {
            self.publish(&verified).await;
        }
    }

    async fn publish(&mut self, verified: &VerifiedAnnouncements) {
        let active = verified.active_for(now_unix(), self.client_version.as_deref());
        let mut display = Vec::with_capacity(active.len());
        for announcement in active {
            let voucher_code = match &announcement.voucher_campaign_id {
                Some(campaign_id) => self.claim(campaign_id).await,
                None => None,
            };
            display.push(DisplayAnnouncement {
                id: announcement.id.clone(),
                headline: announcement.headline.clone(),
                body: announcement.body.clone(),
                level: announcement.level.into(),
                // The contract's own check, not a second copy of its
                // rules: what is safe to render as a link is one
                // decision, and it lives beside the wire format.
                cta: announcement.displayable_cta().map(|safe| DisplayCta {
                    label: safe.label.clone(),
                    url: safe.url.clone(),
                }),
                voucher_code,
            });
        }
        (self.on_update)(display);
    }

    /// This account's code for `campaign_id`, or `None` when there is
    /// none to draw.
    ///
    /// The identity is read BEFORE anything else, and the whole cache is
    /// dropped when it changed. That ordering is the same one
    /// `account_backend` uses before pulling a paid voucher: acting under
    /// a desynced identity binds the wrong wallet, and here it would show
    /// one account the offer drawn for another.
    ///
    /// A `404` is cached as "outside the cohort", which is a normal, quiet
    /// outcome rather than an error to retry forever. Any other failure is
    /// not cached at all, so a transient outage never tells a cohort
    /// member they were never eligible.
    async fn claim(&mut self, campaign_id: &str) -> Option<String> {
        let source = Arc::clone(self.voucher_source.as_ref()?);
        let address = source.address();
        if address == self.sentinel_address {
            // The logged-out sentinel is a fixed, publicly known identity.
            // Asking for a code as that wallet would draw an offer nobody
            // owns and hand it to whoever logs in next.
            return None;
        }
        if self
            .codes
            .as_ref()
            .is_none_or(|held| held.address != address)
        {
            self.codes = Some(SessionCodes {
                address: address.clone(),
                by_campaign: HashMap::new(),
            });
        }
        if let Some(held) = self
            .codes
            .as_ref()
            .and_then(|held| held.by_campaign.get(campaign_id))
        {
            return held.clone();
        }
        let fetched = match source.fetch(campaign_id.to_owned()).await {
            Ok(code) => code,
            Err(e) => {
                // No code and no cache entry: the next cycle asks again.
                // The error is rendered by status only; its body may echo
                // identity material, and the code must never reach a log.
                log::debug!("Warren campaign voucher lookup failed: {e}");
                return None;
            }
        };
        // The daemon can hot-swap the wallet while the request is in
        // flight (create, restore, logout). A code drawn for the previous
        // identity is not this one's, so it is dropped rather than cached.
        if source.address() != address {
            return None;
        }
        if let Some(held) = self.codes.as_mut() {
            held.by_campaign
                .insert(campaign_id.to_owned(), fetched.clone());
        }
        fetched
    }
}

/// Outcome of one refresh, used only to drive the retry policy.
enum RefreshOutcome {
    /// Fetched (or confirmed unchanged) and published.
    Ok,
    /// Reached the server but did not accept the answer. A fast retry
    /// would not help, so this falls back to the normal cadence.
    Rejected,
    /// Never reached the server.
    TransportFailure,
}

/// True when an incoming envelope is older than the highest one already
/// trusted, i.e. a replay that could resurrect a withdrawn announcement.
/// Equal generations are accepted: the server re-signs the same set
/// periodically, and refusing that would freeze a client on a stale
/// envelope that is about to expire.
fn is_rollback(incoming: u64, highest: u64) -> bool {
    incoming < highest
}

/// Redacted one-line reason for a verification failure. The message must
/// never carry the pubkey or the body.
fn describe(e: &AnnouncementsError) -> &'static str {
    match e {
        AnnouncementsError::Json(_) => "malformed envelope",
        AnnouncementsError::UnsupportedVersion { .. } => "unsupported envelope version",
        AnnouncementsError::ServerPubkeyMismatch { .. } => "server pubkey is not the pinned one",
        AnnouncementsError::InvalidHex | AnnouncementsError::PubkeyNotOnCurve => {
            "malformed key or signature"
        }
        AnnouncementsError::BadSignature => "bad signature",
        AnnouncementsError::InputTooLarge => "envelope too large",
        _ => "verification failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use warren_discovery_core::{Announcement, AnnouncementCta, NoticeLevel as WireLevel};

    use super::*;

    fn announcement(id: &str, headline: &str) -> Announcement {
        Announcement {
            id: id.to_owned(),
            headline: headline.to_owned(),
            body: "body".to_owned(),
            level: WireLevel::Info,
            cta: None,
            voucher_campaign_id: None,
            min_client_version: None,
            max_client_version: None,
            expires_at: None,
        }
    }

    /// A voucher source that answers from a script and records what it
    /// was asked, with a hot-swappable address so an identity change is
    /// expressible without a daemon.
    struct FakeSource {
        address: Mutex<String>,
        /// Address to swap to the moment a fetch starts, standing in for
        /// a create / restore / logout that races the request.
        swap_on_fetch: Mutex<Option<String>>,
        answer: Mutex<Result<Option<String>, ()>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeSource {
        fn granting(code: &str) -> Arc<Self> {
            Arc::new(Self {
                address: Mutex::new("wbAAAA".to_owned()),
                swap_on_fetch: Mutex::new(None),
                answer: Mutex::new(Ok(Some(code.to_owned()))),
                calls: Mutex::new(Vec::new()),
            })
        }

        fn outside_the_cohort() -> Arc<Self> {
            Arc::new(Self {
                address: Mutex::new("wbAAAA".to_owned()),
                swap_on_fetch: Mutex::new(None),
                answer: Mutex::new(Ok(None)),
                calls: Mutex::new(Vec::new()),
            })
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl CampaignVoucherSource for FakeSource {
        fn address(&self) -> String {
            self.address.lock().expect("address lock").clone()
        }

        fn fetch(&self, campaign_id: String) -> BoxFut<Result<Option<String>, ClientError>> {
            self.calls.lock().expect("calls lock").push(campaign_id);
            if let Some(next) = self.swap_on_fetch.lock().expect("swap lock").take() {
                *self.address.lock().expect("address lock") = next;
            }
            let answer = self
                .answer
                .lock()
                .expect("answer lock")
                .clone()
                .map_err(|()| ClientError::AllHostsBlocked);
            Box::pin(async move { answer })
        }
    }

    /// An updater whose `on_update` records what it published, with no
    /// HTTP client involvement.
    fn recording_updater(
        client_version: Option<&str>,
        voucher_source: Option<Arc<dyn CampaignVoucherSource>>,
    ) -> (
        WarrenAnnouncementsUpdater,
        Arc<Mutex<Vec<Vec<DisplayAnnouncement>>>>,
    ) {
        let published = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&published);
        let updater = WarrenAnnouncementsUpdater {
            api_url: "https://example.invalid".to_owned(),
            pinned_server_pubkey: None,
            client_version: client_version.map(str::to_owned),
            etag: None,
            highest_generation: 0,
            last: None,
            voucher_source,
            sentinel_address: sentinel_address(),
            codes: None,
            http: reqwest::Client::new(),
            on_update: Box::new(move |announcements| {
                sink.lock().expect("test sink lock").push(announcements);
            }),
        };
        (updater, published)
    }

    /// An envelope whose expiry is expressed relative to the real clock,
    /// because `publish` reads the real clock (that is the behaviour under
    /// test: a lapsed envelope must publish nothing).
    fn envelope(announcements: Vec<Announcement>, valid_for: i64) -> VerifiedAnnouncements {
        let now = now_unix();
        VerifiedAnnouncements {
            announcements,
            generation: 1,
            signed_at: now,
            expires_at: now.saturating_add_signed(valid_for),
        }
    }

    fn only_batch(
        published: &Arc<Mutex<Vec<Vec<DisplayAnnouncement>>>>,
    ) -> Vec<DisplayAnnouncement> {
        let batches = published.lock().expect("sink");
        assert_eq!(batches.len(), 1, "exactly one publication expected");
        batches[0].clone()
    }

    #[tokio::test]
    async fn publishes_the_operator_card_with_its_level_and_safe_cta() {
        let (mut updater, published) = recording_updater(None, None);
        let mut card = announcement("a1", "Warren production is open");
        card.body = "One free month on production.".to_owned();
        card.level = WireLevel::Warning;
        card.cta = Some(AnnouncementCta {
            label: "Download".to_owned(),
            url: "https://warren.ro/download".to_owned(),
        });

        updater.publish(&envelope(vec![card], 3_600)).await;

        assert_eq!(
            only_batch(&published),
            vec![DisplayAnnouncement {
                id: "a1".to_owned(),
                headline: "Warren production is open".to_owned(),
                body: "One free month on production.".to_owned(),
                level: NoticeLevel::Warning,
                cta: Some(DisplayCta {
                    label: "Download".to_owned(),
                    url: "https://warren.ro/download".to_owned(),
                }),
                voucher_code: None,
            }],
            "the UI must receive the card verbatim, with its severity and its button"
        );
    }

    #[tokio::test]
    async fn withholds_the_button_of_an_unsafe_cta_but_still_shows_the_card() {
        let (mut updater, published) = recording_updater(None, None);
        let mut card = announcement("a1", "Warren production is open");
        card.cta = Some(AnnouncementCta {
            label: "Download".to_owned(),
            // Userinfo hides the real host: the contract refuses it.
            url: "https://warren.ro@evil.example/dl".to_owned(),
        });

        updater.publish(&envelope(vec![card], 3_600)).await;

        let batch = only_batch(&published);
        assert_eq!(
            batch.len(),
            1,
            "the operator's text must still reach the reader"
        );
        assert_eq!(
            batch[0].cta, None,
            "an unsafe link must never become a clickable control"
        );
    }

    #[tokio::test]
    async fn publishes_an_empty_batch_once_the_envelope_has_lapsed() {
        // Anti-freeze: this is what clears the card on a client whose
        // refresh is being blocked, without any renderer-side timer.
        let (mut updater, published) = recording_updater(None, None);

        updater
            .publish(&envelope(vec![announcement("a1", "stale")], -1))
            .await;

        assert_eq!(
            only_batch(&published),
            Vec::new(),
            "an expired envelope must publish the empty set, not nothing at all"
        );
    }

    #[tokio::test]
    async fn withholds_a_version_targeted_announcement_from_an_out_of_range_client() {
        let (mut updater, published) = recording_updater(Some("1.9.0"), None);
        let mut targeted = announcement("a1", "for 1.11 and up");
        targeted.min_client_version = Some("1.11.0".to_owned());

        updater.publish(&envelope(vec![targeted], 3_600)).await;

        assert_eq!(
            only_batch(&published),
            Vec::new(),
            "the daemon applies the version range, so the UI never has to"
        );
    }

    #[test]
    fn a_lower_generation_is_a_rollback_and_an_equal_one_is_not() {
        assert!(
            is_rollback(4, 5),
            "an older envelope could carry a withdrawn announcement"
        );
        assert!(
            !is_rollback(5, 5),
            "the server re-signs the same set periodically; refusing that would \
             strand the client on an envelope that is about to expire"
        );
        assert!(!is_rollback(6, 5));
    }

    #[tokio::test]
    async fn an_announcement_with_a_campaign_publishes_this_account_code() {
        let source = FakeSource::granting("ABCDEFGHJKMNPQRS");
        let (mut updater, published) = recording_updater(
            None,
            Some(Arc::clone(&source) as Arc<dyn CampaignVoucherSource>),
        );
        let mut offer = announcement("a1", "Warren production is open");
        offer.voucher_campaign_id = Some("prod-launch".to_owned());
        let plain = announcement("a2", "Scheduled maintenance");

        updater.publish(&envelope(vec![offer, plain], 3_600)).await;

        let batch = only_batch(&published);
        assert_eq!(
            batch[0].voucher_code.as_deref(),
            Some("ABCDEFGHJKMNPQRS"),
            "the campaign id IS the offer, so its presence must draw the code"
        );
        assert_eq!(
            batch[1].voucher_code, None,
            "an announcement without a campaign must not carry a code"
        );
        assert_eq!(
            source.calls(),
            vec!["prod-launch".to_owned()],
            "only the announcement carrying an offer may reach the signed endpoint"
        );
    }

    #[tokio::test]
    async fn a_404_on_the_claim_publishes_the_announcement_without_a_code() {
        // Outside the cohort is the ordinary case for an account created
        // after publication: the card is still the operator's message.
        let source = FakeSource::outside_the_cohort();
        let (mut updater, published) = recording_updater(
            None,
            Some(Arc::clone(&source) as Arc<dyn CampaignVoucherSource>),
        );
        let mut offer = announcement("a1", "Warren production is open");
        offer.voucher_campaign_id = Some("prod-launch".to_owned());

        updater.publish(&envelope(vec![offer], 3_600)).await;

        let batch = only_batch(&published);
        assert_eq!(batch.len(), 1, "a 404 must not drop the announcement");
        assert_eq!(batch[0].voucher_code, None);
    }

    #[tokio::test]
    async fn the_code_is_held_for_the_session_and_redrawn_on_an_identity_change() {
        let source = FakeSource::granting("ABCDEFGHJKMNPQRS");
        let (mut updater, _published) = recording_updater(
            None,
            Some(Arc::clone(&source) as Arc<dyn CampaignVoucherSource>),
        );
        let mut offer = announcement("a1", "Warren production is open");
        offer.voucher_campaign_id = Some("prod-launch".to_owned());
        let envelope = envelope(vec![offer], 3_600);

        updater.publish(&envelope).await;
        updater.publish(&envelope).await;
        assert_eq!(
            source.calls().len(),
            1,
            "the five-minute cadence must not re-ask on every cycle"
        );

        *source.address.lock().expect("address lock") = "wbBBBB".to_owned();
        *source.answer.lock().expect("answer lock") = Ok(Some("ZZZZZZZZZZZZZZZZ".to_owned()));
        updater.publish(&envelope).await;

        assert_eq!(
            source.calls().len(),
            2,
            "a new wallet has its own code, or none: the held one is not its"
        );
    }

    #[tokio::test]
    async fn a_code_drawn_for_a_wallet_replaced_mid_request_is_discarded() {
        let source = FakeSource::granting("ABCDEFGHJKMNPQRS");
        *source.swap_on_fetch.lock().expect("swap lock") = Some("wbBBBB".to_owned());
        let (mut updater, published) = recording_updater(
            None,
            Some(Arc::clone(&source) as Arc<dyn CampaignVoucherSource>),
        );
        let mut offer = announcement("a1", "Warren production is open");
        offer.voucher_campaign_id = Some("prod-launch".to_owned());

        updater.publish(&envelope(vec![offer], 3_600)).await;

        assert_eq!(
            only_batch(&published)[0].voucher_code,
            None,
            "a logout or a restore that lands mid-request must not show the \
             previous wallet's offer to the new one"
        );
    }

    #[tokio::test]
    async fn the_logged_out_sentinel_never_asks_for_a_code() {
        let source = FakeSource::granting("ABCDEFGHJKMNPQRS");
        *source.address.lock().expect("address lock") = sentinel_address();
        let (mut updater, published) = recording_updater(
            None,
            Some(Arc::clone(&source) as Arc<dyn CampaignVoucherSource>),
        );
        let mut offer = announcement("a1", "Warren production is open");
        offer.voucher_campaign_id = Some("prod-launch".to_owned());

        updater.publish(&envelope(vec![offer], 3_600)).await;

        assert!(
            source.calls().is_empty(),
            "the sentinel is publicly known: an offer drawn under it belongs to nobody"
        );
        assert_eq!(only_batch(&published)[0].voucher_code, None);
    }

    #[test]
    fn the_debug_rendering_never_carries_the_code() {
        // A `{:?}` on a status snapshot is exactly how a bearer token ends
        // up in a log or a problem report.
        let card = DisplayAnnouncement {
            id: "a1".to_owned(),
            headline: "Warren production is open".to_owned(),
            body: "body".to_owned(),
            level: NoticeLevel::Info,
            cta: None,
            voucher_code: Some("ABCDEFGHJKMNPQRS".to_owned()),
        };

        let rendered = format!("{card:?}");

        assert!(
            !rendered.contains("ABCDEFGHJKMNPQRS"),
            "the code must never be renderable: {rendered}"
        );
        assert!(
            rendered.contains("has_voucher_code: true"),
            "whether a code is held is still worth seeing: {rendered}"
        );
    }
}
