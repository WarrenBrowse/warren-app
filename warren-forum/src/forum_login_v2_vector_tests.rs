//! Replays the shared golden vector `vectors/forum_login_v2.json` (the
//! warren-vectors submodule): the bound login approval the app signs, and the
//! outcome, completion code and handoff URL each of the broker's pinned
//! answers must class as. warren-connect replays the same file on the other
//! side of the wire, so a mismatch here is a wire regression, never a reason
//! to touch the vector.
//!
//! In the crate for the reason `forum_login_vector_tests` is: the vector names
//! a synthetic connect host, and the builders and parsers that accept it are
//! crate-private so no FFI flow can sign for, or trust a handoff on, a host
//! the allowlist refuses.

use super::{
    FailReason, ForumIdentity, ForumLoginOutcome, LoginCompletion, SessionPreflight,
    build_signed_request_with_nonce, classify_status_preflight, connect_host, outcome_for_response,
    outcome_for_response_for, parse_login_completion, parse_login_completion_for,
    signed_post_with_nonce,
};
use warren_identity::ed25519_dalek::SigningKey;

const VECTOR_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../vectors/forum_login_v2.json"
);

/// The v1 corpus still pins the approved answer of a provider that predates
/// the bound approval: the legacy completion a v2 client must keep following.
const V1_VECTOR_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../vectors/forum_login_v1.json"
);

fn read(path: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("read {path}: {err} (run `git submodule update --init vectors`)")
    });
    serde_json::from_str(&raw).unwrap_or_else(|err| panic!("{path} parses: {err}"))
}

fn load() -> serde_json::Value {
    let vector = read(VECTOR_PATH);
    assert_eq!(vector["version"], 2, "this suite replays forum_login v2");
    vector
}

fn str_of<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is a string in {value}"))
}

fn vector_host(vector: &serde_json::Value) -> &str {
    str_of(&vector["signer"], "connect_host")
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
            u32::try_from(vector["provider"]["notify_slot"].as_u64().expect("slot")).expect("u32"),
        ),
    }
}

fn completion_of(outcome: ForumLoginOutcome, name: &str) -> LoginCompletion {
    match outcome {
        ForumLoginOutcome::Approved {
            completion: Some(completion),
            ..
        } => completion,
        other => panic!("{name}: a bound approval carries a completion, got {other:?}"),
    }
}

#[test]
fn the_bound_login_is_rebuilt_byte_for_byte() {
    let vector = load();
    let bytes: [u8; 32] = hex::decode(str_of(&vector["signer"], "signing_key_hex"))
        .expect("hex")
        .try_into()
        .expect("32 bytes");
    let key = SigningKey::from_bytes(&bytes);
    let timestamp = vector["signer"]["timestamp"].as_u64().expect("timestamp");

    let requests = vector["requests"].as_array().expect("requests");
    assert!(!requests.is_empty());
    for request in requests {
        let name = str_of(request, "name");
        assert_eq!(
            name, "login_bound",
            "unknown forum_login_v2 request name {name}: teach this suite to replay it"
        );
        let nonce: [u8; 16] = hex::decode(str_of(request, "nonce_hex"))
            .expect("hex")
            .try_into()
            .expect("16 bytes");
        let path = str_of(request, "path");

        let built = build_signed_request_with_nonce(
            &key,
            str_of(request, "sid"),
            connect_host(),
            timestamp,
            nonce,
        )
        .expect("the vector's sid builds against the allowlisted host");
        assert_eq!(
            std::str::from_utf8(&built.body).expect("utf8"),
            str_of(request, "body_utf8"),
            "{name}: body bytes"
        );
        assert_eq!(built.url, format!("https://{}{path}", connect_host()));
        let pinned = request["headers"].as_object().expect("headers");
        for (header, value) in pinned {
            let got = built
                .headers
                .iter()
                .find(|(n, _)| n == header)
                .map(|(_, v)| v.as_str());
            assert_eq!(got, value.as_str(), "{name}: header {header}");
        }
        assert_eq!(built.headers.len(), pinned.len(), "{name}: header set");

        let raw = signed_post_with_nonce(
            &key,
            vector_host(&vector),
            path,
            built.body.clone(),
            timestamp,
            nonce,
        )
        .expect("the raw builder signs any host");
        assert_eq!(raw.url, str_of(request, "url"), "{name}: url");
    }
}

#[test]
fn every_pinned_login_answer_classes_as_its_outcome() {
    let vector = load();
    let host = vector_host(&vector);
    let code = str_of(&vector["provider"], "completion_code");
    let sid = str_of(&vector["requests"][0], "sid");
    let group = &vector["responses"]["login"];
    let mut seen = 0;
    for name in group.as_object().expect("login answers").keys() {
        if name.starts_with('_') {
            continue;
        }
        let (status, body) = answer(group, name);
        let outcome = outcome_for_response_for(status, body.as_bytes(), host);
        match name.as_str() {
            "approved_same_device" | "approved_cross_device" => {
                let ForumLoginOutcome::Approved {
                    identity: got_identity,
                    completion,
                } = outcome
                else {
                    panic!("{name}: {outcome:?}");
                };
                assert_eq!(got_identity, Some(identity(&vector)), "{name}: identity");
                let completion = completion.unwrap_or_else(|| panic!("{name}: completion"));
                assert_eq!(completion.code(), code, "{name}: code");
                let expected_handoff = (name == "approved_same_device")
                    .then(|| format!("https://{host}/handoff#sid={sid}&code={code}"));
                assert_eq!(
                    completion.handoff_url(),
                    expected_handoff.as_deref(),
                    "{name}: handoff"
                );
            }
            "app_update_required" | "login_version_unsupported" => assert_eq!(
                outcome,
                ForumLoginOutcome::Failed(FailReason::Http(400)),
                "{name}"
            ),
            "clock_skew" => assert_eq!(outcome, ForumLoginOutcome::ClockSkew, "{name}"),
            "subscription_required" => {
                assert_eq!(outcome, ForumLoginOutcome::SubscriptionRequired, "{name}");
            }
            "session_unknown" => assert_eq!(outcome, ForumLoginOutcome::Expired, "{name}"),
            other => {
                panic!("unknown forum_login_v2 login answer {other}: teach this suite its outcome")
            }
        }
        seen += 1;
    }
    assert_eq!(seen, 7, "the v2 login answer set has seven members");
}

#[test]
fn the_same_device_answer_hands_off_only_on_the_allowlisted_host() {
    // The vector was produced on a synthetic host. Through the production
    // parser that host is a foreign one: the code stands and the handoff is
    // dropped. With the production connect host in its place, the handoff
    // stands, which is what a live provider's answer looks like.
    let vector = load();
    let host = vector_host(&vector);
    let (status, body) = answer(&vector["responses"]["login"], "approved_same_device");
    let code = str_of(&vector["provider"], "completion_code");

    let foreign = completion_of(outcome_for_response(status, body.as_bytes()), "foreign");
    assert_eq!(foreign.code(), code);
    assert_eq!(foreign.handoff_url(), None);

    let live = body.replace(host, connect_host());
    let completion = completion_of(outcome_for_response(status, live.as_bytes()), "live");
    let example = str_of(&vector["handoff"], "example").replace(host, connect_host());
    assert_eq!(completion.handoff_url(), Some(example.as_str()));
}

#[test]
fn a_handoff_in_a_query_string_or_naming_another_code_is_dropped() {
    let vector = load();
    let host = vector_host(&vector);
    let (_, body) = answer(&vector["responses"]["login"], "approved_same_device");
    let code = str_of(&vector["provider"], "completion_code");
    let example = str_of(&vector["handoff"], "example");

    let query = body.replace(example, &example.replacen('#', "?", 1));
    let other_code = body.replace(example, &example.replace(code, "999999"));
    for (what, tampered) in [("query string", query), ("another code", other_code)] {
        assert_ne!(tampered, body, "{what}: the substitution applied");
        let completion = parse_login_completion_for(tampered.as_bytes(), host)
            .unwrap_or_else(|| panic!("{what}: the code stands"));
        assert_eq!(completion.code(), code, "{what}");
        assert_eq!(completion.handoff_url(), None, "{what}");
    }
}

#[test]
fn the_answer_of_a_provider_that_predates_the_bound_approval_has_no_completion() {
    let v1 = read(V1_VECTOR_PATH);
    let (status, body) = answer(&v1["responses"]["login"], "approved");
    assert_eq!(parse_login_completion(body.as_bytes()), None);
    match outcome_for_response(status, body.as_bytes()) {
        ForumLoginOutcome::Approved {
            identity: Some(_),
            completion: None,
        } => {}
        other => panic!("the legacy answer approves with its identity alone: {other:?}"),
    }
}

#[test]
fn every_pinned_app_status_answer_preflights_as_the_app_expects() {
    // The app's unsigned read before it signs, answered without the login
    // cookie on either id.
    let vector = load();
    let group = &vector["responses"]["session_status_app"];
    let mut seen = 0;
    for name in group
        .as_object()
        .expect("session_status_app answers")
        .keys()
    {
        if name.starts_with('_') {
            continue;
        }
        let (status, _) = answer(group, name);
        let preflight = classify_status_preflight(status, None, 0);
        match name.as_str() {
            "pending" => assert_eq!(preflight, SessionPreflight::Pending { offset_secs: 0 }),
            "gone" => assert_eq!(preflight, SessionPreflight::Gone),
            other => panic!(
                "unknown forum_login_v2 session_status_app answer {other}: teach this suite its preflight"
            ),
        }
        seen += 1;
    }
    assert_eq!(seen, 2);
}

#[test]
fn the_answer_groups_are_the_ones_this_suite_knows() {
    // The app reads `login` and `session_status_app`, both replayed above,
    // and fires `cancel` without reading its answer. The other groups are the
    // provider's own pages talking to the browser that holds the login cookie,
    // which no app ever does. A group added to the vector must be placed on
    // one side or the other.
    let vector = load();
    let mut groups: Vec<&str> = vector["responses"]
        .as_object()
        .expect("responses")
        .keys()
        .map(String::as_str)
        .filter(|name| !name.starts_with('_'))
        .collect();
    groups.sort_unstable();
    assert_eq!(
        groups,
        [
            "cancel",
            "complete",
            "confirm",
            "login",
            "session_status",
            "session_status_app"
        ]
    );
}
