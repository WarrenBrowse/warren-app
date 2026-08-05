use std::cmp::Ordering;

use anyhow::Context;
use itertools::Itertools;
use mullvad_version::PreStableType;

use crate::format::installer::Installer;
use crate::format::release::Release;
use crate::format::response::Response;
use crate::version::parameters::VersionParameters;

/// Lowest version to accept using 'verify'
pub const MIN_VERIFY_METADATA_VERSION: usize = 0;

/// Version update information derived from querying a [`Response`] and filtering with [`VersionParameters`]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct VersionInfo {
    /// Stable version info
    pub stable: Metadata,
    /// Beta version info (if available and newer than `stable`).
    /// If latest stable version is newer, this will be `None`.
    pub beta: Option<Metadata>,
}

/// Contains information about a version for the current target
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Metadata {
    /// Version
    pub version: mullvad_version::Version,
    /// URLs to use for downloading the app installer
    pub urls: Vec<String>,
    /// Size of installer, in bytes
    pub size: usize,
    /// Version changelog, in English
    pub changelog: String,
    /// Version changelog per language tag, English staying the fallback
    pub changelog_translations: std::collections::BTreeMap<String, String>,
    /// App installer checksum
    pub sha256: [u8; 32],
}

impl VersionInfo {
    /// Convert signed response data to public version type
    /// NOTE: `response` is assumed to be verified and untampered. It is not verified.
    pub fn try_from_response(
        params: &VersionParameters,
        response: Response,
    ) -> anyhow::Result<Self> {
        // Fail if there are duplicate versions.
        // Check this before anything else so that it's rejected independently of `params`.
        if !response.releases.iter().map(|r| &r.version).all_unique() {
            anyhow::bail!("API response contains multiple release for the same version");
        }

        // Filter releases based on rollout, architecture and dev versions.
        let available_versions: Vec<Metadata> = response.releases
        .into_iter()
        // Filter out releases that are not rolled out to us
        .filter(|release| release.rollout >= params.rollout)
        // Filter out dev versions
        .filter(|release| !release.version.is_dev())
        .flat_map(|Release { version, changelog, changelog_translations, installers, .. }| {
            if installers.is_empty() && params.allow_empty {
                // HACK: If there are no installers (e.g. on Linux), return the version anyway
                return Some(anyhow::Ok(Metadata {
                    version,
                    size: 0,
                    urls: vec![],
                    changelog,
                    changelog_translations,
                    sha256: [0u8; 32],
                }));
            }
            installers
                .into_iter()
                // Find installer for the requested architecture (assumed to be unique)
                .find(|installer| params.architecture == installer.architecture)
                // Map each artifact to a [Version]
                .map(|Installer { urls, size, sha256,.. }| {
                    anyhow::Ok(Metadata {
                        version,
                        size,
                        urls,
                        changelog,
                        changelog_translations,
                        sha256: hex::decode(sha256)
                        .context("Invalid checksum hex")?
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Invalid checksum length"))?,
                    })
                })
        }).try_collect()?;

        // Find latest stable version
        let stable = available_versions
            .iter()
            .filter(|release| release.version.pre_stable.is_none())
            .max_by(|a, b| a.version.partial_cmp(&b.version).unwrap_or(Ordering::Equal))
            .context("No stable version found")?
            .clone();

        // Find the latest beta version
        let beta = available_versions
            .iter()
            .filter(|release| matches!(release.version.pre_stable, Some(PreStableType::Beta(_))))
            // If the latest beta version is older than latest stable, dispose of it
            .filter(|release| release.version > stable.version)
            .max_by(|a, b| a.version.partial_cmp(&b.version).unwrap_or(Ordering::Equal))
            .cloned();

        Ok(Self { stable, beta })
    }
}

/// A version is considered supported if the version exists in the metadata. Versions with a
/// rollout of 0 is still considered supported.
pub fn is_version_supported(
    current_version: mullvad_version::Version,
    response: &Response,
) -> bool {
    response
        .releases
        .iter()
        .any(|release| release.version.eq(&current_version))
}

/// Decide whether the running app version is still supported, i.e. allowed to
/// keep running (the inverse of "must force-update").
///
/// If the manifest declares a [`Response::minimum_supported_version`], the app
/// is supported iff it is at least that version. This is the forced-update
/// lever: raise the minimum to hard-block older clients. Otherwise the app is
/// supported when it is listed in `releases`, or when it is newer than every
/// release listed there. Dev builds are always supported so local testing is
/// never blocked.
///
/// Being newer than the whole manifest counts as supported because the manifest
/// only ever describes what has been published: a build is newer than it for the
/// whole window between a release's build job and its publish job, and for every
/// local or pre-release build. Blocking there raises the forced-update screen on
/// whoever runs the newest code, and no update can clear it.
///
/// Shared by the desktop daemon and the mobile (`warren-jni`) version check so
/// every platform applies the same rule.
pub fn is_current_version_supported(
    current_version: &mullvad_version::Version,
    response: &Response,
) -> bool {
    if current_version.is_dev() {
        return true;
    }
    match response.minimum_supported_version.as_ref() {
        Some(minimum) => current_version >= minimum,
        None => {
            is_version_supported(current_version.clone(), response)
                || response
                    .releases
                    .iter()
                    .all(|release| current_version > &release.version)
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use insta::assert_yaml_snapshot;

    use crate::format::response::*;
    use crate::version::info::*;
    use crate::version::parameters::*;
    use crate::version::rollout::*;

    // These tests rely on `insta` for snapshot testing. If they fail due to snapshot assertions,
    // then most likely the snapshots need to be updated. The most convenient way to review
    // changes to, and update, snapshots is by running `cargo insta review`.

    const TEST_RESPONSE: &[u8] = include_bytes!("../../test-version-response.json");

    /// Test version info response handler (rollout 1, x86)
    #[test]
    fn test_version_info_parser_x86() -> anyhow::Result<()> {
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;

        let params = VersionParameters {
            architecture: Architecture::X86,
            rollout: FULLY_ROLLED_OUT,
            allow_empty: false,
            lowest_metadata_version: 0,
        };

        // Expect: The available latest versions for X86, where the rollout is 1.
        let info = VersionInfo::try_from_response(&params, response.signed.clone())?;

        assert_yaml_snapshot!(info);

        Ok(())
    }

    /// Test version info response handler (rollout 0.01, arm64)
    #[test]
    fn test_version_info_parser_arm64() -> anyhow::Result<()> {
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;

        let params = VersionParameters {
            architecture: Architecture::Arm64,
            rollout: SUPPORTED_VERSION,
            allow_empty: false,
            lowest_metadata_version: 0,
        };

        let info = VersionInfo::try_from_response(&params, response.signed)?;

        // Expect: The available latest versions for arm64, where the rollout is .01.
        assert_yaml_snapshot!(info);

        Ok(())
    }

    /// Versions without installers should be returned if `allow_empty` is set
    #[test]
    fn test_version_info_empty() -> anyhow::Result<()> {
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;

        let params = VersionParameters {
            architecture: Architecture::X86,
            rollout: SUPPORTED_VERSION,
            allow_empty: true,
            lowest_metadata_version: 0,
        };

        let info = VersionInfo::try_from_response(&params, response.signed)?;

        // Expect: The available latest versions for x86, where the rollout is .01.
        assert_yaml_snapshot!(info);

        Ok(())
    }

    /// Test whether [SUPPORTED_VERSION] ignores unsupported versions (where rollout = 0.0)
    #[test]
    fn test_version_unsupported_filtering() -> anyhow::Result<()> {
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;

        let params = VersionParameters {
            architecture: Architecture::X86,
            rollout: SUPPORTED_VERSION,
            allow_empty: true,
            lowest_metadata_version: 0,
        };

        let info = VersionInfo::try_from_response(&params, response.signed.clone())?;

        // Expect: The available latest versions for x86, where the rollout is non-zero.
        assert_yaml_snapshot!(info);

        let params = VersionParameters {
            architecture: Architecture::X86,
            rollout: IGNORE,
            allow_empty: true,
            lowest_metadata_version: 0,
        };

        let info = VersionInfo::try_from_response(&params, response.signed)?;

        // Expect: There is an even higher version where the rollout is zero.
        assert_yaml_snapshot!(info);

        Ok(())
    }

    #[test]
    fn test_is_version_supported() -> anyhow::Result<()> {
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;

        let supported_version = mullvad_version::Version::from_str("2025.3").unwrap();
        let supported_rollout_zero_version = mullvad_version::Version::from_str("2030.3").unwrap();
        let non_supported_version = mullvad_version::Version::from_str("2025.5").unwrap();

        assert!(is_version_supported(supported_version, &response.signed));
        assert!(is_version_supported(
            supported_rollout_zero_version,
            &response.signed
        ));
        assert!(!is_version_supported(
            non_supported_version,
            &response.signed
        ));

        Ok(())
    }

    #[test]
    fn a_version_newer_than_every_release_is_supported() -> anyhow::Result<()> {
        // A build is newer than the manifest for the whole window between a
        // release's build job and its publish job, and for every local or
        // pre-release build. Blocking there tells whoever runs the newest code
        // that their version is no longer supported, and the forced-update
        // screen it raises cannot be cleared by updating.
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;
        let newer_than_every_release = mullvad_version::Version::from_str("2031.1").unwrap();

        assert!(is_current_version_supported(
            &newer_than_every_release,
            &response.signed
        ));

        Ok(())
    }

    #[test]
    fn an_older_unlisted_version_stays_unsupported() -> anyhow::Result<()> {
        // The force-update lever must keep working: only versions ABOVE the
        // whole manifest get the benefit of the doubt.
        let response = SignedResponse::deserialize_insecure(TEST_RESPONSE)?;
        let older_unlisted = mullvad_version::Version::from_str("2025.5").unwrap();

        assert!(!is_current_version_supported(
            &older_unlisted,
            &response.signed
        ));

        Ok(())
    }
}
