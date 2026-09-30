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
