//! Replays the app-level client-rule fixtures under `fixtures/client-rules/`
//! (schema: `fixtures/client-rules/README.md`) against the shared crate,
//! which is the reference the Kotlin, TypeScript and Swift mirrors are pinned
//! to. The Rust side owns the sid shape, the host allowlist, the sign-in code
//! rule, the outcome tables and the FFI envelopes; the URL-level classes of a
//! deep link (scheme, action, missing parameters) have no Rust parser yet and
//! are replayed by the platform mirrors only.

use warren_forum::{
    FailReason, ForumIdentity, ForumLoginOutcome, ReportOutcome, build_cancel_url,
    build_status_url, connect_host, envelope, is_allowed_connect_host, is_valid_sid,
    normalize_sign_in_code, outcome_for_response, report_envelope, report_outcome_for_response,
};

fn fixture(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/../fixtures/client-rules/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
    serde_json::from_str(&raw).unwrap_or_else(|err| panic!("{name} parses: {err}"))
}

fn str_of<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is a string in {value}"))
}

fn skipped_for_rust(case: &serde_json::Value) -> bool {
    case["skip"]
        .as_array()
        .is_some_and(|skip| skip.iter().any(|p| p == "rust"))
}

/// The query parameters of a deep link, read the way a platform mirror reads
/// them; the fixture URLs are plain enough that no URL crate is needed here.
fn query_params(url: &str) -> Vec<(String, String)> {
    url.split_once('?')
        .map(|(_, query)| query)
        .unwrap_or("")
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

fn param<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn the_allowlist_and_the_schemes_of_the_link_fixture_are_the_crates() {
    let link = fixture("forum_link.json");
    let hosts = link["allowed_hosts"].as_array().expect("allowed_hosts");
    assert!(!hosts.is_empty());
    for host in hosts {
        assert!(is_allowed_connect_host(host.as_str().expect("host")));
    }
    assert!(hosts.iter().any(|h| h == connect_host()));

    // The scheme table is the product-env fixture's, spelled once more here
    // so a link case can be read on its own.
    let env = fixture("product_env.json");
    for (name, scheme) in link["schemes"].as_object().expect("schemes") {
        assert_eq!(
            &env["environments"][name]["deep_link_scheme"], scheme,
            "scheme of {name} drifted between the two fixtures"
        );
    }
}

#[test]
fn the_login_link_cases_agree_with_the_sid_and_host_rules() {
    let link = fixture("forum_link.json");
    let mut replayed = 0;
    for case in link["login_cases"].as_array().expect("login_cases") {
        if skipped_for_rust(case) {
            continue;
        }
        let name = str_of(case, "name");
        let Some(url) = case["url"].as_str() else {
            continue;
        };
        let params = query_params(url);
        let expect = &case["expect"];
        if let Some(accepted) = expect.get("accepted") {
            let sid = str_of(accepted, "sid");
            let host = str_of(accepted, "host");
            assert_eq!(param(&params, "sid"), Some(sid), "{name}: sid");
            assert_eq!(param(&params, "host"), Some(host), "{name}: host");
            assert!(is_valid_sid(sid), "{name}: sid shape");
            assert!(is_allowed_connect_host(host), "{name}: host allowlist");
            assert_eq!(
                build_status_url(sid, host).as_deref(),
                Some(format!("https://{host}/v1/session/{sid}/status").as_str()),
                "{name}: status url"
            );
            assert_eq!(
                build_cancel_url(sid, host).as_deref(),
                Some(format!("https://{host}/v1/session/{sid}/cancel").as_str()),
                "{name}: cancel url"
            );
            replayed += 1;
            continue;
        }
        match str_of(expect, "rejected") {
            "bad-sid-shape" => {
                let sid = param(&params, "sid").expect("a bad-sid case carries a sid");
                assert!(!is_valid_sid(sid), "{name}: the sid must be refused");
                assert_eq!(build_status_url(sid, connect_host()), None, "{name}");
                replayed += 1;
            }
            "host-not-allowlisted" => {
                let host = param(&params, "host").expect("a host case carries a host");
                assert!(
                    !is_allowed_connect_host(host),
                    "{name}: the host must be refused"
                );
                let sid = param(&params, "sid").expect("sid");
                assert_eq!(build_cancel_url(sid, host), None, "{name}");
                replayed += 1;
            }
            // The URL-level classes are the platform mirrors' to replay.
            _ => {}
        }
    }
    assert!(
        replayed >= 8,
        "only {replayed} link cases reached the crate's rules"
    );
}

#[test]
fn the_sign_in_code_cases_replay_through_the_normaliser() {
    let link = fixture("forum_link.json");
    let cases = link["sign_in_code_cases"].as_array().expect("cases");
    assert!(!cases.is_empty());
    for case in cases {
        if skipped_for_rust(case) {
            continue;
        }
        let name = str_of(case, "name");
        let typed = str_of(case, "typed");
        let expect = case["expect"].as_str().map(str::to_owned);
        assert_eq!(normalize_sign_in_code(typed), expect, "{name}");
    }
}

#[test]
fn the_connect_host_and_the_forum_origin_of_every_environment_are_the_crates() {
    let env = fixture("product_env.json");
    let environments = env["environments"].as_object().expect("environments");
    assert_eq!(environments.len(), 3);
    for (name, row) in environments {
        // The crate reads the compiled environment's row, and every row names
        // the same broker today.
        assert_eq!(str_of(row, "connect_host"), connect_host(), "{name}");
        // A topic URL under the forum origin keeps its link, which is the
        // crate's whole knowledge of the forum host.
        let origin = str_of(row, "forum_public_url");
        let body = format!(
            r#"{{"status":"created","topic_id":1,"topic_url":"{origin}/t/1","logs":"none"}}"#
        );
        match report_outcome_for_response(201, body.as_bytes()) {
            ReportOutcome::Created { topic_url, .. } => assert_eq!(
                topic_url.as_deref(),
                Some(format!("{origin}/t/1").as_str()),
                "{name}: the forum origin is trusted"
            ),
            other => panic!("{name}: {other:?}"),
        }
    }
}

fn expected_identity(expect: &serde_json::Value) -> Option<ForumIdentity> {
    let handle = expect.get("handle")?.as_str()?;
    Some(ForumIdentity {
        handle: handle.to_owned(),
        notify_slot: expect
            .get("notify_slot")
            .and_then(serde_json::Value::as_u64)
            .map(|n| u32::try_from(n).expect("slot")),
    })
}

fn fail_reason(expect: &serde_json::Value) -> FailReason {
    let reason = str_of(expect, "reason");
    let status = reason
        .strip_prefix("http-")
        .unwrap_or_else(|| {
            panic!("an HTTP-born failure carries an http-<status> reason, got {reason}")
        })
        .parse()
        .expect("status");
    FailReason::Http(status)
}

fn status_and_body(case: &serde_json::Value) -> (u16, &str) {
    (
        u16::try_from(case["status"].as_u64().expect("status")).expect("u16"),
        str_of(case, "body"),
    )
}

#[test]
fn every_login_case_classes_and_envelopes_as_the_fixture_says() {
    let outcomes = fixture("forum_outcomes.json");
    let cases = outcomes["login"]["cases"].as_array().expect("login cases");
    assert!(cases.len() >= 10);
    for case in cases {
        if skipped_for_rust(case) {
            continue;
        }
        let name = str_of(case, "name");
        let (status, body) = status_and_body(case);
        let expect = &case["expect"];
        let expected = match str_of(expect, "kind") {
            "approved" => ForumLoginOutcome::Approved(expected_identity(expect)),
            "subscription-required" => ForumLoginOutcome::SubscriptionRequired,
            "clock-skew" => ForumLoginOutcome::ClockSkew,
            "expired" => ForumLoginOutcome::Expired,
            "failed" => ForumLoginOutcome::Failed(fail_reason(expect)),
            other => panic!("{name}: unknown login kind {other}"),
        };
        let outcome = outcome_for_response(status, body.as_bytes());
        assert_eq!(outcome, expected, "{name}: outcome");
        assert_eq!(
            envelope(&outcome),
            str_of(case, "envelope"),
            "{name}: envelope"
        );
    }
}

fn client_side_reason(token: &str) -> FailReason {
    match token {
        "transport" => FailReason::Transport,
        "runtime" => FailReason::Runtime,
        "build" => FailReason::Build,
        "upload-timeout" => FailReason::UploadTimeout,
        other => panic!("unknown client-side failure reason {other}"),
    }
}

#[test]
fn every_client_side_failure_envelopes_as_the_fixture_says() {
    let outcomes = fixture("forum_outcomes.json");
    let login = outcomes["login"]["client_side_failures"]["cases"]
        .as_array()
        .expect("login client-side cases");
    assert!(!login.is_empty());
    for case in login {
        let reason = client_side_reason(str_of(case, "reason"));
        assert_eq!(
            envelope(&ForumLoginOutcome::Failed(reason)),
            str_of(case, "envelope"),
            "login {}",
            str_of(case, "name")
        );
    }
    let report = outcomes["report"]["client_side_failures"]["cases"]
        .as_array()
        .expect("report client-side cases");
    assert!(!report.is_empty());
    for case in report {
        let reason = client_side_reason(str_of(case, "reason"));
        assert_eq!(
            report_envelope(&ReportOutcome::Failed(reason)),
            str_of(case, "envelope"),
            "report {}",
            str_of(case, "name")
        );
    }
}

#[test]
fn every_report_case_classes_and_envelopes_as_the_fixture_says() {
    let outcomes = fixture("forum_outcomes.json");
    let cases = outcomes["report"]["cases"]
        .as_array()
        .expect("report cases");
    assert!(cases.len() >= 15);
    for case in cases {
        if skipped_for_rust(case) {
            continue;
        }
        let name = str_of(case, "name");
        let (status, body) = status_and_body(case);
        let expect = &case["expect"];
        let expected = match str_of(expect, "kind") {
            "created" => ReportOutcome::Created {
                topic_id: expect["topic_id"].as_u64().expect("topic_id"),
                topic_url: expect["topic_url"].as_str().map(str::to_owned),
                identity: expected_identity(expect),
                logs: str_of(expect, "logs").to_owned(),
            },
            "subscription-required" => ReportOutcome::SubscriptionRequired,
            "clock-skew" => ReportOutcome::ClockSkew,
            "rate-limited" => ReportOutcome::RateLimited,
            "too-large" => ReportOutcome::TooLarge,
            "invalid" => ReportOutcome::Invalid,
            "server-error" => ReportOutcome::ServerError,
            "failed" => ReportOutcome::Failed(fail_reason(expect)),
            other => panic!("{name}: unknown report kind {other}"),
        };
        let outcome = report_outcome_for_response(status, body.as_bytes());
        assert_eq!(outcome, expected, "{name}: outcome");
        assert_eq!(
            report_envelope(&outcome),
            str_of(case, "envelope"),
            "{name}: envelope"
        );
    }
}
