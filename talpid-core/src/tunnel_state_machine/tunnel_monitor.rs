//! Abstracts over an active VPN tunnel.
//!
//! Warren fork: the only tunnel backend is the QUIC-based
//! [`WarrenTunnelMonitor`]. The historical WireGuard backend has been
//! removed.
use std::path;

use talpid_tunnel::TunnelArgs;
use talpid_types::tunnel::ErrorStateCause;
use talpid_warren_tunnel::{WarrenTunnelMonitor, WarrenTunnelParameters};

const TUNNEL_LOG_FILENAME: &str = "tunnel.log";

/// Results from operations in the tunnel module.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in the [`TunnelMonitor`].
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Tunnel can't have IPv6 enabled because the system has disabled IPv6 support.
    #[error("Can't enable IPv6 on tunnel interface because IPv6 is disabled")]
    EnableIpv6,

    /// Running on an operating system which is not supported yet.
    #[error("Tunnel type not supported on this operating system")]
    UnsupportedPlatform,

    /// Failed to rotate tunnel log file
    #[error("Failed to rotate tunnel log file")]
    RotateLogError(#[from] crate::logging::RotateLogError),

    /// There was an error listening for events from the Warren tunnel.
    #[error("Failed while listening for events from the Warren tunnel")]
    WarrenTunnelMonitoring(#[from] talpid_warren_tunnel::Error),
}

impl From<Error> for ErrorStateCause {
    fn from(error: Error) -> ErrorStateCause {
        match error {
            Error::EnableIpv6 => ErrorStateCause::Ipv6Unavailable,
            // A Warren *business* rejection — the exit refused the
            // handshake for a reason the user can act on (no active
            // subscription, device limit reached, account suspended, …).
            // It is non-retryable and carries a user-facing message;
            // surface it as `AuthFailed(reason)` so the app shows the
            // precise cause instead of a generic "failed to start" or the
            // misleading "no matching relay" the retry loop used to
            // produce. This is the single funnel for every exit-side
            // rejection code mapped in `warren_tunnel` →
            // `talpid_warren_tunnel::map_handshake_error`.
            Error::WarrenTunnelMonitoring(talpid_warren_tunnel::Error::BackendFatal(reason)) => {
                ErrorStateCause::AuthFailed(Some(reason))
            }
            #[cfg(target_os = "windows")]
            error => match error.get_tunnel_device_error() {
                Some(error) => ErrorStateCause::CreateTunnelDevice {
                    os_error: error.raw_os_error(),
                },
                None => ErrorStateCause::StartTunnelError,
            },
            #[cfg(not(target_os = "windows"))]
            _ => ErrorStateCause::StartTunnelError,
        }
    }
}

impl Error {
    /// Return whether retrying the operation that caused this error is likely to succeed.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Error::WarrenTunnelMonitoring(error) => error.is_recoverable(),
            _ => false,
        }
    }

    /// Get the inner tunnel device error, if there is one
    #[cfg(target_os = "windows")]
    pub fn get_tunnel_device_error(&self) -> Option<&std::io::Error> {
        // The Warren backend does not (yet) expose an equivalent
        // `tunnel_device_error`.
        None
    }
}

/// Abstraction for monitoring a VPN tunnel.
pub struct TunnelMonitor {
    monitor: WarrenTunnelMonitor,
}

impl TunnelMonitor {
    /// Starts a tunnel via the Warren (QUIC) backend.
    ///
    /// # Errors
    ///
    /// [`Error::WarrenTunnelMonitoring`] if the backend fails to
    /// initialize.
    pub fn start_warren_tunnel(
        params: &WarrenTunnelParameters,
        log_dir: &Option<path::PathBuf>,
        args: TunnelArgs<'_>,
    ) -> Result<Self> {
        let log_file = Self::prepare_tunnel_log_file(log_dir.as_ref())?;
        let monitor = WarrenTunnelMonitor::start(params, args, log_file.as_deref())?;
        Ok(TunnelMonitor { monitor })
    }

    #[cfg(not(target_os = "windows"))]
    fn prepare_tunnel_log_file(log_dir: Option<&path::PathBuf>) -> Result<Option<path::PathBuf>> {
        Ok(log_dir.map(|dir| dir.join(TUNNEL_LOG_FILENAME)))
    }

    #[cfg(target_os = "windows")]
    fn prepare_tunnel_log_file(log_dir: Option<&path::PathBuf>) -> Result<Option<path::PathBuf>> {
        if let Some(log_dir) = log_dir {
            let tunnel_log = log_dir.join(TUNNEL_LOG_FILENAME);
            crate::logging::rotate_log(&tunnel_log)?;
            Ok(Some(tunnel_log))
        } else {
            Ok(None)
        }
    }

    /// Consumes the monitor and blocks until the tunnel exits or there is an error.
    pub fn wait(self) -> Result<()> {
        self.monitor.wait().map_err(Error::from)
    }
}
