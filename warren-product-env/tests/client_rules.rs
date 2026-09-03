//! Replays `fixtures/client-rules/product_env.json` against the crate, which
//! is the reference table every other copy of the product anchors (the
//! desktop TypeScript and packaging tables, the Android flavors, the Kotlin
//! and Swift decoders of the FFI table) is pinned to through the same file.

use warren_product_env::{ALL, ProductEnv};

fn fixture() -> serde_json::Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/client-rules/product_env.json"
    );
    let raw = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path}: {err}"));
    serde_json::from_str(&raw).expect("product_env.json parses")
}

fn str_of<'a>(row: &'a serde_json::Value, key: &str) -> &'a str {
    row[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is a string in {row}"))
}

#[test]
fn every_environment_row_is_the_crates_anchor_table() {
    let fixture = fixture();
    let environments = fixture["environments"].as_object().expect("environments");
    assert_eq!(
        environments.len(),
        ALL.len(),
        "the fixture names an environment the crate does not, or misses one"
    );
    for env in ALL {
        let row = &environments[env.name()];
        let columns: [(&str, &str); 10] = [
            ("name", env.name()),
            ("api_url", env.api_url()),
            ("api_host", env.api_host()),
            ("desktop_update_url", env.desktop_update_url()),
            ("display_name", env.display_name()),
            ("unix_product_dir", env.unix_product_dir()),
            ("application_id", env.application_id()),
            ("deep_link_scheme", env.deep_link_scheme()),
            ("connect_host", env.connect_host()),
            ("forum_public_url", env.forum_public_url()),
        ];
        for (column, anchor) in columns {
            assert_eq!(str_of(row, column), anchor, "{}: {column}", env.name());
        }
    }
}

/// The JSON the mobile FFI hands Kotlin and Swift is the fixture row, key for
/// key and value for value: a column added to one side without the other is a
/// decoder reading a field the table does not carry.
#[test]
fn the_anchors_json_of_every_environment_is_its_fixture_row() {
    let fixture = fixture();
    for env in ALL {
        let rendered: serde_json::Value =
            serde_json::from_str(&env.anchors_json()).expect("anchors_json is JSON");
        assert_eq!(
            rendered,
            fixture["environments"][env.name()],
            "{}: the FFI table drifted from the fixture row",
            env.name()
        );
    }
}

#[test]
fn the_prod_row_is_the_unsuffixed_one_and_the_others_carry_their_name() {
    // The per-environment suffixes are what keep two installs apart; a row
    // that lost its suffix would collide with prod on disk and on the wire.
    let fixture = fixture();
    let environments = &fixture["environments"];
    assert_eq!(str_of(&environments["prod"], "deep_link_scheme"), "warren");
    assert_eq!(
        str_of(&environments["prod"], "application_id"),
        "com.warrenbrowse.vpn"
    );
    for env in [ProductEnv::Staging, ProductEnv::Beta] {
        let row = &environments[env.name()];
        assert_eq!(
            str_of(row, "deep_link_scheme"),
            format!("warren-{}", env.name())
        );
        assert_eq!(
            str_of(row, "application_id"),
            format!("com.warrenbrowse.vpn.{}", env.name())
        );
        assert_eq!(
            str_of(row, "unix_product_dir"),
            format!("warren-vpn-{}", env.name())
        );
    }
}
