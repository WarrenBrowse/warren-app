//! The errors of the Warren tunnel backend, and the causes they carry.

use super::*;

/// Errors specific to the Warren tunnel backend.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The first session could not be established.
    #[error("Warren handshake failed")]
    Handshake(#[source] HandshakeFailure),

    /// Could not open the TUN device via
    /// [`talpid_tunnel::tun_provider::TunProvider`].
    #[error("Warren tun setup failed")]
    TunSetup(#[source] TunSetupFailure),

    /// The routes that send traffic into the tunnel could not be installed.
    /// The session is up but carries nothing, so the tunnel must never report
    /// itself connected: the state machine blocks with this cause instead.
    #[error("Warren tunnel routes not installed")]
    RouteSetup(#[source] RouteSetupFailure),

    /// Transient backend error that the state machine should retry:
    /// TUN I/O error, temporary network disruption, or a session
    /// closed by the peer without an explicit rejection code. The
    /// tunnel data plane was interrupted but the configuration and
    /// credentials are still valid - a reconnect is expected to
    /// succeed.
    #[error("Warren tunnel transient backend error (recoverable)")]
    BackendTransient(#[source] TransientCause),

    /// The exit refused the session for a policy reason that clears on its
    /// own: the wallet is absent from the exit's allowlist, which is exactly
    /// what a just-redeemed subscription looks like until the exit's next
    /// allowlist poll.
    ///
    /// Recoverable like [`Self::BackendTransient`], so the state machine
    /// retries and the kill-switch stays up. It is a variant of its own only
    /// because it is the single retriable failure that can persist forever
    /// (a wallet with no subscription is refused every time), so the client
    /// asks the account API whether there is a subscription to sync and
    /// surfaces the refusal when there is none, instead of dialing in silence.
    /// The message carries the `[EXPIRED_ACCOUNT]` auth-failed token so that,
    /// when it is surfaced, the app shows its existing localized copy instead
    /// of a generic failure.
    #[error("Warren exit refused the session (recoverable): {0}")]
    SessionRejected(Refusal),

    /// Fatal backend error that the state machine must NOT retry
    /// automatically: configuration mismatch, authentication failure, or
    /// PKI/TLS setup failure. Retrying immediately would produce the same
    /// outcome and only waste network bandwidth.
    ///
    /// NOTE: a multi-hop `not-allowlisted` rejection is deliberately NOT
    /// fatal: it self-heals once the exit's allowlist refresh picks up a
    /// freshly-redeemed subscription, so [`reject_error`] maps it to
    /// [`Self::SessionRejected`] (recoverable). Do not "promote" it to
    /// fatal: that would strand a just-subscribed user in an
    /// uncancelable-until-manual-reconnect state.
    #[error("Warren tunnel fatal backend error: {0}")]
    BackendFatal(FatalCause),

    /// No multi-hop circuit was supplied. The Warren fleet is multi-hop
    /// only, so the daemon's circuit selection always populates
    /// [`WarrenTunnelParameters::multi_hop`] with a 1-hop or 2-hop
    /// circuit. A `None` reaching the tunnel backend means circuit
    /// assembly failed upstream; fail closed rather than open an
    /// unprotected direct path.
    #[error("Warren tunnel requires a multi-hop circuit but none was supplied")]
    NoCircuit,
}

impl Error {
    /// Whether the error is worth retrying from the state-machine
    /// side.
    ///
    /// - [`Error::Handshake`]: `true` - transient network glitch is the common cause; the next
    ///   attempt will likely succeed.
    /// - [`Error::TunSetup`]: `false` - privilege / kernel module / name collision: retry will not
    ///   help without operator action.
    /// - [`Error::BackendTransient`]: `true` - TUN I/O error or peer-initiated session close; the
    ///   configuration is still valid and a fresh connect should succeed.
    /// - [`Error::SessionRejected`]: `true` - the exit's allowlist has not caught up with the
    ///   subscription yet; the next poll admits the session.
    /// - [`Error::BackendFatal`]: `false` - auth failure or a definitive ban; retrying immediately
    ///   would waste bandwidth and produce the same outcome.
    /// - [`Error::NoCircuit`]: `false` - fail-closed invariant guard; no reconnect can conjure a
    ///   circuit the daemon did not supply.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Handshake(_) | Error::BackendTransient(_) | Error::SessionRejected(_)
        )
    }

    /// The user-facing reason of a self-healing exit refusal, `None` for every
    /// other error.
    ///
    /// Lets the state machine bound its retry loop on a refusal specifically,
    /// without sniffing error strings, and reuse the same text once it decides
    /// the refusal has outlived the exit's sync window.
    #[must_use]
    pub fn session_rejection(&self) -> Option<&Refusal> {
        match self {
            Error::SessionRejected(refusal) => Some(refusal),
            _ => None,
        }
    }
}

/// Why the first session could not be established.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum HandshakeFailure {
    /// The daemon closed the tunnel while the multi-hop start was waiting.
    #[error("multi-hop start aborted by the daemon close signal")]
    ClosedBeforeStart,
    /// The daemon closed the tunnel during the first dial.
    #[error("multi-hop first dial aborted by the daemon close signal")]
    ClosedDuringFirstDial,
    /// The supervisor produced no session within the bound.
    #[error("multi-hop supervisor did not produce an initial session within {0:?}")]
    NoInitialSessionWithin(std::time::Duration),
    /// No entry relay is reachable over the address families this network has.
    #[error("no entry relay reachable on this network's address families")]
    Unroutable,
    /// The quantum-resistant setting requires the post-quantum seal and the
    /// exit's signed descriptor offers no ML-KEM key.
    #[error(
        "the exit offers no signed post-quantum key, and the quantum-resistant tunnel setting \
         requires the post-quantum seal"
    )]
    PostQuantumUnavailable,
    /// The supervisor stopped before its first session, for another reason.
    #[error("multi-hop supervisor stopped before an initial session")]
    SupervisorStopped,
    /// The exit negotiated a DAITA configuration this client cannot run.
    #[error("negotiated DAITA spec unusable")]
    DaitaSpec(#[source] warrenguard_daita::DaitaError),
    /// The physical interface the carrier socket is pinned to is unknown.
    #[error("cannot resolve the physical interface for the carrier socket bypass")]
    NoPhysicalInterface(#[source] anyhow::Error),
}

/// Why the TUN device could not be opened.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum TunSetupFailure {
    /// A thread panicked while holding the TUN provider.
    #[error("tun_provider mutex poisoned")]
    ProviderPoisoned,
    /// The provider refused to open the device.
    #[error("the TUN provider could not open the device")]
    Open(#[source] talpid_tunnel::tun_provider::Error),
}

/// Why the routes into the tunnel could not be installed.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum RouteSetupFailure {
    /// The relay is reached over IPv6: the macOS split keys its cleanup on
    /// the relay's IPv4 host address.
    #[error("the relay is reached over IPv6, which the default route split does not support")]
    RelayOverIpv6,
    /// The split of the default route failed.
    #[error("default route split")]
    DefaultRouteSplit(#[source] anyhow::Error),
    /// The host route that keeps the relay off the tunnel failed.
    #[error("relay bypass route")]
    RelayBypass(#[source] talpid_routing::Error),
    /// The routes into the tunnel failed.
    #[error("tunnel routes")]
    TunnelRoutes(#[source] talpid_routing::Error),
}

/// Why a session ended in a way a reconnect is expected to fix.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransientCause {
    /// The exit's inner address pool is exhausted.
    #[error("exit rejected the session ({0}); its address pool is exhausted")]
    AddressPoolExhausted(RejectionReason),
    /// The exit saw this setup before: the path replayed it.
    #[error("exit refused a replayed setup ({0}); redialling")]
    SetupReplayed(RejectionReason),
    /// One exit refused this device's time, which may be that exit's clock.
    #[error("exit refused the setup's time ({0}); trying again")]
    ClockRefusedOnce(RejectionReason),
    /// Sessions establish but no return traffic flows.
    #[error(
        "tunnel datapath dead: sessions establish but no return traffic flows \
         (consecutive zero-downlink redials)"
    )]
    DatapathDead,
    /// A guard of the live session (pumps, liveness, migration, drain, egress
    /// probe, carrier guard) ended it, with the reason it logged.
    #[error("pump terminated abnormally: {0}")]
    SessionEnded(String),
}

/// Why the exit, or this device's clock, ends every session for good. The
/// [`Display`](std::fmt::Display) of each opens on the auth-failed token the
/// app localizes; the state machine hands it on as the blocked reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FatalCause {
    /// The wallet is on the fleet-wide ban list for port-forwarding abuse.
    BannedForPortForwarding(RejectionReason),
    /// The wallet is on the fleet-wide ban list for another reason.
    Banned(RejectionReason),
    /// The account already uses its maximum number of simultaneous devices.
    DeviceLimit(RejectionReason),
    /// Two different exits refused this device's time.
    ClockSkew(RejectionReason),
}

impl std::fmt::Display for FatalCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FatalCause::BannedForPortForwarding(reason) => write!(
                f,
                "[BANNED_PORT_FORWARDING] exit rejected the session ({reason}); access \
                 suspended for port-forwarding abuse"
            ),
            FatalCause::Banned(reason) => write!(
                f,
                "[BANNED] exit rejected the session ({reason}); access suspended"
            ),
            FatalCause::DeviceLimit(reason) => write!(
                f,
                "[TOO_MANY_CONNECTIONS] exit refused the session ({reason}); the account \
                 already uses its maximum number of simultaneous devices"
            ),
            FatalCause::ClockSkew(reason) => write!(
                f,
                "{CLOCK_SKEW_REASON} two exits refused this device's time ({reason}); set the \
                 date and time to automatic"
            ),
        }
    }
}

/// A self-healing exit refusal, as the state machine judges it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The reason surfaced when the subscription is not active, opening on
    /// the `[EXPIRED_ACCOUNT]` auth-failed token the app localizes.
    pub reason: String,
    /// The reason surfaced instead when the account API says the
    /// subscription is active, for a refusal an active subscription does not
    /// clear on its own: the exit refused every session token without saying
    /// why, as an exit too old to name the device limit does. `None` when an
    /// active subscription means the refusal clears once the exit syncs it.
    pub if_active: Option<String>,
    /// Keep dialing for as long as the account API cannot say whether the
    /// subscription is active, rather than surfacing [`Self::reason`] after
    /// the patience: a session no token admits, whose tokens the same API
    /// mints, so an unanswered query says nothing of an expiry.
    pub retry_while_unverified: bool,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}
