// Warren VPN Android JNI bridge - crate root.
//
// The crate is laid out as two modules:
//
//   - [`wallet`] - pure-rust BIP39 + Ed25519 primitives wrapping
//     `warren-identity`. Always compiled, unit-tested on host.
//   - [`android_jni`] (target_os = "android" only) - the actual JNI exports
//     (`Java_com_warrenbrowse_vpn_jni_WarrenJni_*`). Calls into `wallet`
//     for the mnemonic / signing surface; stubs out the tunnel surface
//     until D.4 lands `warren_tunnel::PacketDevice::from_fd`.

pub mod wallet;

#[cfg(all(target_os = "android", feature = "tunnel"))]
mod tunnel;

#[cfg(target_os = "android")]
mod android_jni;

// ---------------------------------------------------------------------------
// Security fix tests (host-runnable, no JNI required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod security_tests {
    /// Fix M-3: `parking_lot::Mutex` must remain usable on the calling thread
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

        // Verify that the lock is fully operational after the incident.
        *guard = Some(42);
        drop(guard);
        assert_eq!(*mutex.lock(), Some(42));
    }

    /// Fix L-2: the mnemonic string must be zeroized (all bytes set to 0)
    /// immediately after key derivation — it must not survive into the
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
