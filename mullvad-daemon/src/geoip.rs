use std::time::Duration;

use futures::join;
use mullvad_api::rest::{Error, RequestServiceHandle};
use mullvad_types::location::{AmIMullvad, GeoIpLocation, LocationEventData};
use std::sync::LazyLock;
use talpid_core::mpsc::Sender;
use talpid_future::retry::{ExponentialBackoff, Jittered, retry_future};
use talpid_types::ErrorExt;

use crate::{DaemonEventSender, InternalDaemonEvent};

// Define the Mullvad connection checking api endpoint.
//
// In a development build the host name for the connection checking endpoint can
// be overridden by defining the env variable `MULLVAD_CONNCHECK_HOST`.
//
// If `MULLVAD_CONNCHECK_HOST` is set when running `mullvad-daemon` in a
// production build, a warning will be logged and the env variable *won't* have
// any effect on the api call. The default host name `am.i.mullvad.net` will
// always be used in release mode.
//
// Warren fork: when Warren tunnel mode is active the Mullvad conncheck endpoint
// is intentionally skipped (see `GeoIpHandler::send_geo_location_request`).
// Leaking Warren users to Mullvad infrastructure via am.i.mullvad.net would
// reveal their existence. Set `WARREN_CONNCHECK_HOST` to override the host for
// development; leave it unset to disable the check entirely in production.
static MULLVAD_CONNCHECK_HOST: LazyLock<String> = LazyLock::new(|| {
    const DEFAULT_CONNCHECK_HOST: &str = "am.i.mullvad.net";
    let conncheck_host_var = std::env::var("MULLVAD_CONNCHECK_HOST").ok();
    let host = if cfg!(feature = "api-override") {
        match conncheck_host_var.as_deref() {
            Some(host) => {
                log::debug!("Overriding conncheck endpoint. Using {}", &host);
                host
            }
            None => DEFAULT_CONNCHECK_HOST,
        }
    } else {
        if conncheck_host_var.is_some() {
            log::warn!("These variables are ignored in production builds: MULLVAD_CONNCHECK_HOST");
        };
        DEFAULT_CONNCHECK_HOST
    };
    host.to_string()
});

/// Resolve the effective conncheck host for the current mode.
///
/// - In Warren mode, `WARREN_CONNCHECK_HOST` env var is checked first.
///   If set, that host is used; if unset, `None` is returned so the
///   caller can skip the check entirely (avoids leaking to Mullvad infra).
/// - Outside Warren mode, the standard `MULLVAD_CONNCHECK_HOST` logic applies
///   and this function always returns `Some`.
fn resolve_conncheck_host(warren_mode: bool) -> Option<String> {
    if warren_mode {
        match std::env::var("WARREN_CONNCHECK_HOST").ok() {
            Some(host) if !host.trim().is_empty() => {
                log::debug!("Warren conncheck: using WARREN_CONNCHECK_HOST={host}");
                Some(host)
            }
            // No override → skip conncheck to avoid Mullvad infrastructure contact.
            _ => {
                log::debug!(
                    "Warren mode: conncheck disabled (set WARREN_CONNCHECK_HOST to override)"
                );
                None
            }
        }
    } else {
        Some(MULLVAD_CONNCHECK_HOST.clone())
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
    /// disabled (Warren mode with no `WARREN_CONNCHECK_HOST` override).
    conncheck_host: Option<String>,
}

impl GeoIpHandler {
    pub fn new(
        rest_service: RequestServiceHandle,
        location_sender: DaemonEventSender,
        warren_mode: bool,
    ) -> Self {
        let conncheck_host = resolve_conncheck_host(warren_mode);
        Self {
            request_id: 0,
            rest_service,
            location_sender,
            conncheck_host,
        }
    }

    /// Send a location request to the configured conncheck endpoint.
    ///
    /// In Warren mode without a `WARREN_CONNCHECK_HOST` override, this is a
    /// no-op: the Mullvad conncheck (`am.i.mullvad.net`) must not be contacted
    /// to avoid revealing Warren users to Mullvad infrastructure.
    pub fn send_geo_location_request(&mut self, use_ipv6: bool) {
        let Some(host) = self.conncheck_host.clone() else {
            log::debug!("GeoIpHandler: conncheck skipped (warren_mode, no WARREN_CONNCHECK_HOST)");
            return;
        };

        // Increment request ID
        self.request_id = self.request_id.wrapping_add(1);

        self.abort_current_request();

        let request_id = self.request_id;
        let rest_service = self.rest_service.clone();
        let location_sender = self.location_sender.clone();
        tokio::spawn(async move {
            if let Ok(location) = get_geo_location_with_retry(use_ipv6, rest_service, &host).await
            {
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

    /// C-1 regression: when Warren mode is active and no `WARREN_CONNCHECK_HOST`
    /// is set, `resolve_conncheck_host` must return `None` so we never contact
    /// `am.i.mullvad.net` (which would reveal Warren user existence to Mullvad).
    ///
    /// Note: this test is only reliable if `WARREN_CONNCHECK_HOST` is not set
    /// in the test environment. We detect and skip gracefully if it is.
    #[test]
    fn warren_mode_without_env_override_disables_conncheck() {
        if std::env::var("WARREN_CONNCHECK_HOST").is_ok() {
            // Cannot assert None when the override is present; skip silently.
            return;
        }
        let host = resolve_conncheck_host(true);
        assert!(
            host.is_none(),
            "expected None in Warren mode without WARREN_CONNCHECK_HOST, got {host:?}"
        );
    }

    /// C-1: outside Warren mode, `resolve_conncheck_host` always returns Some
    /// (the standard Mullvad host path).
    #[test]
    fn non_warren_mode_always_returns_some_host() {
        let host = resolve_conncheck_host(false);
        assert!(
            host.is_some(),
            "expected Some host outside Warren mode, got None"
        );
        let host = host.unwrap();
        assert!(
            !host.is_empty(),
            "conncheck host must not be an empty string"
        );
    }

    /// C-1: the non-Warren conncheck host must not be a Warren domain —
    /// ensures we do not accidentally misconfigure the non-Warren path.
    #[test]
    fn non_warren_conncheck_host_is_mullvad_domain() {
        let host = resolve_conncheck_host(false).unwrap();
        assert!(
            host.contains("mullvad"),
            "non-Warren conncheck host should reference mullvad.net, got: {host:?}"
        );
    }
}
