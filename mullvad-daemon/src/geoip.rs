use std::time::Duration;

use futures::join;
use mullvad_api::rest::{Error, RequestServiceHandle};
use mullvad_types::location::{AmIMullvad, GeoIpLocation, LocationEventData};
use talpid_core::mpsc::Sender;
use talpid_future::retry::{ExponentialBackoff, Jittered, retry_future};
use talpid_types::ErrorExt;

use crate::{DaemonEventSender, InternalDaemonEvent};

/// Resolve the effective conncheck host.
///
/// The Warren tunnel is the only mode, so the Mullvad conncheck endpoint
/// (`am.i.mullvad.net`) is never contacted: leaking Warren users to Mullvad
/// infrastructure would reveal their existence. `WARREN_CONNCHECK_HOST` may
/// override the host for development; when unset, `None` is returned so the
/// caller skips the check entirely.
fn resolve_conncheck_host() -> Option<String> {
    match std::env::var("WARREN_CONNCHECK_HOST").ok() {
        Some(host) if !host.trim().is_empty() => {
            log::debug!("Warren conncheck: using WARREN_CONNCHECK_HOST={host}");
            Some(host)
        }
        // No override → skip conncheck to avoid Mullvad infrastructure contact.
        _ => {
            log::debug!("Warren conncheck disabled (set WARREN_CONNCHECK_HOST to override)");
            None
        }
    }
}

const LOCATION_RETRY_STRATEGY: Jittered<ExponentialBackoff> =
    Jittered::jitter(ExponentialBackoff::new(Duration::from_secs(1), 4));

/// Handler for request to the conncheck endpoint, manages in-flight request and validity of
/// responses.
pub(crate) struct GeoIpHandler {
    /// Unique ID for each request. The ID attached to
    /// [`InternalDaemonEvent::LocationEvent`] is used by
    /// [`crate::Daemon::handle_location_event`] to determine if the location
    /// belongs to the current tunnel state.
    pub request_id: usize,
    rest_service: RequestServiceHandle,
    location_sender: DaemonEventSender,
    /// When `Some`, the conncheck uses this host; when `None` the check is
    /// disabled (no `WARREN_CONNCHECK_HOST` override).
    conncheck_host: Option<String>,
}

impl GeoIpHandler {
    pub fn new(rest_service: RequestServiceHandle, location_sender: DaemonEventSender) -> Self {
        let conncheck_host = resolve_conncheck_host();
        Self {
            request_id: 0,
            rest_service,
            location_sender,
            conncheck_host,
        }
    }

    /// Send a location request to the configured conncheck endpoint.
    ///
    /// Without a `WARREN_CONNCHECK_HOST` override, this is a no-op: the
    /// Mullvad conncheck (`am.i.mullvad.net`) must not be contacted to avoid
    /// revealing Warren users to Mullvad infrastructure.
    pub fn send_geo_location_request(&mut self, use_ipv6: bool) {
        let Some(host) = self.conncheck_host.clone() else {
            log::debug!("GeoIpHandler: conncheck skipped (no WARREN_CONNCHECK_HOST)");
            return;
        };

        // Increment request ID
        self.request_id = self.request_id.wrapping_add(1);

        self.abort_current_request();

        let request_id = self.request_id;
        let rest_service = self.rest_service.clone();
        let location_sender = self.location_sender.clone();
        tokio::spawn(async move {
            if let Ok(location) = get_geo_location_with_retry(use_ipv6, rest_service, &host).await {
                let _ =
                    location_sender.send(InternalDaemonEvent::LocationEvent(LocationEventData {
                        request_id,
                        location,
                    }));
            }
        });
    }

    /// Abort any ongoing conncheck call.
    pub fn abort_current_request(&mut self) {
        self.rest_service.reset();
    }
}

/// Fetch the current `GeoIpLocation` from the given `host`. Handles retries on network errors.
async fn get_geo_location_with_retry(
    use_ipv6: bool,
    rest_service: RequestServiceHandle,
    host: &str,
) -> Result<GeoIpLocation, Error> {
    log::debug!("Fetching GeoIpLocation from {host}");
    let host = host.to_owned();
    retry_future(
        move || send_location_request(rest_service.clone(), use_ipv6, host.clone()),
        move |result| match result {
            Err(error) => error.is_network_error(),
            _ => false,
        },
        LOCATION_RETRY_STRATEGY,
    )
    .await
}

async fn send_location_request(
    request_sender: RequestServiceHandle,
    use_ipv6: bool,
    host: String,
) -> Result<GeoIpLocation, Error> {
    let v4_sender = request_sender.clone();
    let host_v4 = host.clone();
    let v4_future = async move {
        let uri_v4 = format!("https://ipv4.{host_v4}/json");
        let location = send_location_request_internal(&uri_v4, v4_sender).await?;
        Ok::<GeoIpLocation, Error>(GeoIpLocation::from(location))
    };
    let v6_sender = request_sender.clone();
    let v6_future = async move {
        if use_ipv6 {
            let uri_v6 = format!("https://ipv6.{host}/json");
            let location = send_location_request_internal(&uri_v6, v6_sender).await;
            Some(location.map(GeoIpLocation::from))
        } else {
            None
        }
    };

    let (v4_result, v6_result) = join!(v4_future, v6_future);

    match (v4_result, v6_result) {
        (Ok(mut v4), Some(Ok(v6))) => {
            v4.ipv6 = v6.ipv6;
            v4.mullvad_exit_ip = v4.mullvad_exit_ip && v6.mullvad_exit_ip;
            Ok(v4)
        }
        (Ok(v4), None) => Ok(v4),
        (Ok(v4), Some(Err(e))) => {
            log_network_error(e, "IPv6");
            Ok(v4)
        }
        (Err(e), Some(Ok(v6))) => {
            log_network_error(e, "IPv4");
            Ok(v6)
        }
        (Err(e_v4), _) => Err(e_v4),
    }
}

async fn send_location_request_internal(
    uri: &str,
    service: RequestServiceHandle,
) -> Result<AmIMullvad, Error> {
    let future_service = service.clone();
    let request = mullvad_api::rest::get(uri)?;
    future_service.request(request).await?.deserialize().await
}

fn log_network_error(err: Error, version: &'static str) {
    if !err.is_offline() {
        let err_message = &format!("Unable to fetch {version} GeoIP location");
        log::debug!("{}", err.display_chain_with_msg(err_message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C-1 regression: with no `WARREN_CONNCHECK_HOST` set,
    /// `resolve_conncheck_host` must return `None` so we never contact
    /// `am.i.mullvad.net` (which would reveal Warren user existence to Mullvad).
    ///
    /// Note: this test is only reliable if `WARREN_CONNCHECK_HOST` is not set
    /// in the test environment. We detect and skip gracefully if it is.
    #[test]
    fn without_env_override_disables_conncheck() {
        if std::env::var("WARREN_CONNCHECK_HOST").is_ok() {
            // Cannot assert None when the override is present; skip silently.
            return;
        }
        let host = resolve_conncheck_host();
        assert!(
            host.is_none(),
            "expected None without WARREN_CONNCHECK_HOST, got {host:?}"
        );
    }
}
