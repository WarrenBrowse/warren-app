// Warren VPN Android JNI bridge - crate root.
//
// The crate is laid out as two modules:
//
//   - [`wallet`] - pure-rust BIP39 + Ed25519 primitives wrapping
//     `warren-identity`. Always compiled, unit-tested on host.
//   - [`android_jni`] (target_os = "android" only) - the actual JNI exports
//     (`Java_com_warrenbrowse_vpn_jni_WarrenJni_*`). Calls into `wallet`
//     for the mnemonic / signing surface; stubs out the tunnel surface
//     until `warren_tunnel::PacketDevice::from_fd` lands.

pub mod wallet;

// Product/deployment constants (per compiled product environment).
// Host-compiled so the drift gate runs in the routine test scope; public
// (not pub(crate)) because the host build has no internal consumer.
pub mod product;

// Community-forum wallet login (doc 55): the pure request-construction +
// validation + outcome mapping (host-testable); the network POST that consumes
// it is Android-gated in `android_jni`.
pub mod forum;
// Problem-report collection for the in-app bug report (Android arm of the
// shared collector; the metadata/redaction inputs are host-tested).
pub mod report;

// Tunnel session status contract (`redial::SessionStatus`) published to
// Kotlin via `getTunnelStatus`.
pub mod redial;

// Supervised multi-hop session driver (engine supervisor + supervised pumps ->
// the polled `i32`): portable, so the real control loop is host-tested against
// a loopback exit; only the `AndroidTun` it wraps is device-bound.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
mod supervised_session;

// Android bindings for the engine's in-tunnel egress liveness probe: an exit
// that answers the client while forwarding nothing to the internet is invisible
// to every other guard. Portable except the datapath probe, so it is host-tested
// against the real engine scheduler.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
mod egress_probe;

// Android bindings for the engine's migration watchdog: the QUIC path follows a
// Wi-Fi to cellular handover instead of re-handshaking. Portable except for the
// `VpnService.protect` the rebind policy applies, so the wiring is host-tested.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
mod migration;

// Loopback relay+exit speaking the real multi-hop wire, shared by the host
// tests of the two modules above.
#[cfg(test)]
mod loopback_exit;

#[cfg(all(target_os = "android", feature = "tunnel"))]
mod tunnel;

// The IPv4 remap helpers are pure byte manipulation (host-testable); only the
// `PacketDevice` impl inside is Android-gated.
mod remap_tun;

/// Client-side bandwidth ceiling on the tunnel packet device
/// (config `max_rate_bps`). The limiter logic is host-testable; only
/// the wiring into the pump is Android-gated.
mod rate_limited_tun;

// "Port follows the client" suggestion logic (host-testable); the stateful
// wiring that consumes it is Android-gated in `tunnel`.
mod natpmp_follow;

// Multi-hop circuit selection (single-hop collapse vs distinct-entry two-hop):
// pure index arithmetic, host-tested; the datapath that dials the chosen node
// is Android-gated in `tunnel`.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
mod circuit_select;

// v7 anonymous session credentials (Privacy Pass, warren-core doc 64): the per-wallet
// token mint/refresh/stack core is host-tested with a mock transport; the
// provider that feeds the tunnel handshake is Android-gated inside.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
mod token_provider;

// VpnService-protected HTTP transport for the token mint: host-testable
// (injected protector), wired to `VpnService.protect` on Android.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
mod protected_transport;

/// Live network probes recorded into a problem report (the connect host
/// through the protected and the plain path, the resolver, the API), and the
/// clock offset they measure.
#[cfg(any(test, target_os = "android"))]
mod probes;

// The multi-hop directory, fetched once per hour instead of once per dial
// (the daemon's cadence); host-tested against a counting transport.
#[cfg(any(test, target_os = "android"))]
mod directory_cache;

// The one HTTP stack the unsigned and signed API calls share, retired at
// every TUN transition so no pooled connection outlives the network it was
// opened on; host-tested.
#[cfg(any(test, target_os = "android"))]
mod api_transport;

// Both verdicts of the signed update manifest from one read; the pairing is
// host-tested, the fetch is Android-gated in `android_jni`.
#[cfg(any(test, target_os = "android"))]
mod version_check;

#[cfg(target_os = "android")]
mod android_jni;

// The forum flows' JNI exports (login, cancel, report, collection), split
// from the datapath bridge so the forum surface reads as one module.
#[cfg(target_os = "android")]
mod forum_android;

// The Rust log file and the logcat tee behind `initLogger`; the rotation
// and the line format are host-tested.
#[cfg(any(test, target_os = "android"))]
mod rust_log;

// ---------------------------------------------------------------------------
// Security fix tests (host-runnable, no JNI required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod security_tests {
    /// `parking_lot::Mutex` must remain usable on the calling thread
    /// after another thread panics while holding the lock.
    ///
    /// With `std::sync::Mutex` this test would fail because the panic poisons
    /// the mutex and every subsequent `lock()` returns `Err(PoisonError)`.
    /// `parking_lot::Mutex` does not have the poisoning concept, so the lock
    /// is acquired normally and the value is intact.
    #[test]
    fn parking_lot_mutex_survives_panic_in_other_thread() {
        use std::sync::Arc;

        let mutex: Arc<parking_lot::Mutex<Option<u32>>> = Arc::new(parking_lot::Mutex::new(None));

        // Spawn a thread that acquires the lock then panics while holding it.
        let mutex_clone = Arc::clone(&mutex);
        let handle = std::thread::spawn(move || {
            let _guard = mutex_clone.lock();
            panic!("intentional panic while holding the lock");
        });

        // Let the panic propagate to the spawned thread's join handle.
        let _ = handle.join(); // expected to be Err(…)

        // The main thread must still be able to lock the mutex without any
        // error, poison check, or unwrap.  With std::sync::Mutex this line
        // would need `.unwrap_or_else(|e| e.into_inner())` to recover.
        let mut guard = mutex.lock();
        assert!(guard.is_none(), "value should be unchanged after the panic");

        // Verify that the lock is fully operational after the panic.
        *guard = Some(42);
        drop(guard);
        assert_eq!(*mutex.lock(), Some(42));
    }

    /// The mnemonic string must be zeroized (all bytes set to 0)
    /// immediately after key derivation - it must not survive into the
    /// returned value or any heap allocation the caller still holds.
    ///
    /// We cannot observe the internal zeroization directly, but we can verify
    /// two key properties:
    ///   1. `Zeroizing<String>` calls `zeroize()` on drop, which overwrites
    ///      the heap buffer with zeros.
    ///   2. The derived `SigningKey` is independent of (and outlives) the
    ///      mnemonic container.
    #[test]
    fn mnemonic_is_zeroized_after_key_derivation() {
        use zeroize::{Zeroize, Zeroizing};

        let phrase = crate::wallet::generate_mnemonic();

        // Wrap in Zeroizing to mirror what connectTunnel does.
        let mut zeroizing_phrase = Zeroizing::new(phrase.clone());

        // Derive the key from the wrapped phrase.
        let signing_key = crate::wallet::signing_key_from_mnemonic(&zeroizing_phrase)
            .expect("valid mnemonic must produce a signing key");

        // Explicitly zeroize (drop does it too, but this makes the intent clear
        // and lets us assert the bytes are actually cleared).
        zeroizing_phrase.zeroize();
        assert!(
            zeroizing_phrase.as_bytes().iter().all(|&b| b == 0),
            "mnemonic heap buffer must be all-zero after explicit zeroize()"
        );

        // The derived key must still be usable and must match a fresh
        // derivation from the original phrase.
        let reference_key = crate::wallet::signing_key_from_mnemonic(&phrase)
            .expect("reference derivation must succeed");
        assert_eq!(
            signing_key.verifying_key(),
            reference_key.verifying_key(),
            "key derived before zeroize must equal key derived from original phrase"
        );
    }
}
