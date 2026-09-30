//! The Warren servers' clock, as every wallet-signed request of the daemon is
//! stamped with it.
//!
//! The API and the forum broker refuse a signature whose timestamp is further
//! than a minute from their own clock. A machine whose clock drifted past that
//! is refused on every signed call, and nothing it can act on says why: a
//! Windows client 91 s fast lost its port forwarding, its session tokens and
//! its account screen for days (forum topic 219), while the forum requests of
//! the same daemon went through because only they were corrected.
//!
//! The SDK now learns the server's clock from the `Date` of a refusal and
//! signs again at it ([`warren_api::clock`]). This module makes that reading
//! count for the whole daemon: ONE [`ServerClock`] for the process, attached
//! to every `WarrenApiClient` the daemon builds and read by the forum signer,
//! so the first refusal any of them meets corrects all of them. It is one per
//! process rather than one per wallet because the offset is this machine's
//! clock against the servers', the same whichever wallet signs.
//!
//! The forum broker's `Date` is still read before a forum request is signed,
//! as the daemon did before the SDK carried the correction, and it is recorded
//! on the same clock.
//!
//! **No-log policy**: only the offset (a duration) is ever logged, never the
//! request, the key or anything a report carries.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use warren_api::clock::{MAX_FORWARD_CORRECTION_SECS, ServerClock, applicable_offset};
use warren_api::{HttpTransport, WarrenApiClient};

/// Connect and total budget of the broker clock read. Deliberately under the
/// deadline of the report upload that follows: a broker that cannot be
/// reached at all must cost the reporter a few seconds, not the whole
/// attempt, and the signature still goes out on the last reading.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(8);

/// The server clock of this process, and whether it was ever read.
pub(crate) struct ApiClock {
    clock: Arc<ServerClock>,
    /// Set when this module records a reading. The SDK records a refusal's
    /// `Date` on [`Self::clock`] without saying so; a non-zero offset tells
    /// that reading apart from none.
    read: AtomicBool,
}

impl ApiClock {
    fn new() -> Self {
        Self {
            clock: Arc::new(ServerClock::new()),
            read: AtomicBool::new(false),
        }
    }

    /// `client`, stamping its signed requests with this clock.
    pub(crate) fn attach<T: HttpTransport>(
        &self,
        client: WarrenApiClient<T>,
    ) -> WarrenApiClient<T> {
        client.with_server_clock(Arc::clone(&self.clock))
    }

    /// The clock itself, to tell the clients stamping with it.
    #[cfg(test)]
    pub(crate) fn server_clock(&self) -> &Arc<ServerClock> {
        &self.clock
    }

    /// Records the offset the `Date` of an answer received at `device_now`
    /// shows. Returns it, or `None` when the header does not parse.
    fn observe(&self, date: &str, device_now: u64) -> Option<i64> {
        let offset = self.clock.observe_date(date, device_now)?;
        self.read.store(true, Ordering::Relaxed);
        Some(offset)
    }

    /// The last measured offset (server minus device, seconds, positive when
    /// this machine is behind), or `None` while no answer has been read.
    pub(crate) fn measured_offset(&self) -> Option<i64> {
        let offset = self.clock.offset_secs();
        (self.read.load(Ordering::Relaxed) || offset != 0).then_some(offset)
    }

    /// The timestamp a forum request is signed at when the device clock reads
    /// `device_now` and the broker answered with `broker_date`. A broker
    /// answer without a usable `Date` leaves the last reading standing, which
    /// may be the API's.
    fn forum_stamp(&self, broker_date: Option<&str>, device_now: u64) -> u64 {
        if let Some(date) = broker_date {
            self.observe(date, device_now);
        }
        self.clock.stamp(device_now)
    }
}

/// The daemon's one server clock.
pub(crate) fn shared() -> &'static ApiClock {
    static CLOCK: OnceLock<ApiClock> = OnceLock::new();
    CLOCK.get_or_init(ApiClock::new)
}

/// The endpoint the broker's clock is read from: its health answer is
/// unauthenticated, carries nothing about the caller, and is served by the
/// same host under the same certificate as the signed POST that follows.
fn health_url(host: &str) -> String {
    format!("https://{host}/healthz")
}

/// Reads the broker's clock once and returns the timestamp to sign a forum
/// request at. Every failure (no client, no answer, no `Date`) signs on the
/// last reading of the shared clock, the device clock when there is none.
pub(crate) async fn forum_signing_timestamp(host: &str) -> u64 {
    let device_now = crate::warren_artifact_refresh::now_unix();
    let date = read_broker_date(host).await;
    let clock = shared();
    let stamp = clock.forum_stamp(date.as_deref(), device_now);
    let offset = clock.clock.offset_secs();
    match applicable_offset(offset) {
        0 if offset > MAX_FORWARD_CORRECTION_SECS => log::info!(
            "Server clock: the servers answered {offset} s ahead, too far to stamp at; signing on this machine's clock"
        ),
        0 => (),
        applied => log::info!(
            "Server clock: this machine is {applied} s off the servers, correcting the stamp"
        ),
    }
    stamp
}

async fn read_broker_date(host: &str) -> Option<String> {
    let client = crate::warren_tls::configure(
        reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT),
        true,
    )
    .build()
    .ok()?;
    match client.get(health_url(host)).send().await {
        Ok(response) => response
            .headers()
            .get(reqwest::header::DATE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        Err(_) => {
            // The cause is not logged: a transport error can quote the
            // request, and the class is all this line needs.
            log::debug!("Server clock: the broker's clock could not be read, signing anyway");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use warren_api::transport::{HttpRequest, HttpResponse, TransportError};
    use warren_identity::WarrenIdentity;

    use super::*;

    /// 2023-11-14 22:13:20 UTC.
    const SERVER_NOW: u64 = 1_700_000_000;
    const SERVER_DATE: &str = "Tue, 14 Nov 2023 22:13:20 GMT";

    #[test]
    fn a_machine_behind_the_broker_signs_at_the_brokers_clock() {
        // Five minutes slow is inside nobody's notice and outside the
        // broker's window, so every report was refused with a notice about
        // the clock and no way to act on it.
        let clock = ApiClock::new();
        assert_eq!(
            clock.forum_stamp(Some(SERVER_DATE), SERVER_NOW - 300),
            SERVER_NOW
        );
    }

    #[test]
    fn a_broker_answer_without_a_date_signs_on_the_last_reading() {
        let fresh = ApiClock::new();
        assert_eq!(
            fresh.forum_stamp(None, SERVER_NOW + 91),
            SERVER_NOW + 91,
            "nothing read yet: the device clock, as before the correction"
        );

        // The API refused this machine 91 s fast a moment ago: the forum
        // request is stamped with what that refusal showed.
        let read = ApiClock::new();
        read.clock.observe_date(SERVER_DATE, SERVER_NOW + 91);
        assert_eq!(read.forum_stamp(None, SERVER_NOW + 91), SERVER_NOW);
    }

    #[test]
    fn a_broker_answering_from_the_future_does_not_move_the_stamp() {
        // A `Date` a year ahead would mint signatures the broker could hold
        // and present when they become current.
        let clock = ApiClock::new();
        let a_year_before = SERVER_NOW - 365 * 24 * 60 * 60;
        assert_eq!(
            clock.forum_stamp(Some(SERVER_DATE), a_year_before),
            a_year_before
        );
    }

    #[test]
    fn nothing_is_measured_until_an_answer_is_read() {
        let clock = ApiClock::new();
        assert_eq!(clock.measured_offset(), None);

        clock.forum_stamp(Some(SERVER_DATE), SERVER_NOW);
        assert_eq!(
            clock.measured_offset(),
            Some(0),
            "a right clock, measured, reads 0 rather than not measured"
        );
    }

    #[test]
    fn a_reading_the_sdk_made_on_its_own_counts_as_measured() {
        let clock = ApiClock::new();
        clock.clock.observe_date(SERVER_DATE, SERVER_NOW + 91);
        assert_eq!(clock.measured_offset(), Some(-91));
    }

    #[test]
    fn the_broker_is_read_on_its_own_health_endpoint() {
        assert_eq!(
            health_url("connect.warrenbrowse.com"),
            "https://connect.warrenbrowse.com/healthz"
        );
    }

    fn http_date(unix_secs: u64) -> String {
        chrono::DateTime::from_timestamp(i64::try_from(unix_secs).unwrap(), 0)
            .unwrap()
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string()
    }

    fn now() -> u64 {
        crate::warren_artifact_refresh::now_unix()
    }

    /// The API of forum topic 219: its clock is 91 s behind this machine's,
    /// and it refuses a stamp outside its minute with `clock_skew`.
    #[derive(Clone, Default)]
    struct SkewedApi {
        requests: Arc<AtomicUsize>,
        refused: Arc<Mutex<u32>>,
    }

    impl SkewedApi {
        const BEHIND_SECS: u64 = 91;
    }

    impl HttpTransport for SkewedApi {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            let server_now = now() - Self::BEHIND_SECS;
            let stamp: u64 = request
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("x-warren-timestamp"))
                .and_then(|(_, value)| value.parse().ok())
                .expect("a signed request carries its timestamp");
            if stamp.abs_diff(server_now) > 60 {
                *self.refused.lock().unwrap() += 1;
                return Ok(
                    HttpResponse::new(401, br#"{"error":"clock_skew"}"#.to_vec())
                        .with_date(http_date(server_now)),
                );
            }
            Ok(HttpResponse::new(204, Vec::new()).with_date(http_date(server_now)))
        }
    }

    /// The daemon's transport, recording the status and body of every
    /// answer.
    #[derive(Clone)]
    struct Recording {
        inner: crate::warren_api_transport::WarrenApiTransport,
        answers: Arc<Mutex<Vec<(u16, String)>>>,
    }

    impl HttpTransport for Recording {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            let response = self.inner.execute(request).await?;
            self.answers.lock().unwrap().push((
                response.status,
                String::from_utf8_lossy(&response.body).into_owned(),
            ));
            Ok(response)
        }
    }

    /// Against the real API (`WARREN_LIVE_API`, e.g.
    /// `https://api.beta.warrenbrowse.com`), with a throwaway wallet: the
    /// shared clock starts out stamping 91 s ahead of the servers, as the
    /// machine of forum topic 219 did. The API refuses the stamp for the
    /// clock, its `Date` reaches the SDK through the daemon's transport, and
    /// the same request signed again at the corrected stamp is no longer
    /// refused for the clock. Neither this machine's clock nor its network
    /// settings are touched. Run with `--ignored`.
    #[tokio::test]
    #[ignore = "reaches the live API named by WARREN_LIVE_API"]
    async fn a_live_api_refusal_for_the_clock_corrects_the_stamp() {
        let Ok(api) = std::env::var("WARREN_LIVE_API") else {
            panic!("set WARREN_LIVE_API to the API base, e.g. https://api.beta.warrenbrowse.com");
        };
        let clock = ApiClock::new();
        let device_now = now();
        clock.observe(&http_date(device_now + 91), device_now);
        let answers = Arc::new(Mutex::new(Vec::new()));
        let transport = Recording {
            inner: crate::warren_api_transport::WarrenApiTransport::new(),
            answers: Arc::clone(&answers),
        };
        let seed: [u8; 32] = rand::random();
        let client = clock.attach(WarrenApiClient::new(
            api,
            WarrenIdentity::from_seed(&seed),
            transport,
        ));

        let result = client.subscription().await;

        let answers = answers.lock().unwrap().clone();
        let statuses: Vec<u16> = answers.iter().map(|(status, _)| *status).collect();
        eprintln!(
            "live answers: {answers:?}; offset now {:?}; result: {}",
            clock.measured_offset(),
            match &result {
                Ok(_) => "a subscription".to_owned(),
                Err(error) => format!("{error}"),
            }
        );
        assert_eq!(statuses.first(), Some(&401), "the skewed stamp is refused");
        assert_eq!(statuses.len(), 2, "one refusal, one corrected retry");
        assert!(
            !matches!(
                result,
                Err(warren_api::ClientError::ClockSkew { .. })
                    | Err(warren_api::ClientError::ServerStatus { status: 401, .. })
            ),
            "the corrected stamp must not be refused for the clock"
        );
        assert!(
            clock
                .measured_offset()
                .is_some_and(|offset| offset.unsigned_abs() <= 5),
            "the refusal's Date shows this machine's own clock"
        );
    }

    fn client(clock: &ApiClock, api: &SkewedApi) -> WarrenApiClient<SkewedApi> {
        clock.attach(WarrenApiClient::new(
            "https://api.example.test".to_owned(),
            WarrenIdentity::from_seed(&[7u8; 32]),
            api.clone(),
        ))
    }

    /// The daemon rebuilds a `WarrenApiClient` per call. Each one used to
    /// start from the device clock, so a machine 91 s fast would pay a
    /// refusal on every call; on the shared clock only the first one does.
    #[tokio::test]
    async fn a_refusal_read_by_one_client_corrects_the_next_one() {
        let clock = ApiClock::new();
        let api = SkewedApi::default();

        client(&clock, &api)
            .delete_account()
            .await
            .expect("the first client signs again at the refusal's clock");
        assert_eq!(api.requests.load(Ordering::SeqCst), 2);

        client(&clock, &api)
            .delete_account()
            .await
            .expect("the next client stamps right the first time");
        assert_eq!(api.requests.load(Ordering::SeqCst), 3);
        assert_eq!(*api.refused.lock().unwrap(), 1);
        assert!(
            clock
                .measured_offset()
                .is_some_and(|offset| offset.abs_diff(-91) <= 1),
            "the refusal's offset is what a report shows"
        );
    }
}
