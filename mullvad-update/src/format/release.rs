//! App release

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::installer::Installer;
use crate::version::rollout::{Rollout, is_complete_rollout};

/// App release
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Release {
    /// Mullvad app version
    pub version: mullvad_version::Version,
    /// Changelog entries, in English
    pub changelog: String,
    /// Changelog entries per language tag (`fr`, `ro`, ...), for clients that
    /// run in another language than English. `changelog` stays the fallback,
    /// so a language absent here simply reads the English notes.
    ///
    /// Adding this stayed backwards compatible on purpose: the signature covers
    /// the raw JSON of `signed`, and unlike [`super::response::Response`] this
    /// type does not deny unknown fields, so a client built before the field
    /// existed still verifies and reads a manifest that carries it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changelog_translations: BTreeMap<String, String>,
    /// Installer details for different architectures
    pub installers: Vec<Installer>,
    /// Fraction of users that should receive the new version
    #[serde(default = "Rollout::complete")]
    #[serde(skip_serializing_if = "is_complete_rollout")]
    pub rollout: Rollout,
}

impl PartialEq for Release {
    fn eq(&self, other: &Self) -> bool {
        self.version.eq(&other.version)
    }
}

impl PartialOrd for Release {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.version.partial_cmp(&other.version)
    }
}
