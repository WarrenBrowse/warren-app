//! Community-forum wallet login FFI for iOS (`POST /v1/forum/login`, doc 55)
//! and the forum page's attach-logs upload (`POST /v1/forum/attach-logs`).
//!
//! iOS counterpart of Android's `warren-jni` `forumLogin`, `forumAttachLogs`,
//! `forumAttachCancel` and `forumCodeProbe`: every signed request is signed AND
//! POSTed entirely in Rust, so only the opaque `sid`, the connect `host`, the
//! topic and the gzipped report cross the boundary and the wallet signature
//! never surfaces to Swift. The wire bytes + validation + outcome mapping live
//! in the host-tested [`crate::forum`] module; this layer only reads the FFI
//! inputs, executes the requests on the shared iOS runtime through reqwest,
//! and returns a JSON envelope `CString`.
//!
//! Memory ownership: the returned heap `CString` MUST be freed once via
//! `warren_wallet_free_mnemonic` (type-agnostic: it reclaims any `CString` this
//! crate produces). Envelope shapes match Android's, the `login` and `attach`
//! tables of `fixtures/client-rules/forum_outcomes.json`:
//! `{"ok":true,"handle":"..","notify_slot":n}` (both additive) /
//! `{"ok":false,"error":"subscription-required"|"clock-skew"|"expired"}` /
//! `{"ok":false,"error":"error","reason":"<class>"}`; the attach envelope adds
//! `not-author`, `too-large` and `server-error`; the code probe answers
//! `{"kind":"login"|"gone"|"unknown"}` or `{"kind":"attach","topic_id":N}`.
//!
//! Blocking: every entry `block_on`s the shared iOS tokio runtime; the Swift
//! facade invokes them off the main thread.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::Duration;

use warren_api::{HttpRequest, HttpTransport, Method, ReqwestTransport};
use warren_identity::WarrenIdentity;
use zeroize::Zeroizing;

use crate::forum::{
    self, CodeKind, CodePlacement, FailReason, ForumAttachOutcome, ForumLoginOutcome,
    ForumRequestError, SessionPreflight,
};

const SEED_LEN: usize = 32;

/// The connect timeout of every request this module sends, the SDK
/// transport's own.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// The total timeout of an unsigned read (a status, a meta), the SDK
/// transport's own 15 s, so a preflight can never outlast the POST it
/// precedes.
const READ_TIMEOUT: Duration = Duration::from_secs(15);

/// A reqwest client shared by every request of one flow: the preflight and
/// the upload it precedes ride one connection pool. Timeouts are given per
/// request, so the upload's body-sized deadline applies to it alone.
fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .ok()
}

/// One unsigned GET, read back as its status, its `Date` header and its
/// body; `None` when it got no HTTP answer.
async fn get_plain(
    client: &reqwest::Client,
    url: String,
    flow: &str,
    what: &str,
) -> Option<(u16, Option<String>, Vec<u8>)> {
    match client.get(&url).timeout(READ_TIMEOUT).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let date = response
                .headers()
                .get(reqwest::header::DATE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = response
                .bytes()
                .await
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Some((status, date, body))
        }
        Err(err) => {
            let class = if err.is_timeout() {
                "timeout"
            } else {
                "transport"
            };
            log::warn!("{flow}: {what} read failed ({class})");
            None
        }
    }
}

/// Reads a session's status (`url`, the login's or the attach session's)
/// once before signing. The `Date` header of that TLS-authenticated answer is
/// the trusted clock a device that never synchronised its own is corrected
/// against, which turns the 2026-08-18 class (every attempt refused by the
/// broker's 60 s window) into a request that works; a 404 names a dead
/// session before a signature is spent. The classing is the shared crate's,
/// the one Android applies. Any failure of the read itself is `Unknown`: the
/// request is then stamped with the device clock and the provider decides, as
/// before the preflight existed. `flow` names the caller in the log; nothing
/// about the request is logged.
async fn preflight(client: &reqwest::Client, url: Option<String>, flow: &str) -> SessionPreflight {
    let Some(url) = url else {
        return SessionPreflight::Unknown;
    };
    match get_plain(client, url, flow, "status preflight").await {
        Some((status, date, _)) => {
            let verdict = forum::classify_status_preflight(status, date.as_deref(), device_now());
            if let SessionPreflight::Pending { offset_secs } = verdict
                && offset_secs.abs() > 30
            {
                log::warn!(
                    "{flow}: device clock is {offset_secs} s off the connect host, correcting"
                );
            }
            verdict
        }
        None => {
            log::warn!("{flow}: status preflight failed, signing anyway");
            SessionPreflight::Unknown
        }
    }
}

/// The device clock, in Unix seconds, zero before the epoch.
fn device_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Reads a 32-byte seed into a zeroizing buffer. `None` when `seed` is null.
///
/// # Safety
/// `seed`, when non-null, must point to at least `SEED_LEN` readable bytes.
unsafe fn read_seed(seed: *const u8) -> Option<Zeroizing<[u8; SEED_LEN]>> {
    if seed.is_null() {
        return None;
    }
    let mut buf = Zeroizing::new([0u8; SEED_LEN]);
    // SAFETY: `seed` points to at least SEED_LEN readable bytes (precondition).
    unsafe {
        std::ptr::copy_nonoverlapping(seed, buf.as_mut_ptr(), SEED_LEN);
    }
    Some(buf)
}

/// Reads a NUL-terminated C string into an owned `String`, or `None` if null or
/// not valid UTF-8.
///
/// # Safety
/// `p`, when non-null, must be a valid NUL-terminated C string.
unsafe fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a valid NUL-terminated C string (precondition).
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

/// Copies `len` bytes at `bytes` into an owned buffer. `None` for a null
/// pointer with a non-zero length; an empty slice for a zero length.
///
/// # Safety
/// `bytes`, when non-null, must point to at least `len` readable bytes.
unsafe fn bytes_to_vec(bytes: *const u8, len: usize) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if bytes.is_null() {
        return None;
    }
    // SAFETY: `bytes` points to at least `len` readable bytes (precondition).
    Some(unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec())
}

/// Allocates the JSON envelope `CString` for `json`. Every envelope this
/// module hands out is built from fixed tokens, a number and a proquint
/// handle, so it never carries an interior NUL and this never fails in
/// practice.
fn json_cstring(json: String) -> *mut c_char {
    match CString::new(json) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn envelope_cstring(outcome: ForumLoginOutcome) -> *mut c_char {
    json_cstring(forum::envelope(&outcome))
}

fn attach_envelope_cstring(outcome: ForumAttachOutcome) -> *mut c_char {
    json_cstring(forum::attach_envelope(&outcome))
}

/// Sign and submit a forum-login challenge for `sid` to the connect `host`.
///
/// Derives the `WarrenIdentity` from the 32-byte wallet `seed`, reads the
/// session's status once (a dead session is `expired` without a signature
/// spent; the answer's `Date` corrects the device clock), builds the signed
/// `POST /v1/forum/login` request at the corrected time (host allowlist + sid
/// shape checked in `crate::forum`), sends it, and returns the outcome
/// envelope: `{"ok":true,...}` with the forum identity the broker handed back,
/// `subscription-required` on 403, `clock-skew` on connect's 401 token,
/// `expired` on 404, `error` with a `reason` class for anything else (input,
/// build, runtime, transport, an unnamed status). Nothing about the request
/// (seed, sid, signature, nonce) is ever logged.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes; `sid` and
/// `host` must be valid NUL-terminated C strings. The returned pointer must be
/// freed exactly once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_login(
    seed: *const u8,
    sid: *const c_char,
    host: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(seed), Some(sid), Some(host)) = (
            unsafe { read_seed(seed) },
            unsafe { cstr_to_string(sid) },
            unsafe { cstr_to_string(host) },
        ) else {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        };

        if !warren_forum::is_allowed_connect_host(&host) || !warren_forum::is_valid_sid(&sid) {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        }
        let handle = match crate::warren_ios_runtime() {
            Ok(handle) => handle,
            Err(_) => return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Runtime)),
        };
        let Some(client) = client() else {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        };
        let offset_secs = match handle.block_on(preflight(
            &client,
            forum::build_status_url(&sid, &host),
            "forumLogin",
        )) {
            SessionPreflight::Pending { offset_secs } => offset_secs,
            SessionPreflight::Gone => return envelope_cstring(ForumLoginOutcome::Expired),
            SessionPreflight::Unknown => 0,
        };
        let Some(timestamp) = forum::timestamp_with_offset(offset_secs) else {
            return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build));
        };
        let identity = WarrenIdentity::from_seed(&seed);
        let signed = match forum::build_signed_request_at(&identity, &sid, &host, timestamp) {
            Ok(req) => req,
            Err(_) => return envelope_cstring(ForumLoginOutcome::Failed(FailReason::Build)),
        };

        let request = HttpRequest {
            method: Method::Post,
            url: signed.url,
            headers: signed.headers,
            body: signed.body,
            use_sni: true,
        };
        let outcome = match handle.block_on(ReqwestTransport::new().execute(request)) {
            Ok(response) => forum::outcome_for_response(response.status, &response.body),
            Err(_) => ForumLoginOutcome::Failed(FailReason::Transport),
        };
        envelope_cstring(outcome)
    })
}

/// An unsigned, bodyless POST whose answer nobody waits for: the two cancel
/// notifications. A failed one just means the browser page polls to its
/// timeout.
fn post_best_effort(url: Option<String>) {
    let Some(url) = url else {
        return;
    };
    let Ok(handle) = crate::warren_ios_runtime() else {
        return;
    };
    let request = HttpRequest {
        method: Method::Post,
        url,
        headers: Vec::new(),
        body: Vec::new(),
        use_sni: true,
    };
    let _ = handle.block_on(ReqwestTransport::new().execute(request));
}

/// Best-effort: notify the connect `host` that the user declined the forum login
/// for `sid` (`POST /v1/session/<sid>/cancel`), so the waiting browser page
/// unblocks instead of polling to timeout. Unsigned (no seed / wallet material);
/// mirrors the desktop `cancelForumLogin`. Failures are ignored (connect drops
/// a login session on its own after 5 minutes, the `pending_ttl_secs.login` of
/// `fixtures/client-rules/forum_link.json`). Blocking; call off the main thread.
///
/// # Safety
/// `sid` and `host` must be valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_cancel(sid: *const c_char, host: *const c_char) {
    crate::ffi_guard((), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(sid), Some(host)) = (unsafe { cstr_to_string(sid) }, unsafe {
            cstr_to_string(host)
        }) else {
            return;
        };
        post_best_effort(forum::build_cancel_url(&sid, &host));
    })
}

/// Sign and submit the attach-logs upload (`POST /v1/forum/attach-logs`) for
/// the forum's "attach your logs" page: `sid` and `host` from the deep link
/// (or the typed code), `topic_id` the topic the logs join (0 for a pre-topic
/// session, where the report is still being composed), `log_gz` the gzipped
/// redacted problem report of `log_gz_len` bytes. The mirror of Android's
/// `forumAttachLogs` and of the desktop `approveForumAttach` plus the daemon's
/// signer: refuse what costs no round trip (the shared gate), preflight the
/// attach session (clock offset, dead session), sign the body at the
/// corrected time, send it under the body-sized upload deadline, and class
/// the answer. Returns the envelope of [`warren_forum::attach_envelope`]:
/// `{"ok":true}` when attached or parked, `{"ok":false,"error":"not-author"}`
/// (403), `expired` (404, which also covers a topic the session is not bound
/// to), `too-large` (over the cap here, before any byte leaves, or 413),
/// `clock-skew`, `server-error` (5xx), or `error` with a `reason` class
/// (`build`, `runtime`, `transport`, `upload-timeout`, `http-<status>`). The
/// seed, sid, signature and report are never logged.
///
/// # Safety
/// `seed`, when non-null, must point to at least 32 readable bytes; `sid` and
/// `host` must be valid NUL-terminated C strings; `log_gz`, when non-null,
/// must point to at least `log_gz_len` readable bytes. The returned pointer
/// must be freed exactly once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_attach_logs(
    seed: *const u8,
    sid: *const c_char,
    topic_id: u64,
    host: *const c_char,
    log_gz: *const u8,
    log_gz_len: usize,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(seed), Some(sid), Some(host), Some(log_gz)) = (
            unsafe { read_seed(seed) },
            unsafe { cstr_to_string(sid) },
            unsafe { cstr_to_string(host) },
            unsafe { bytes_to_vec(log_gz, log_gz_len) },
        ) else {
            return attach_envelope_cstring(ForumAttachOutcome::Failed(FailReason::Build));
        };
        attach_envelope_cstring(forum_attach_logs(&seed, &sid, &host, topic_id, &log_gz))
    })
}

fn forum_attach_logs(
    seed: &[u8; SEED_LEN],
    sid: &str,
    host: &str,
    topic_id: u64,
    log_gz: &[u8],
) -> ForumAttachOutcome {
    if let Some(refused) = forum::refuse_before_transport(sid, host, log_gz) {
        log::warn!(
            "forumAttachLogs: refused before transport ({})",
            attach_class(&refused)
        );
        return refused;
    }
    let Ok(handle) = crate::warren_ios_runtime() else {
        return ForumAttachOutcome::Failed(FailReason::Runtime);
    };
    let Some(client) = client() else {
        return ForumAttachOutcome::Failed(FailReason::Build);
    };
    let offset_secs = match handle.block_on(preflight(
        &client,
        forum::build_attach_status_url(sid, host),
        "forumAttachLogs",
    )) {
        SessionPreflight::Pending { offset_secs } => offset_secs,
        SessionPreflight::Gone => {
            log::info!("forumAttachLogs: session already gone before signing");
            return ForumAttachOutcome::Expired;
        }
        SessionPreflight::Unknown => 0,
    };
    let Some(timestamp) = forum::timestamp_with_offset(offset_secs) else {
        log::warn!("forumAttachLogs: could not stamp the request");
        return ForumAttachOutcome::Failed(FailReason::Build);
    };
    let identity = WarrenIdentity::from_seed(seed);
    let signed =
        match forum::build_signed_attach_request(&identity, sid, host, topic_id, log_gz, timestamp)
        {
            Ok(req) => req,
            Err(ForumRequestError::LogTooLarge) => return ForumAttachOutcome::TooLarge,
            Err(_) => {
                log::warn!("forumAttachLogs: could not build signed request");
                return ForumAttachOutcome::Failed(FailReason::Build);
            }
        };
    let bytes = signed.body.len();
    // The upload rides the flow's client under its own body-sized deadline:
    // the SDK transport's 15 s is sized for a few hundred bytes, and a report
    // with a few MiB of logs on a mobile uplink died in it after the data was
    // spent.
    let deadline = forum::upload_deadline(bytes);
    let mut request = client.post(&signed.url).timeout(deadline);
    for (name, value) in &signed.headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let started = std::time::Instant::now();
    match handle.block_on(async {
        let response = request.body(signed.body).send().await?;
        let status = response.status().as_u16();
        let body = response.bytes().await?;
        Ok::<(u16, Vec<u8>), reqwest::Error>((status, body.to_vec()))
    }) {
        Ok((status, body)) => {
            let outcome = forum::attach_outcome_for_response(status, &body);
            log::info!(
                "forumAttachLogs: provider answered {status} in {} ms for a {bytes} byte body ({})",
                started.elapsed().as_millis(),
                attach_class(&outcome)
            );
            outcome
        }
        Err(err) => {
            let class = if err.is_timeout() {
                "timeout"
            } else {
                "transport"
            };
            log::warn!(
                "forumAttachLogs: transport error ({class}) after {} ms of a {} s deadline for a {bytes} byte body",
                started.elapsed().as_millis(),
                deadline.as_secs()
            );
            if err.is_timeout() {
                ForumAttachOutcome::Failed(FailReason::UploadTimeout)
            } else {
                ForumAttachOutcome::Failed(FailReason::Transport)
            }
        }
    }
}

fn attach_class(outcome: &ForumAttachOutcome) -> &'static str {
    match outcome {
        ForumAttachOutcome::Attached => "attached",
        ForumAttachOutcome::NotAuthor => "not author",
        ForumAttachOutcome::Expired => "expired",
        ForumAttachOutcome::TooLarge => "too large",
        ForumAttachOutcome::ClockSkew => "clock skew",
        ForumAttachOutcome::ServerError => "server error",
        ForumAttachOutcome::Failed(_) => "failed",
    }
}

/// Best-effort: tell the connect provider the user declined the attach
/// (`POST /v1/attach/<sid>/cancel`) so the waiting forum page shows
/// "cancelled" instead of polling to its timeout. Unsigned; mirrors the
/// desktop `cancelForumAttach`. Failures are ignored: the session expires on
/// its own in 30 minutes (`pending_ttl_secs.attach`). Blocking; call off the
/// main thread.
///
/// # Safety
/// `sid` and `host` must be valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_attach_cancel(sid: *const c_char, host: *const c_char) {
    crate::ffi_guard((), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(sid), Some(host)) = (unsafe { cstr_to_string(sid) }, unsafe {
            cstr_to_string(host)
        }) else {
            return;
        };
        post_best_effort(forum::build_attach_cancel_url(&sid, &host));
    })
}

/// Places a session id typed by hand before any consent is raised: the login
/// status read first, the attach status read only when the login one answers
/// 404, and the attach meta only for a pending attach session, because it
/// names the topic a code typed by hand cannot carry. Returns
/// `{"kind":"login"|"gone"|"unknown"}` or `{"kind":"attach","topic_id":N}`
/// with 0 for a pre-topic session ([`warren_forum::code_placement_envelope`]).
/// Unsigned, no wallet material; blocks on up to three GETs, so invoke off
/// the main thread. The sid is never logged.
///
/// # Safety
/// `sid` and `host` must be valid NUL-terminated C strings. The returned
/// pointer must be freed exactly once via `warren_wallet_free_mnemonic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn warren_forum_code_probe(
    sid: *const c_char,
    host: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        // SAFETY: the inputs uphold the documented preconditions.
        let (Some(sid), Some(host)) = (unsafe { cstr_to_string(sid) }, unsafe {
            cstr_to_string(host)
        }) else {
            return json_cstring(forum::code_placement_envelope(CodePlacement::Unknown));
        };
        json_cstring(forum::code_placement_envelope(forum_code_probe(
            &sid, &host,
        )))
    })
}

fn forum_code_probe(sid: &str, host: &str) -> CodePlacement {
    let (Some(login_url), Some(attach_url), Some(meta_url)) = (
        forum::build_status_url(sid, host),
        forum::build_attach_status_url(sid, host),
        forum::build_attach_meta_url(sid, host),
    ) else {
        log::warn!("forumCodeProbe: refused a code outside the allowlist or sid shape");
        return CodePlacement::Unknown;
    };
    let Ok(handle) = crate::warren_ios_runtime() else {
        return CodePlacement::Unknown;
    };
    let Some(client) = client() else {
        return CodePlacement::Unknown;
    };
    let read = |url: String, what: &str| {
        handle
            .block_on(get_plain(&client, url, "forumCodeProbe", what))
            .map(|(status, _, body)| (status, body))
    };
    let login_status = read(login_url, "login status").map(|(status, _)| status);
    let attach = if login_status == Some(404) {
        read(attach_url, "attach status")
    } else {
        None
    };
    // The meta is read only for a pending attach session: it names the topic
    // a code typed by hand cannot carry, and nothing else needs it.
    let pending = matches!(
        forum::classify_code_probe(
            login_status,
            attach
                .as_ref()
                .map(|(status, body)| (*status, body.as_slice()))
        ),
        CodeKind::Attach
    );
    let meta = if pending {
        read(meta_url, "attach meta")
    } else {
        None
    };
    let placement = forum::place_code(
        login_status,
        attach
            .as_ref()
            .map(|(status, body)| (*status, body.as_slice())),
        meta.as_ref()
            .map(|(status, body)| (*status, body.as_slice())),
    );
    log::info!(
        "forumCodeProbe: login status {:?}, attach status {:?}, meta status {:?}: {:?}",
        login_status,
        attach.as_ref().map(|(status, _)| *status),
        meta.as_ref().map(|(status, _)| *status),
        placement
    );
    placement
}
