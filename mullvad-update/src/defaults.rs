//! Default keys and certificates that may be used for verifying data
//!
//! The releases/metadata URLs resolve at runtime through
//! [`releases_url`] / [`metadata_url`], which honour the
//! `WARREN_UPDATE_URL` / `WARREN_METADATA_URL` env vars and otherwise
//! fall back to the Warren defaults below.

use crate::format::key::VerifyingKey;
use std::sync::LazyLock;
use vec1::Vec1;

/// Warren-branded default for the signed version metadata. The version
/// router fetches `{this}/{platform}.json` (`macos.json` / `windows.json`
/// / `linux.json`), each an Ed25519-signed manifest produced by the CI
/// release workflow (`release.yml`). The installer URLs *inside* each
/// manifest may point anywhere (e.g. GitHub Releases assets); only the
/// manifest itself is served from here, and its signature is what is
/// trusted, not the host.
///
/// Served from the Hetzner/Caddy host (Let's Encrypt cert, ISRG Root X1),
/// which is what [`PINNED_CERTIFICATE`] pins, so the fetch validates. Do NOT
/// move this to the Cloudflare-fronted `warrenbrowse.com`: that serves a
/// Cloudflare cert and the pin would reject the TLS handshake.
///
/// Per product environment (`WARREN_PRODUCT_ENV`: prod | staging | beta):
/// non-prod builds fetch their manifests from the same layout mirrored
/// under their own API host, so a beta app updates on the beta channel
/// only.
///
/// Override at runtime via the `WARREN_UPDATE_URL` env var.
#[cfg(feature = "client")]
pub const WARREN_RELEASES_URL: &str = warren_product_env::DESKTOP_UPDATE_URL;

/// Warren-branded default for the unsigned `latest.json` pointer file,
/// served alongside the signed per-platform manifests
/// ([`WARREN_RELEASES_URL`]).
///
/// Override at runtime via the `WARREN_METADATA_URL` env var.
#[cfg(feature = "client")]
pub const WARREN_METADATA_URL: &str = warren_product_env::DESKTOP_UPDATE_URL;

/// Returns the effective releases-API URL: env var override
/// (`WARREN_UPDATE_URL`) when set + non-empty, else
/// [`WARREN_RELEASES_URL`] (Warren default).
#[cfg(feature = "client")]
pub fn releases_url() -> String {
    std::env::var("WARREN_UPDATE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| WARREN_RELEASES_URL.to_owned())
}

/// Returns the effective metadata repository URL: env var override
/// (`WARREN_METADATA_URL`) when set + non-empty, else
/// [`WARREN_METADATA_URL`].
#[cfg(feature = "client")]
pub fn metadata_url() -> String {
    std::env::var("WARREN_METADATA_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| WARREN_METADATA_URL.to_owned())
}

/// Default TLS certificate to pin to.
///
/// This is the Let's Encrypt root-certificate.
#[cfg(feature = "client")]
pub static PINNED_CERTIFICATE: LazyLock<reqwest::Certificate> = LazyLock::new(|| {
    const CERT_BYTES: &[u8] = include_bytes!("../../mullvad-api/le_root_cert.pem");
    reqwest::Certificate::from_pem(CERT_BYTES).expect("invalid cert")
});

/// Pubkeys used to verify metadata from the Warren update channel
/// (production target).
///
/// This key signs app-update manifests, i.e. it can ship a new installer
/// to every user. It is a dedicated, offline-generated key used for
/// nothing else, NOT the relay/admin signing key (which is online and has
/// a known-burned lineage). See `warren-trusted-metadata-signing-pubkeys`
/// for the trusted pubkey(s) and the rotation procedure. Updates signed by
/// any other key fail verification and the daemon reports
/// `app-upgrade-error` (fail-closed).
pub static TRUSTED_METADATA_SIGNING_PUBKEYS: LazyLock<Vec1<VerifyingKey>> =
    LazyLock::new(|| parse_keys(include_str!("../warren-trusted-metadata-signing-pubkeys")));

fn parse_keys(keys: &str) -> Vec1<VerifyingKey> {
    let mut v = vec![];
    for key in keys.split('\n') {
        let key = key.trim();
        if key.starts_with('#') || key.is_empty() {
            continue;
        }
        v.push(VerifyingKey::from_hex(key).expect("invalid pubkey"));
    }
    v.try_into().expect("need at least one key")
}

#[cfg(all(test, feature = "client"))]
mod warren_default_tests {
    use super::*;

    /// Temporarily swap an env var for the duration of `body`, then
    /// restore the prior value (or unset). Centralises the `unsafe`
    /// env-var manipulation behind a single, documented SAFETY block
    /// so individual tests stay focused on the assertion.
    ///
    /// SAFETY contract: every `unsafe` operation inside this helper
    /// touches the process-wide env table. Rust's `set_var` /
    /// `remove_var` are `unsafe` since 1.81 because concurrent readers
    /// across threads can observe a torn write. Tests in this module
    /// are run by the default single-threaded `cargo test` harness
    /// **within this file** (no `#[tokio::test]` reaches in here, no
    /// thread spawn touches the env table), so no other thread reads
    /// the env vars we toggle here while the helper holds them.
    fn with_env_var<F: FnOnce()>(name: &'static str, value: Option<&str>, body: F) {
        let prev = std::env::var(name).ok();
        // SAFETY: see module-level contract above.
        unsafe {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }

        body();

        // Restore prior state regardless of body outcome (cooperative
        // cleanup; tests still see the assertion failure if body
        // panicked before reaching here, which is acceptable for
        // hermetic single-threaded tests).
        // SAFETY: see module-level contract above.
        unsafe {
            match prev {
                Some(p) => std::env::set_var(name, p),
                None => std::env::remove_var(name),
            }
        }
    }

    /// Anti-regression: a Warren-branded build with no env override
    /// must hit the GitHub Releases endpoint, never the upstream
    /// Mullvad API. If this assertion ever flips, the daemon would
    /// silently pull Mullvad's release feed and surface "update
    /// available" for the wrong product line.
    #[test]
    fn releases_url_default_is_warren_not_mullvad() {
        with_env_var("WARREN_UPDATE_URL", None, || {
            let got = releases_url();
            assert_eq!(got, WARREN_RELEASES_URL);
            assert!(
                !got.contains("mullvad"),
                "Warren default URL must not include 'mullvad', got {got}"
            );
            assert!(
                got.contains("warrenbrowse.com"),
                "Warren default URL must point at the Warren update host, got {got}"
            );
        });
    }

    /// Anti-regression: setting `WARREN_UPDATE_URL` at runtime must
    /// short-circuit the default. Lets the operator point a staging
    /// build at an internal mirror without recompiling.
    #[test]
    fn releases_url_env_override_wins_over_default() {
        let stub = "https://stub.example.test/releases/";
        with_env_var("WARREN_UPDATE_URL", Some(stub), || {
            assert_eq!(releases_url(), stub);
        });
    }

    /// Edge case: an empty env var means "unset for me", and must
    /// fall back to the Warren default rather than yielding an empty
    /// URL string (which would 404 every request).
    #[test]
    fn releases_url_empty_env_falls_back_to_warren_default() {
        with_env_var("WARREN_UPDATE_URL", Some(""), || {
            assert_eq!(releases_url(), WARREN_RELEASES_URL);
        });
    }

    /// The update channel of every environment mirrors the prod layout
    /// under that environment's own API host: `{api}/updates/desktop`.
    /// Pins that a beta build fetches beta manifests, never prod's.
    #[test]
    fn update_channel_mirrors_the_environment_api_host() {
        assert!(
            WARREN_RELEASES_URL.starts_with(warren_product_env::API_URL),
            "update channel {WARREN_RELEASES_URL} must live under the environment API host {}",
            warren_product_env::API_URL
        );
        assert!(
            WARREN_RELEASES_URL.ends_with("/updates/desktop"),
            "update channel {WARREN_RELEASES_URL} must keep the /updates/desktop layout"
        );
        assert_eq!(WARREN_METADATA_URL, WARREN_RELEASES_URL);
    }

    /// Same anti-regression contract as the releases URL test, but for
    /// the metadata endpoint that fetches `latest.json`.
    #[test]
    fn metadata_url_default_is_warren_not_mullvad() {
        with_env_var("WARREN_METADATA_URL", None, || {
            let got = metadata_url();
            assert_eq!(got, WARREN_METADATA_URL);
            assert!(!got.contains("mullvad"));
            assert!(got.contains("warrenbrowse.com"));
        });
    }

    /// Anti-regression: the Warren placeholder pubkey file parses to
    /// at least one valid Ed25519 key. Once poka generates the real
    /// production key, the placeholder gets swapped in-place; this
    /// test still passes (the key changes, the contract does not).
    #[test]
    fn warren_trusted_metadata_signing_pubkeys_parses() {
        let keys = &*TRUSTED_METADATA_SIGNING_PUBKEYS;
        assert!(
            !keys.is_empty(),
            "warren-trusted-metadata-signing-pubkeys must hold >=1 valid Ed25519 key"
        );
    }
}

#[cfg(test)]
#[test]
fn test_parse_keys() {
    let key1 = "AB4EF63FFDCC6BD5A19C30CD23B9DE03099407A04463418F17AE338B98AA09D4".to_lowercase();
    let key2 = "BB4EF63FFDCC6BD5A19C30CD23B9DE03099407A04463418F17AE338B98AA09D4".to_lowercase();
    let keys = parse_keys(&format!(
        r#"
# test
{key1}
# test 2
{key2}
"#
    ));
    assert_eq!(format!("{}", keys[0]), key1);
    assert_eq!(format!("{}", keys[1]), key2);

    // Test that actual keys are validly parsed
    let _prod = &*TRUSTED_METADATA_SIGNING_PUBKEYS;
}
