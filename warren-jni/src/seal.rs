//! Which seal the session's dial uses, as the connect screen's
//! quantum-resistance chip shows it.
//!
//! Single-homed here so the rule is host-tested; the Android-only session code
//! in `tunnel.rs` publishes it once the exit's key has been negotiated.

/// Whether a dial handed this exit ML-KEM key seals with X-Wing. The engine
/// takes empty key material as absent and seals with X25519 alone, so an empty
/// key must not light the chip either.
#[must_use]
pub(crate) fn seals_post_quantum(exit_mlkem768_pubkey: Option<&[u8]>) -> bool {
    exit_mlkem768_pubkey.is_some_and(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_negotiated_ml_kem_key_is_the_post_quantum_seal() {
        assert!(seals_post_quantum(Some(&[7u8; 1184])));
    }

    #[test]
    fn an_exit_without_a_signed_ml_kem_key_is_sealed_classically() {
        assert!(!seals_post_quantum(None));
    }

    #[test]
    fn empty_key_material_is_the_classical_seal_as_the_engine_reads_it() {
        assert!(!seals_post_quantum(Some(&[])));
    }
}
