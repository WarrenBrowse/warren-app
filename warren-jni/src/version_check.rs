//! Both verdicts of the signed update manifest from one read.
//!
//! The manifest answers two questions, whether the running version is still
//! allowed to run (the forced-update gate, fail-open) and whether a newer
//! stable release exists (the "update available" prompt, fail-closed). The
//! app used to fetch and verify the manifest once per question, every six
//! hours and at every start; one read now answers both. The rules are the
//! desktop's (`mullvad_update::version`), only the pairing is here, so it is
//! host-tested against hand-built manifests.

use mullvad_update::format::response::Response;
use mullvad_update::version::is_current_version_supported;
use mullvad_version::Version;

/// What the manifest says about the running version.
pub(crate) struct VersionVerdict {
    /// Whether the running version may keep running.
    pub(crate) supported: bool,
    /// The newest stable release strictly above the running one, if any.
    pub(crate) latest: Option<Version>,
}

impl VersionVerdict {
    /// The answer when the manifest could not be read (offline, bad
    /// signature, unparseable version): a flaky network must never lock the
    /// user out, and must never show an update that may not exist.
    #[cfg_attr(
        not(target_os = "android"),
        expect(dead_code, reason = "only the Android fetch answers with it")
    )]
    pub(crate) const UNKNOWN: Self = Self {
        supported: true,
        latest: None,
    };
}

/// Both verdicts for `current` from one verified manifest.
pub(crate) fn version_verdict(current: &Version, signed: &Response) -> VersionVerdict {
    VersionVerdict {
        supported: is_current_version_supported(current, signed),
        latest: newest_stable_above(current, signed),
    }
}

/// The maximum release version in `signed` that is stable (no pre-stable
/// qualifier, not a dev build) AND strictly greater than `current`, or
/// `None` when no such release exists.
fn newest_stable_above(current: &Version, signed: &Response) -> Option<Version> {
    signed
        .releases
        .iter()
        .map(|release| &release.version)
        .filter(|version| version.pre_stable.is_none() && !version.is_dev())
        .filter(|version| *version > current)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
}

#[cfg(test)]
mod tests {
    use mullvad_update::format::release::Release;
    use mullvad_update::format::response::Response;

    use super::{newest_stable_above, version_verdict};

    fn version(s: &str) -> mullvad_version::Version {
        s.parse().expect("test version must parse")
    }

    fn release(s: &str) -> Release {
        Release {
            version: version(s),
            changelog: String::new(),
            // The selector these tests exercise picks a version, never notes,
            // so an untranslated release is all they need to stand for.
            changelog_translations: std::collections::BTreeMap::new(),
            installers: Vec::new(),
            rollout: mullvad_update::version::rollout::Rollout::complete(),
        }
    }

    fn response(versions: &[&str]) -> Response {
        Response {
            releases: versions.iter().map(|v| release(v)).collect(),
            ..Response::default()
        }
    }

    #[test]
    fn one_manifest_answers_both_the_gate_and_the_prompt() {
        let mut signed = response(&["1.0.0", "1.2.0", "1.3.0"]);
        signed.minimum_supported_version = Some(version("1.2.0"));

        let verdict = version_verdict(&version("1.0.0"), &signed);

        assert!(!verdict.supported, "1.0.0 is below the signed minimum");
        assert_eq!(verdict.latest, Some(version("1.3.0")));
    }

    #[test]
    fn a_current_version_at_the_minimum_is_supported_with_nothing_newer() {
        let mut signed = response(&["1.2.0"]);
        signed.minimum_supported_version = Some(version("1.2.0"));

        let verdict = version_verdict(&version("1.2.0"), &signed);

        assert!(verdict.supported);
        assert_eq!(verdict.latest, None);
    }

    #[test]
    fn newer_stable_present_returns_some() {
        let signed = response(&["1.0.0", "1.2.0", "1.3.0"]);
        let got = newest_stable_above(&version("1.0.0"), &signed);
        assert_eq!(got, Some(version("1.3.0")));
    }

    #[test]
    fn only_same_or_older_returns_none() {
        let signed = response(&["1.0.0", "0.9.0"]);
        assert_eq!(newest_stable_above(&version("1.0.0"), &signed), None);
    }

    #[test]
    fn empty_release_list_returns_none() {
        let signed = response(&[]);
        assert_eq!(newest_stable_above(&version("1.0.0"), &signed), None);
    }

    #[test]
    fn pre_release_and_dev_above_current_are_ignored() {
        // A higher beta and a higher dev build are present, but neither is a
        // stable release, so no update should be reported.
        let signed = response(&["1.1.0-beta1", "1.2.0-dev-abc123"]);
        assert_eq!(newest_stable_above(&version("1.0.0"), &signed), None);
    }

    #[test]
    fn picks_max_stable_ignoring_higher_prerelease() {
        // 1.4.0-beta1 sorts above 1.3.0 but is not stable; the newest STABLE
        // strictly above current is 1.3.0.
        let signed = response(&["1.3.0", "1.4.0-beta1", "1.1.0"]);
        assert_eq!(
            newest_stable_above(&version("1.0.0"), &signed),
            Some(version("1.3.0"))
        );
    }
}
