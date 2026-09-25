//! Which strikes this device already warned about.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use warren_api::AccountStrike;

/// On-disk format version. A ledger of another version is refused rather
/// than guessed at: the worst a refusal costs is one repeated warning.
const LEDGER_VERSION: u32 = 1;

/// Domain separator of the digest a strike is remembered by.
const DIGEST_DOMAIN: &[u8] = b"warren/strike-announced/v1\0";

/// Why a persisted ledger could not be read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LedgerError {
    /// The bytes are not a ledger document.
    #[error("the strike ledger is not valid JSON")]
    Json(#[source] serde_json::Error),
    /// The document was written by another format version.
    #[error("the strike ledger has unsupported version {0}")]
    Version(u32),
    /// An entry is not a SHA-256 hex digest.
    #[error("the strike ledger holds an entry that is not a digest")]
    Digest,
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    version: u32,
    announced: Vec<String>,
}

/// The strikes already announced, remembered by a digest of their case
/// reference so the file names no case.
///
/// Only the strikes still inside the window are kept: the standing answer is
/// the whole live set, so a digest it no longer lists belongs to a strike
/// that expired or was voided, and forgetting it bounds the file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StrikeLedger {
    announced: BTreeSet<String>,
}

impl StrikeLedger {
    /// An empty ledger: every live strike is new to it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a ledger written by [`Self::to_json`].
    ///
    /// # Errors
    /// [`LedgerError`] when the document is malformed, of another version,
    /// or holds an entry that is not a digest.
    pub fn from_json(json: &str) -> Result<Self, LedgerError> {
        let persisted: Persisted = serde_json::from_str(json).map_err(LedgerError::Json)?;
        if persisted.version != LEDGER_VERSION {
            return Err(LedgerError::Version(persisted.version));
        }
        if !persisted.announced.iter().all(|d| is_digest(d)) {
            return Err(LedgerError::Digest);
        }
        Ok(Self {
            announced: persisted.announced.into_iter().collect(),
        })
    }

    /// The ledger as the document [`Self::from_json`] reads.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&Persisted {
            version: LEDGER_VERSION,
            announced: self.announced.iter().cloned().collect(),
        })
        .unwrap_or_else(|_| unreachable!("a version and a list of strings always serialize"))
    }

    /// The strikes of `live` not announced before, in `live`'s order, and
    /// records the whole live set as announced.
    pub fn take_new(&mut self, live: &[AccountStrike]) -> Vec<AccountStrike> {
        let fresh = live
            .iter()
            .filter(|strike| !self.announced.contains(&digest(&strike.case_reference)))
            .cloned()
            .collect();
        self.announced = live
            .iter()
            .map(|strike| digest(&strike.case_reference))
            .collect();
        fresh
    }
}

fn digest(case_reference: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update(case_reference.as_bytes());
    hex::encode(hasher.finalize())
}

fn is_digest(entry: &str) -> bool {
    entry.len() == 64 && entry.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strike;

    fn references(strikes: &[AccountStrike]) -> Vec<&str> {
        strikes.iter().map(|s| s.case_reference.as_str()).collect()
    }

    #[test]
    fn every_live_strike_is_new_to_an_empty_ledger() {
        let live = [strike("PF-1", 50000, 10), strike("PF-2", 50001, 20)];

        let fresh = StrikeLedger::new().take_new(&live);

        assert_eq!(references(&fresh), ["PF-1", "PF-2"]);
    }

    #[test]
    fn a_strike_is_new_only_once() {
        let mut ledger = StrikeLedger::new();
        ledger.take_new(&[strike("PF-1", 50000, 10)]);

        let fresh = ledger.take_new(&[strike("PF-1", 50000, 10), strike("PF-2", 50001, 20)]);

        assert_eq!(references(&fresh), ["PF-2"]);
    }

    #[test]
    fn a_strike_that_left_the_window_is_forgotten() {
        let mut ledger = StrikeLedger::new();
        ledger.take_new(&[strike("PF-1", 50000, 10), strike("PF-2", 50001, 20)]);

        ledger.take_new(&[strike("PF-2", 50001, 20)]);

        assert_eq!(ledger.announced.len(), 1);
    }

    #[test]
    fn the_persisted_ledger_round_trips_and_names_no_case() {
        let mut ledger = StrikeLedger::new();
        ledger.take_new(&[strike("PF-2026-0001", 50000, 10)]);

        let json = ledger.to_json();
        let mut restored = StrikeLedger::from_json(&json).expect("a ledger it wrote");

        assert!(!json.contains("PF-2026-0001"), "{json}");
        assert!(
            restored
                .take_new(&[strike("PF-2026-0001", 50000, 10)])
                .is_empty(),
            "a restored ledger must remember what it announced"
        );
    }

    #[test]
    fn a_ledger_of_another_version_is_refused() {
        let json = r#"{"version":2,"announced":[]}"#;

        assert!(matches!(
            StrikeLedger::from_json(json),
            Err(LedgerError::Version(2))
        ));
    }

    #[test]
    fn a_ledger_holding_a_raw_case_reference_is_refused() {
        let json = r#"{"version":1,"announced":["PF-2026-0001"]}"#;

        assert!(matches!(
            StrikeLedger::from_json(json),
            Err(LedgerError::Digest)
        ));
    }

    #[test]
    fn a_document_that_is_not_a_ledger_is_refused() {
        assert!(matches!(
            StrikeLedger::from_json("not json"),
            Err(LedgerError::Json(_))
        ));
    }
}
