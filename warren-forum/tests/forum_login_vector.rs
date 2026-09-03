//! Replays the shared golden vector `vectors/forum_login_v1.json` (the
//! warren-vectors submodule): the exact signed request bytes a client sends
//! to the connect broker for a login and for an in-app report, and the
//! outcome each of the broker's pinned answers must class as. The same file
//! is replayed by warren-connect on the other side of the wire, so a
//! mismatch here is a real wire regression, never a reason to touch the
//! vector.

use warren_forum::{
    FailReason, ForumIdentity, ForumLoginOutcome, ReportOutcome, SignedForumRequest,
    build_signed_report_request_with_nonce, build_signed_request_with_nonce, connect_host,
    outcome_for_response, report_outcome_for_response, signed_post_with_nonce,
};
use warren_identity::ed25519_dalek::SigningKey;

const VECTOR_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../vectors/forum_login_v1.json"
);

fn load() -> serde_json::Value {
    let raw = std::fs::read_to_string(VECTOR_PATH).unwrap_or_else(|err| {
        panic!("read {VECTOR_PATH}: {err} (run `git submodule update --init vectors`)")
    });
    let vector: serde_json::Value = serde_json::from_str(&raw).expect("forum_login_v1.json parses");
    assert_eq!(vector["version"], 1, "this suite replays forum_login v1");
    vector
}

fn str_of<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is a string in {value}"))
}

fn signing_key(vector: &serde_json::Value) -> SigningKey {
    let bytes: [u8; 32] = hex::decode(str_of(&vector["signer"], "signing_key_hex"))
        .expect("hex")
        .try_into()
        .expect("32 bytes");
    SigningKey::from_bytes(&bytes)
}

fn nonce(request: &serde_json::Value) -> [u8; 16] {
    hex::decode(str_of(request, "nonce_hex"))
        .expect("hex")
        .try_into()
        .expect("16 bytes")
}

fn header(req: &SignedForumRequest, name: &str) -> String {
    req.headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("built request carries no {name} header"))
}

/// The five headers the vector pins, byte for byte, and nothing else.
fn assert_headers(req: &SignedForumRequest, pinned: &serde_json::Value, name: &str) {
    let pinned = pinned
        .as_object()
        .unwrap_or_else(|| panic!("{name}: headers is an object"));
    for (header_name, value) in pinned {
        assert_eq!(
            header(req, header_name),
            value.as_str().expect("header value"),
            "{name}: header {header_name}"
        );
    }
    assert_eq!(
        req.headers.len(),
        pinned.len(),
        "{name}: the built request carries a header the vector does not pin"
    );
}

#[test]
fn every_pinned_request_is_rebuilt_byte_for_byte() {
    let vector = load();
    let key = signing_key(&vector);
    let timestamp = vector["signer"]["timestamp"]
        .as_u64()
        .expect("signer.timestamp");
    let host = str_of(&vector["signer"], "connect_host");
    let requests = vector["requests"].as_array().expect("requests");
    assert!(!requests.is_empty());

    for request in requests {
        let name = str_of(request, "name");
        let path = str_of(request, "path");
        let body_utf8 = str_of(request, "body_utf8");
        let url = str_of(request, "url");
        let nonce = nonce(request);

        let built = match name {
            "login" => {
                let sid = str_of(request, "sid");
                // The allowlisted host is the one the app signs for; the
                // signature covers the path and the body, never the host,
                // so the headers must be the vector's exactly.
                let allowlisted =
                    build_signed_request_with_nonce(&key, sid, connect_host(), timestamp, nonce)
                        .expect("the vector's sid builds against the allowlisted host");
                assert_eq!(
                    allowlisted.url,
                    format!("https://{}{path}", connect_host()),
                    "{name}: url"
                );
                assert_eq!(allowlisted.body, body_utf8.as_bytes(), "{name}: body");
                assert_headers(&allowlisted, &request["headers"], name);
                // The vector's own synthetic host goes through the raw
                // builder, which is what pins the URL byte for byte.
                signed_post_with_nonce(
                    &key,
                    host,
                    path,
                    body_utf8.as_bytes().to_vec(),
                    timestamp,
                    nonce,
                )
                .expect("the raw builder signs any host")
            }
            "report_with_log" | "report_without_log" => {
                let fields = request["fields"].to_string();
                let log_gz = request
                    .get("log_gz_hex")
                    .and_then(serde_json::Value::as_str)
                    .map(|gz| hex::decode(gz).expect("log_gz_hex"));
                let built = build_signed_report_request_with_nonce(
                    &key,
                    &fields,
                    log_gz.as_deref(),
                    timestamp,
                    nonce,
                )
                .expect("the vector's report builds");
                // The report always goes to the allowlisted host: only the
                // path of the vector's URL is the crate's to reproduce.
                assert_eq!(
                    built.url,
                    format!("https://{}{path}", connect_host()),
                    "{name}: url"
                );
                assert!(
                    url.ends_with(path),
                    "{name}: vector url {url} ends with {path}"
                );
                built
            }
            other => {
                panic!("unknown forum_login_v1 request name {other}: teach this suite to replay it")
            }
        };

        assert_eq!(
            std::str::from_utf8(&built.body).expect("utf8 body"),
            body_utf8,
            "{name}: body bytes"
        );
        assert_headers(&built, &request["headers"], name);
    }
}

#[test]
fn the_login_url_of_the_raw_builder_is_the_vectors() {
    // The other test proves the allowlisted build; this one pins that the raw
    // builder, fed the vector's synthetic host, reproduces the vector's URL
    // exactly, so a URL-shaping change cannot hide behind the host swap.
    let vector = load();
    let request = vector["requests"]
        .as_array()
        .expect("requests")
        .iter()
        .find(|r| r["name"] == "login")
        .expect("login request");
    let built = signed_post_with_nonce(
        &signing_key(&vector),
        str_of(&vector["signer"], "connect_host"),
        str_of(request, "path"),
        str_of(request, "body_utf8").as_bytes().to_vec(),
        vector["signer"]["timestamp"].as_u64().expect("timestamp"),
        nonce(request),
    )
    .expect("builds");
    assert_eq!(built.url, str_of(request, "url"));
}

fn answer<'a>(group: &'a serde_json::Value, name: &str) -> (u16, &'a str) {
    let answer = &group[name];
    let status = u16::try_from(answer["status"].as_u64().expect("status")).expect("u16");
    (status, str_of(answer, "body_utf8"))
}

fn identity(vector: &serde_json::Value) -> ForumIdentity {
    ForumIdentity {
        handle: str_of(&vector["provider"], "handle").to_owned(),
        notify_slot: Some(
            u32::try_from(vector["provider"]["notify_slot"].as_i64().expect("slot")).expect("u32"),
        ),
    }
}

#[test]
fn every_pinned_login_answer_classes_as_its_outcome() {
    let vector = load();
    let group = &vector["responses"]["login"];
    let mut seen = 0;
    for name in group.as_object().expect("login answers").keys() {
        if name.starts_with('_') {
            continue;
        }
        let (status, body) = answer(group, name);
        let expected = match name.as_str() {
            "approved" => ForumLoginOutcome::Approved(Some(identity(&vector))),
            "clock_skew" => ForumLoginOutcome::ClockSkew,
            "subscription_required" => ForumLoginOutcome::SubscriptionRequired,
            "session_unknown" => ForumLoginOutcome::Expired,
            other => {
                panic!("unknown forum_login_v1 login answer {other}: teach this suite its outcome")
            }
        };
        assert_eq!(
            outcome_for_response(status, body.as_bytes()),
            expected,
            "login answer {name}"
        );
        seen += 1;
    }
    assert_eq!(seen, 4, "the v1 login answer set has four members");
}

#[test]
fn every_pinned_report_answer_classes_as_its_outcome() {
    let vector = load();
    let group = &vector["responses"]["report"];
    let topic_id = vector["provider"]["topic_id"].as_u64().expect("topic_id");
    let forum_origin = str_of(&vector["provider"], "forum_public_url");
    let mut seen = 0;
    for name in group.as_object().expect("report answers").keys() {
        if name.starts_with('_') {
            continue;
        }
        let (status, body) = answer(group, name);
        let created = |logs: &str| ReportOutcome::Created {
            topic_id,
            // The synthetic forum origin is not the trusted domain, so the
            // link is dropped: a topic URL only ever becomes a tappable link
            // when it points at the production forum.
            topic_url: None,
            identity: Some(identity(&vector)),
            logs: logs.to_owned(),
        };
        let expected = match name.as_str() {
            "created" => created("attached"),
            "created_without_logs" => created("none"),
            "created_logs_partial" => created("partial"),
            "clock_skew" => ReportOutcome::ClockSkew,
            "subscription_required" => ReportOutcome::SubscriptionRequired,
            "rate_limited" => ReportOutcome::RateLimited,
            "invalid_report" => ReportOutcome::Invalid,
            "payload_too_large" => ReportOutcome::TooLarge,
            "forum_unavailable" | "feature_disabled" => ReportOutcome::ServerError,
            other => {
                panic!("unknown forum_login_v1 report answer {other}: teach this suite its outcome")
            }
        };
        assert_eq!(
            report_outcome_for_response(status, body.as_bytes()),
            expected,
            "report answer {name}"
        );
        if name.starts_with("created") {
            // The same answer with the production forum in place of the
            // synthetic origin keeps its link: what the vector's provider
            // note ("substitutes its own forum origin") means client side.
            let substituted = body.replace(forum_origin, "https://forum.warrenbrowse.com");
            match report_outcome_for_response(status, substituted.as_bytes()) {
                ReportOutcome::Created { topic_url, .. } => assert_eq!(
                    topic_url.as_deref(),
                    Some(format!("https://forum.warrenbrowse.com/t/{topic_id}").as_str()),
                    "report answer {name} with the production origin"
                ),
                other => panic!("{name}: {other:?}"),
            }
        }
        seen += 1;
    }
    assert_eq!(seen, 10, "the v1 report answer set has ten members");
    // The status the outcome table does not name stays a classed failure.
    assert_eq!(
        report_outcome_for_response(418, b""),
        ReportOutcome::Failed(FailReason::Http(418))
    );
}
