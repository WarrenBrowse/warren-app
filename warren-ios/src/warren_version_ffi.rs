//! Signed update-manifest verification for the iOS version gate.
//!
//! The Swift side fetches `ios.json` from the update host with URLSession and
//! hands the raw bytes here. This module verifies the Ed25519 signature
//! against the pinned trusted metadata key, rejects expired metadata, and
//! applies the shared `minimum_supported_version` rule, so every platform
//! (desktop daemon, Android via warren-jni, iOS via this FFI) enforces the
//! exact same forced-update policy from the exact same verifier
//! (`mullvad-update`). No network or crypto logic lives in Swift.

use std::ffi::{CStr, CString, c_char};

use mullvad_update::format::response::SignedResponse;
use mullvad_update::version::{MIN_VERIFY_METADATA_VERSION, is_current_version_supported};

/// Verify a signed update manifest and evaluate it against the running app
/// version.
///
/// `manifest` / `manifest_len` : raw bytes of the fetched `ios.json`.
/// `current_version` : null-terminated running app version string
/// (e.g. `2026.3` or `2026.3-dev1`).
///
/// Returns a heap-allocated JSON C string
/// `{"supported":bool,"latest_version":"X.Y.Z"}` when the manifest signature
/// and expiry verify (`latest_version` is omitted when the manifest lists no
/// releases). Returns null when verification fails (bad signature, expired
/// metadata, unparseable manifest, invalid input): the caller must then treat
/// the manifest as absent, never trust its content.
///
/// An unparseable `current_version` yields `supported: true` (fail-open: a
/// version-string surprise must not lock users out), while dev builds are
/// always supported per the shared rule.
///
/// # Safety
/// `manifest` must point to `manifest_len` readable bytes. `current_version`
/// must be a valid null-terminated C string. The returned pointer must be
/// passed to `warren_version_check_free` exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_version_check_verify(
    manifest: *const u8,
    manifest_len: usize,
    current_version: *const c_char,
) -> *mut c_char {
    if manifest.is_null() || current_version.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: `manifest` points to `manifest_len` readable bytes (fn precondition).
    let bytes = unsafe { std::slice::from_raw_parts(manifest, manifest_len) };
    // SAFETY: `current_version` is a valid null-terminated C string (fn precondition).
    let current = match unsafe { CStr::from_ptr(current_version) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let Some(json) = verify_manifest(bytes, current) else {
        return std::ptr::null_mut();
    };
    match CString::new(json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a string previously returned by `warren_version_check_verify`.
/// No-op on null.
///
/// # Safety
/// `ptr` must have been returned by `warren_version_check_verify` and must
/// not have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_version_check_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ptr` came from `CString::into_raw` and is not yet freed (fn precondition).
    drop(unsafe { CString::from_raw(ptr) });
}

/// Pure core: verify + evaluate, `None` on any verification failure.
fn verify_manifest(manifest: &[u8], current_version: &str) -> Option<String> {
    let response = SignedResponse::deserialize_and_verify(manifest, MIN_VERIFY_METADATA_VERSION)
        .ok()?
        .signed;

    let supported = match current_version.parse::<mullvad_version::Version>() {
        Ok(current) => is_current_version_supported(&current, &response),
        // Fail-open: an unexpected local version string must not brick the app.
        Err(_) => true,
    };

    let latest = response
        .releases
        .iter()
        .map(|release| &release.version)
        .fold(None::<&mullvad_version::Version>, |acc, v| match acc {
            Some(a) if a.partial_cmp(v).is_some_and(|o| o.is_ge()) => Some(a),
            _ => Some(v),
        });

    let mut out = serde_json::json!({ "supported": supported });
    if let Some(latest) = latest {
        out["latest_version"] = serde_json::json!(latest.to_string());
    }
    Some(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::verify_manifest;

    /// Real production manifest snapshot, signed by the pinned trusted
    /// metadata key. Refresh from
    /// `https://api.warrenbrowse.com/updates/desktop/ios.json` when its
    /// `metadata_expiry` passes (the success-path tests then start failing
    /// on expiry, by design).
    const FIXTURE: &str = include_str!("../tests/fixtures/ios-manifest.json");

    fn parse(json: &str) -> serde_json::Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn accepts_signed_manifest_and_reports_supported_version() {
        let out = verify_manifest(FIXTURE.as_bytes(), "2026.3").expect("manifest must verify");
        let v = parse(&out);
        // The fixture pins minimum_supported_version 2026.3.
        assert_eq!(v["supported"], true);
        assert_eq!(v["latest_version"], "2026.3");
    }

    #[test]
    fn blocks_version_below_minimum() {
        // A pre-stable of the minimum orders below it (2026.3-alpha1 < 2026.3).
        let out =
            verify_manifest(FIXTURE.as_bytes(), "2026.3-alpha1").expect("manifest must verify");
        assert_eq!(parse(&out)["supported"], false);
    }

    #[test]
    fn dev_build_is_always_supported() {
        let out = verify_manifest(FIXTURE.as_bytes(), "2026.3-alpha1-dev-abc123")
            .expect("manifest must verify");
        assert_eq!(parse(&out)["supported"], true);
    }

    #[test]
    fn unparseable_current_version_fails_open() {
        let out = verify_manifest(FIXTURE.as_bytes(), "???").expect("manifest must verify");
        assert_eq!(parse(&out)["supported"], true);
    }

    #[test]
    fn rejects_tampered_manifest() {
        // Raising the minimum without re-signing must break verification.
        let tampered = FIXTURE.replace(
            "\"minimum_supported_version\": \"2026.3\"",
            "\"minimum_supported_version\": \"9999.0.0\"",
        );
        assert_ne!(tampered, FIXTURE, "fixture must contain the minimum field");
        assert!(verify_manifest(tampered.as_bytes(), "2026.3").is_none());
    }

    #[test]
    fn rejects_garbage_input() {
        assert!(verify_manifest(b"not json at all", "2026.3").is_none());
        assert!(verify_manifest(b"{}", "2026.3").is_none());
        assert!(verify_manifest(b"", "2026.3").is_none());
    }
}
