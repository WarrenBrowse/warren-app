//! Tunnel session status contract published to the Kotlin layer.
//!
//! The Android tunnel runs a single session with no daemon state machine,
//! so its lifecycle is surfaced to Kotlin as one `i32` polled via
//! `WarrenJni.getTunnelStatus()`. The multi-hop client owns the actual
//! reconnect supervision (its own `SupervisorConfig`); this type only
//! encodes what the Kotlin layer must react to:
//!
//! - `Reconnecting` (3) means "quick blip": a transparent redial is in
//!   flight and expected to land within seconds. The VpnService TUN is
//!   still established and captures all traffic (the dead pump drops it),
//!   so no kill-switch action is needed during this window.
//! - `Disconnected` (0) after a `Reconnecting` window means "network gone":
//!   the Kotlin fail-closed policy takes over (blackhole interface +
//!   connectivity gated retry), exactly as it does for any other session
//!   death.
//! - `Unauthorized` (4) is terminal: retrying cannot recover a lapsed
//!   subscription, so the session is never redialed past it.
//! - `ExitLeaving` (5) ends the session because its exit announced a
//!   maintenance drain: the Kotlin layer fails over to another exit at once,
//!   blackhole first, and does not count it as a drop.
//! - `Banned` (6) is terminal too: the wallet is suspended (warren-core doc
//!   105), learned from an issuer, the standing poll or the exit's CRL, and
//!   `getBanVerdict` says for what and until when.
//! - `DeviceLimit` (7) is terminal as well: the exit refused another session
//!   because the account already uses its maximum number of simultaneous
//!   devices (a wallet-signed session past the cap, or every session token of
//!   the wallet held by its other devices), and a redial meets the same count
//!   until one of them disconnects. Kotlin shows "too many connections", never
//!   the expired-subscription message.
//! - `NoReachableEntry` (8) ends the session because this network routes
//!   none of the entries the circuit may use (every candidate publishes only
//!   address families the network does not carry, forum topic 210). Kotlin
//!   blocks under the "no dialable network" cause and redials once the
//!   network changes, since a redial on the same one meets the same verdict.
//! - `NoSessionToken` (9) ends a tokens-only session that found no token the
//!   exit admits, for a reason other than every token being in use elsewhere
//!   (that one is `DeviceLimit`): the store is empty for the epoch, or the exit
//!   refused every token. Kotlin applies the drop policy under its own cause,
//!   which the screens offer the account's device list on (warren-core doc
//!   115); the next dial takes the tokens the refresh brings.
//! - `DeviceRevoked` (10) is terminal: another device of the account revoked
//!   this one (warren-core doc 115, decision 1), learned by a token or
//!   entitlement refresh or a device registry call. Kotlin takes the tunnel
//!   down, forgets the wallet and shows the revoked screen; coming back takes
//!   the recovery phrase.

/// Tunnel session status reported back to Kotlin via
/// `WarrenJni.getTunnelStatus()`. Encoded as an `i32` rather than an enum
/// to match the existing JNI int contract.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    /// Transparent redial in flight after a session loss. See the module
    /// doc for the Kotlin-side contract.
    Reconnecting = 3,
    /// The exit refused the setup with a policy rejection: the account is
    /// not authorized, i.e. the subscription has lapsed or was revoked.
    /// Distinct from `Disconnected` so the Kotlin layer can surface
    /// "subscription expired" and STOP the reconnect loop (retrying cannot
    /// recover an unauthorized account until it is renewed).
    Unauthorized = 4,
    /// The exit is leaving (see the module doc): fail over now.
    ExitLeaving = 5,
    /// The wallet is banned: see the module doc.
    Banned = 6,
    /// The account already holds its maximum of simultaneous wallet-signed
    /// sessions: see the module doc.
    DeviceLimit = 7,
    /// This network routes none of the circuit's candidate entries: see the
    /// module doc.
    NoReachableEntry = 8,
    /// A tokens-only session found no token the exit admits: see the module
    /// doc.
    NoSessionToken = 9,
    /// Another device of the account revoked this one: see the module doc.
    DeviceRevoked = 10,
}
