//! The Warren servers' clock, one for the process, stamped on every signed
//! API request a mobile client makes.
//!
//! The SDK learns the server's clock from the `Date` of a refusal and signs
//! again at it ([`warren_api::clock`]). The Android and iOS layers build a
//! client per call (the wallet secret never lingers), so a clock per client
//! would start from the device clock every time: a phone whose clock drifted
//! past the servers' minute would pay a refusal on every call, and a refused
//! mint of port entitlements or session tokens would not learn from the
//! account screen's reading. One clock for the process makes the first
//! refusal any client meets correct all of them; the offset is the device's,
//! whichever wallet signs. The desktop daemon holds its own, which it also
//! reports (`mullvad-daemon`'s `warren_api_clock`).

use std::sync::{Arc, OnceLock};

use warren_api::clock::ServerClock;
use warren_api::{HttpTransport, WarrenApiClient};

/// The process's one server clock.
fn shared() -> &'static Arc<ServerClock> {
    static CLOCK: OnceLock<Arc<ServerClock>> = OnceLock::new();
    CLOCK.get_or_init(|| Arc::new(ServerClock::new()))
}

/// `client`, stamping its signed requests with the process's one clock.
pub fn attach<T: HttpTransport>(client: WarrenApiClient<T>) -> WarrenApiClient<T> {
    client.with_server_clock(Arc::clone(shared()))
}

/// What a tunnel's dated setup is stamped with (the engine's `SetupClock`):
/// the process's one clock, so an exit never refuses a time the API accepts.
pub fn setup_clock() -> Arc<dyn Fn() -> u64 + Send + Sync> {
    let clock = Arc::clone(shared());
    Arc::new(move || clock.now())
}

#[cfg(test)]
mod tests {
    use warren_api::transport::{HttpRequest, HttpResponse, TransportError};
    use warren_identity::WarrenIdentity;

    use super::*;

    struct Unused;

    impl HttpTransport for Unused {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
            Err(TransportError::Connect(
                "no network in this test".to_owned(),
            ))
        }
    }

    fn client(seed: u8) -> WarrenApiClient<Unused> {
        attach(WarrenApiClient::new(
            "https://api.example.test".to_owned(),
            WarrenIdentity::from_seed(&[seed; 32]),
            Unused,
        ))
    }

    #[test]
    fn a_tunnel_setup_is_dated_on_the_process_clock() {
        // 2023-11-14 22:13:20 UTC.
        const SERVER_NOW: u64 = 1_700_000_000;
        let device_now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        };
        shared().observe_date("Tue, 14 Nov 2023 22:13:20 GMT", device_now());

        let dated = (setup_clock())();

        // Leave the process clock on the device's for the other tests.
        shared().observe_date(&httpdate_now(), device_now());
        assert!(
            (SERVER_NOW..SERVER_NOW + 5).contains(&dated),
            "a setup dated {dated}, the server reads {SERVER_NOW}"
        );
    }

    fn httpdate_now() -> String {
        httpdate::fmt_http_date(std::time::SystemTime::now())
    }

    #[test]
    fn every_client_stamps_with_the_one_clock_whichever_wallet_signs() {
        let first = client(1);
        let other_wallet = client(2);

        assert!(Arc::ptr_eq(
            first.server_clock(),
            other_wallet.server_clock()
        ));
        assert!(Arc::ptr_eq(first.server_clock(), shared()));
    }
}
