//! The Android exports of the community-forum flows: the wallet-signed login
//! approval, its cancel, the in-app bug report and the problem-report
//! collection behind it.
//!
//! Everything that decides bytes lives in [`crate::forum`] (shared with iOS)
//! and [`crate::report`]; this module only crosses the JNI boundary, picks
//! the transport, and classes the outcome. Two rules hold on every path:
//! only fixed phrases and coarse classes are ever logged (no sid, no
//! address, no nonce, no body), and the mnemonic lives in a `Zeroizing`
//! buffer for the duration of one call.

use jnix::{
    FromJava, JnixEnv,
    jni::{
        JNIEnv,
        objects::{JClass, JString},
        sys::{jbyteArray, jstring},
    },
};
use warren_api::{HttpRequest, HttpResponse, HttpTransport, Method, TransportError};
use zeroize::Zeroizing;

use crate::forum::{self, FailReason, ForumLoginOutcome, ForumRequestError, ReportOutcome};

/// The transport the forum requests ride: the VpnService-protected one when
/// the crate carries it, so the request never black-holes into a TUN that is
/// still coming up, is blocking, or is wedged (the token mint learnt this the
/// hard way); the plain SDK transport otherwise. Without a registered
/// protector the protected transport is a plain socket, so a device with no
/// VPN service alive behaves exactly as before.
#[cfg(feature = "tunnel")]
type ForumTransport = crate::protected_transport::ProtectedTransport;
#[cfg(not(feature = "tunnel"))]
type ForumTransport = warren_api::ReqwestTransport;

fn forum_transport() -> ForumTransport {
    ForumTransport::new()
}

/// A GET whose `Date` header is read back, for the clock preflight.
#[cfg(feature = "tunnel")]
async fn get_dated(
    transport: &ForumTransport,
    url: String,
) -> Result<(HttpResponse, Option<String>), TransportError> {
    transport
        .execute_dated(HttpRequest {
            method: Method::Get,
            url,
            headers: Vec::new(),
            body: Vec::new(),
            use_sni: true,
        })
        .await
}

#[cfg(not(feature = "tunnel"))]
async fn get_dated(
    transport: &ForumTransport,
    url: String,
) -> Result<(HttpResponse, Option<String>), TransportError> {
    let response = transport
        .execute(HttpRequest {
            method: Method::Get,
            url,
            headers: Vec::new(),
            body: Vec::new(),
            use_sni: true,
        })
        .await?;
    Ok((response, None))
}

/// What the preflight of a login learnt.
enum Preflight {
    /// The session still waits; `offset_secs` corrects the device clock.
    Pending { offset_secs: i64 },
    /// The session is gone (404): signing would only spend a nonce.
    Gone,
    /// The preflight itself failed: sign with the device clock and let the
    /// provider decide, as before the preflight existed.
    Unknown,
}

/// Reads the session's status once before signing. The `Date` header of that
/// TLS-authenticated answer is the trusted clock a device that never
/// synchronised its own can be corrected against, which turns the 2026-08-18
/// class (every attempt refused by the 60 s window) into a login that works.
async fn preflight(transport: &ForumTransport, sid: &str, host: &str) -> Preflight {
    let Some(url) = forum::build_status_url(sid, host) else {
        return Preflight::Unknown;
    };
    match get_dated(transport, url).await {
        Ok((response, date)) if response.status == 404 => {
            let _ = date;
            Preflight::Gone
        }
        Ok((response, date)) if (200..300).contains(&response.status) => {
            let device_now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let offset_secs = date
                .as_deref()
                .and_then(|d| forum::clock_offset_secs(d, device_now))
                .unwrap_or(0);
            if offset_secs.abs() > 30 {
                log::warn!(
                    "forumLogin: device clock is {offset_secs} s off the connect host, correcting"
                );
            }
            Preflight::Pending { offset_secs }
        }
        Ok((response, _)) => {
            log::info!(
                "forumLogin: status preflight answered {}, signing with the device clock",
                response.status
            );
            Preflight::Unknown
        }
        Err(_) => {
            log::warn!("forumLogin: status preflight failed (transport), signing anyway");
            Preflight::Unknown
        }
    }
}

/// Sign and submit a forum-login challenge for `sid` to the connect `host`
/// (`POST /v1/forum/login`).
///
/// Mirrors the desktop daemon's SignForumLogin plus its POST: only the opaque
/// `sid` and the connect `host` cross the boundary. The mnemonic derives the
/// signing key for this call and is not retained; the wallet signature never
/// surfaces to Kotlin because the request is signed AND sent here (like every
/// other signed JNI call). `host` is checked against a hard allowlist so a
/// hostile deep link cannot redirect the signed request. Returns the envelope
/// of [`crate::forum::envelope`]: `{"ok":true,...}` with the handle and slot
/// on acceptance, `{"ok":false,"error":"subscription-required"}` when the
/// wallet has never subscribed (HTTP 403), `clock-skew` when the provider
/// refused the signature for a device clock outside its window, `expired`
/// when the session is gone, or `error` with a `reason` class. The mnemonic,
/// sid, signature and nonce are never logged.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_forumLogin<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    sid: JString<'local>,
    host: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let sid = String::from_java(&jnix_env, sid);
    let host = String::from_java(&jnix_env, host);
    let outcome = forum_login(&phrase, &sid, &host);
    let json = forum::envelope(&outcome);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Drive the login: preflight the session (clock offset, dead session), build
/// the signed request at the corrected time, execute it on the shared runtime
/// through the forum transport, and map the answer. Every failure names its
/// class in the log and in the envelope, never a value.
fn forum_login(mnemonic: &str, sid: &str, host: &str) -> ForumLoginOutcome {
    let Some(runtime) = crate::android_jni::runtime() else {
        log::warn!("forumLogin: initLogger must run first");
        return ForumLoginOutcome::Failed(FailReason::Runtime);
    };
    if !forum::is_allowed_connect_host(host) || !forum::is_valid_sid(sid) {
        log::warn!("forumLogin: refused a link outside the allowlist or sid shape");
        return ForumLoginOutcome::Failed(FailReason::Build);
    }
    let transport = forum_transport();
    let offset_secs = match runtime.block_on(preflight(&transport, sid, host)) {
        Preflight::Pending { offset_secs } => offset_secs,
        Preflight::Gone => {
            log::info!("forumLogin: session already gone before signing");
            return ForumLoginOutcome::Expired;
        }
        Preflight::Unknown => 0,
    };
    let Some(timestamp) = forum::timestamp_with_offset(offset_secs) else {
        log::warn!("forumLogin: could not stamp the request");
        return ForumLoginOutcome::Failed(FailReason::Build);
    };
    let signed = match forum::build_signed_request_at(mnemonic, sid, host, timestamp) {
        Ok(req) => req,
        Err(_) => {
            // No values: the cause could otherwise echo sid/host/mnemonic state.
            log::warn!("forumLogin: could not build signed request");
            return ForumLoginOutcome::Failed(FailReason::Build);
        }
    };
    let request = HttpRequest {
        method: Method::Post,
        url: signed.url,
        headers: signed.headers,
        body: signed.body,
        use_sni: true,
    };
    let started = std::time::Instant::now();
    match runtime.block_on(transport.execute(request)) {
        Ok(response) => {
            let outcome = forum::outcome_for_response(response.status, &response.body);
            log::info!(
                "forumLogin: provider answered {} in {} ms ({})",
                response.status,
                started.elapsed().as_millis(),
                outcome_class(&outcome)
            );
            outcome
        }
        Err(err) => {
            log::warn!(
                "forumLogin: transport error ({}) after {} ms",
                transport_class(&err),
                started.elapsed().as_millis()
            );
            ForumLoginOutcome::Failed(FailReason::Transport)
        }
    }
}

fn outcome_class(outcome: &ForumLoginOutcome) -> &'static str {
    match outcome {
        ForumLoginOutcome::Approved(Some(_)) => "approved with identity",
        ForumLoginOutcome::Approved(None) => "approved",
        ForumLoginOutcome::SubscriptionRequired => "subscription required",
        ForumLoginOutcome::ClockSkew => "clock skew",
        ForumLoginOutcome::Expired => "expired",
        ForumLoginOutcome::Failed(_) => "failed",
    }
}

/// The transport failure class. The SDK's error strings are address-free by
/// construction, so the class is what they say.
fn transport_class(err: &TransportError) -> &'static str {
    match err {
        TransportError::Connect(_) => "connect",
        TransportError::Io(msg) if msg.contains("timed out") => "timeout",
        TransportError::Io(msg) if msg.contains("protect") => "protect-refused",
        TransportError::Io(_) => "io",
        _ => "other",
    }
}

/// Best-effort: tell the connect provider the user declined the forum login
/// (`POST /v1/session/<sid>/cancel`) so the waiting browser page unblocks
/// instead of polling to timeout. Unsigned (no wallet material); mirrors the
/// desktop `cancelForumLogin`. Failures are ignored: the server session expires
/// on its own in 5 minutes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_forumLoginCancel<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    sid: JString<'local>,
    host: JString<'local>,
) {
    let jnix_env = JnixEnv::from(env);
    let sid = String::from_java(&jnix_env, sid);
    let host = String::from_java(&jnix_env, host);
    forum_cancel(&sid, &host);
}

fn forum_cancel(sid: &str, host: &str) {
    let Some(url) = forum::build_cancel_url(sid, host) else {
        return;
    };
    let Some(runtime) = crate::android_jni::runtime() else {
        return;
    };
    let request = HttpRequest {
        method: Method::Post,
        url,
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    // Best-effort: a failed cancel just means the browser polls to timeout.
    let _ = runtime.block_on(forum_transport().execute(request));
}

/// Sign and submit an in-app bug report (`POST /v1/forum/report`).
///
/// `report_json` is the form as one JSON object (the connect contract's field
/// names); `log_gz` is the gzipped redacted problem report, or null to file
/// the report without logs. Signed AND sent here, like the login. Returns the
/// envelope of [`crate::forum::report_envelope`].
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_forumReport<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    mnemonic: JString<'local>,
    report_json: JString<'local>,
    log_gz: jbyteArray,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let phrase = Zeroizing::new(String::from_java(&jnix_env, mnemonic));
    let report_json = String::from_java(&jnix_env, report_json);
    let log_gz = if log_gz.is_null() {
        None
    } else {
        match jnix_env.convert_byte_array(log_gz) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                let json = forum::report_envelope(&ReportOutcome::Failed(FailReason::Build));
                return match jnix_env.new_string(json) {
                    Ok(s) => s.into_inner() as jstring,
                    Err(_) => std::ptr::null_mut(),
                };
            }
        }
    };
    let outcome = forum_report(&phrase, &report_json, log_gz.as_deref());
    let json = forum::report_envelope(&outcome);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Same shape as the login: a clock preflight against the connect host (its
/// health endpoint, since a report has no session), the signed request at the
/// corrected time, the forum transport, a classed outcome.
fn forum_report(mnemonic: &str, report_json: &str, log_gz: Option<&[u8]>) -> ReportOutcome {
    let Some(runtime) = crate::android_jni::runtime() else {
        log::warn!("forumReport: initLogger must run first");
        return ReportOutcome::Failed(FailReason::Runtime);
    };
    let transport = forum_transport();
    let device_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let health = format!("https://{}/healthz", forum::connect_host());
    let offset_secs = match runtime.block_on(get_dated(&transport, health)) {
        Ok((_, date)) => date
            .as_deref()
            .and_then(|d| forum::clock_offset_secs(d, device_now))
            .unwrap_or(0),
        Err(err) => {
            log::warn!(
                "forumReport: clock preflight failed ({}), signing anyway",
                transport_class(&err)
            );
            0
        }
    };
    let Some(timestamp) = forum::timestamp_with_offset(offset_secs) else {
        return ReportOutcome::Failed(FailReason::Build);
    };
    let signed = match forum::build_signed_report_request(mnemonic, report_json, log_gz, timestamp)
    {
        Ok(req) => req,
        Err(ForumRequestError::LogTooLarge) => {
            log::warn!("forumReport: log over the size cap, not sent");
            return ReportOutcome::TooLarge;
        }
        Err(ForumRequestError::Invalid) => {
            log::warn!("forumReport: could not build signed request");
            return ReportOutcome::Failed(FailReason::Build);
        }
    };
    let bytes = signed.body.len();
    let request = HttpRequest {
        method: Method::Post,
        url: signed.url,
        headers: signed.headers,
        body: signed.body,
        use_sni: true,
    };
    let started = std::time::Instant::now();
    match runtime.block_on(transport.execute(request)) {
        Ok(response) => {
            let outcome = forum::report_outcome_for_response(response.status, &response.body);
            log::info!(
                "forumReport: provider answered {} in {} ms for a {} byte body ({})",
                response.status,
                started.elapsed().as_millis(),
                bytes,
                report_class(&outcome)
            );
            outcome
        }
        Err(err) => {
            log::warn!(
                "forumReport: transport error ({}) after {} ms",
                transport_class(&err),
                started.elapsed().as_millis()
            );
            ReportOutcome::Failed(FailReason::Transport)
        }
    }
}

fn report_class(outcome: &ReportOutcome) -> &'static str {
    match outcome {
        ReportOutcome::Created { .. } => "created",
        ReportOutcome::SubscriptionRequired => "subscription required",
        ReportOutcome::ClockSkew => "clock skew",
        ReportOutcome::RateLimited => "rate limited",
        ReportOutcome::TooLarge => "too large",
        ReportOutcome::Invalid => "invalid",
        ReportOutcome::ServerError => "server error",
        ReportOutcome::Failed(_) => "failed",
    }
}

/// Collects the redacted problem report into `output_path`.
///
/// `metadata_json` is one JSON object of the platform facts Kotlin read;
/// `redact_json` a JSON array of strings to redact (the wallet address);
/// `app_log_dir` the Kotlin log directory. The Rust log directory is the one
/// `initLogger` writes. Returns `{"ok":true,"bytes":N}` or
/// `{"ok":false,"error":"<class>"}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_warrenbrowse_vpn_jni_WarrenJni_collectProblemReport<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    metadata_json: JString<'local>,
    redact_json: JString<'local>,
    app_log_dir: JString<'local>,
    output_path: JString<'local>,
) -> jstring {
    let jnix_env = JnixEnv::from(env);
    let metadata_json = String::from_java(&jnix_env, metadata_json);
    let redact_json = String::from_java(&jnix_env, redact_json);
    let app_log_dir = std::path::PathBuf::from(String::from_java(&jnix_env, app_log_dir));
    let output_path = std::path::PathBuf::from(String::from_java(&jnix_env, output_path));
    let result = collect(&metadata_json, &redact_json, &app_log_dir, &output_path);
    let json = crate::report::collect_envelope(&result);
    match jnix_env.new_string(json) {
        Ok(s) => s.into_inner() as jstring,
        Err(_) => std::ptr::null_mut(),
    }
}

fn collect(
    metadata_json: &str,
    redact_json: &str,
    app_log_dir: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<u64, String> {
    let metadata = crate::report::parse_metadata(metadata_json)?;
    let redact = crate::report::parse_redact_strings(redact_json);
    let Some(rust_log_dir) = crate::android_jni::rust_log_dir() else {
        return Err("initLogger must run first".to_owned());
    };
    let started = std::time::Instant::now();
    let result = crate::report::collect(metadata, redact, &rust_log_dir, app_log_dir, output_path);
    match &result {
        Ok(bytes) => log::info!(
            "collectProblemReport: {bytes} bytes in {} ms",
            started.elapsed().as_millis()
        ),
        Err(reason) => log::warn!("collectProblemReport: {reason}"),
    }
    result
}
