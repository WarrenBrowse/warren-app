//! The public environment descriptor (`GET /v1/network`) the beta badge reads.
//!
//! iOS counterpart of Android's `warren-jni` `fetchNetworkInfo`. Display data
//! only, and unauthenticated: the environment label, whether the service is
//! deliberately degraded, the default bandwidth cap and whether payments are
//! offered here. The enforcement lives on the exits; this is only what the app
//! tells the user about the network it is on.
//!
//! An app that never asked showed a beta build with no way for its user to
//! learn that the speed they are getting is the beta's cap rather than their
//! own line, which desktop and Android both say.
//!
//! Memory ownership: the returned heap `CString` MUST be freed once via
//! `warren_wallet_free_mnemonic` (type-agnostic: it reclaims any `CString`
//! this crate produces).

#![cfg(target_os = "ios")]

use std::ffi::CString;
use std::os::raw::c_char;

use warren_api::{HttpRequest, HttpTransport, Method, ReqwestTransport};

/// Fetches the environment descriptor.
///
/// Returns `{"ok":true,"environment":..,"degraded":..,"default_rate_bps":..,
/// "payments_enabled":..}` or `{"ok":false}` for any failure, including an API
/// that predates the endpoint; Swift reads both as "no info" and says the
/// shorter thing.
///
/// Blocking: runs the GET on the shared iOS runtime, so call it off the main
/// thread.
///
/// # Safety
///
/// The returned pointer must be freed exactly once via
/// `warren_wallet_free_mnemonic`. Never null except on allocation failure.
#[unsafe(no_mangle)]
pub extern "C" fn warren_fetch_network_info() -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        CString::new(fetch_network_info_json())
            .map(CString::into_raw)
            .unwrap_or(std::ptr::null_mut())
    })
}

fn fetch_network_info_json() -> String {
    let fail = || r#"{"ok":false}"#.to_owned();
    let Ok(handle) = crate::warren_ios_runtime() else {
        return fail();
    };
    let request = HttpRequest {
        method: Method::Get,
        url: format!(
            "{}/v1/network",
            warren_product_env::API_URL.trim_end_matches('/')
        ),
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    match handle.block_on(ReqwestTransport::new().execute(request)) {
        Ok(response) if response.status == 200 => {
            match serde_json::from_slice::<warren_contract::dto::NetworkInfoResponse>(
                &response.body,
            ) {
                Ok(info) => serde_json::json!({
                    "ok": true,
                    "environment": info.environment,
                    "degraded": info.degraded,
                    "default_rate_bps": info.default_rate_bps,
                    "payments_enabled": info.payments_enabled,
                })
                .to_string(),
                Err(_) => fail(),
            }
        }
        // 404 is an API that predates the endpoint: the same "no info" answer
        // as a transport failure, without error noise.
        _ => fail(),
    }
}
