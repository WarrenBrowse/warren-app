//! The per-environment product anchors are spelled again wherever a build
//! tool needs them before any Rust runs: the desktop TypeScript table
//! (`src/shared/constants/product-env.ts`), the Electron packaging config
//! (`tasks/distribution.cjs`) and the Android flavors
//! (`android/app/build.gradle.kts`). Each copy is read here as text and held
//! to this crate, the reference, so a value edited in one place fails the
//! suite that compiles the reference. The vitest and JVM readers of
//! `fixtures/client-rules/product_env.json` pin the same copies from their
//! own side; this suite runs in `warren-checks.yml`, the workflow that has
//! cargo, which is why it is not a desktop unit test.

use regex::Regex;
use warren_product_env::{ALL, ProductEnv};

const PRODUCT_ENV_TS: &str = "desktop/packages/mullvad-vpn/src/shared/constants/product-env.ts";
const DISTRIBUTION_CJS: &str = "desktop/packages/mullvad-vpn/tasks/distribution.cjs";
const BUILD_GRADLE: &str = "android/app/build.gradle.kts";

fn repo_file(relative: &str) -> String {
    let path = format!("{}/../{relative}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

/// The body of the `<env>: { ... }` entry of a JavaScript object literal
/// whose entries sit at two spaces of indentation and close with `  }`.
fn js_env_block<'a>(source: &'a str, env: ProductEnv, file: &str) -> &'a str {
    let opener = format!("\n  {}: {{\n", env.name());
    let start = source
        .find(&opener)
        .unwrap_or_else(|| panic!("no `{}` entry in {file}", env.name()))
        + opener.len();
    let body = &source[start..];
    let end = body
        .find("\n  }")
        .unwrap_or_else(|| panic!("the `{}` entry of {file} never closes", env.name()));
    &body[..end]
}

/// The value of the one `<key>: '<value>',` line of a JavaScript object body.
fn js_string(block: &str, key: &str, file: &str) -> String {
    let re = Regex::new(&format!(r"(?m)^\s*{key}:\s*'([^']*)',\s*$")).expect("regex");
    let mut matches = re.captures_iter(block);
    let first = matches
        .next()
        .unwrap_or_else(|| panic!("no `{key}` line in this entry of {file}:\n{block}"));
    assert!(
        matches.next().is_none(),
        "`{key}` is set twice in one entry of {file}"
    );
    first[1].to_owned()
}

/// The body of the `create(Flavors.<ENV>) { ... }` block, which sits at eight
/// spaces of indentation inside `productFlavors`.
fn gradle_flavor_block(source: &str, env: ProductEnv) -> &str {
    let opener = format!("create(Flavors.{}) {{\n", env.name().to_ascii_uppercase());
    let start = source
        .find(&opener)
        .unwrap_or_else(|| panic!("no `{}` flavor in {BUILD_GRADLE}", env.name()))
        + opener.len();
    let body = &source[start..];
    let end = body
        .find("\n        }")
        .unwrap_or_else(|| panic!("the `{}` flavor of {BUILD_GRADLE} never closes", env.name()));
    &body[..end]
}

/// The value of a `buildConfigField("String", "<name>", "\"<value>\"")` line.
fn gradle_build_config_string(block: &str, name: &str) -> String {
    let re = Regex::new(&format!(
        r#"buildConfigField\("String",\s*"{name}",\s*"\\"([^"\\]*)\\""\)"#
    ))
    .expect("regex");
    let mut matches = re.captures_iter(block);
    let first = matches.next().unwrap_or_else(|| {
        panic!("no string `{name}` build config field in this flavor of {BUILD_GRADLE}:\n{block}")
    });
    assert!(
        matches.next().is_none(),
        "`{name}` is set twice in one flavor of {BUILD_GRADLE}"
    );
    first[1].to_owned()
}

/// The first `<key> = "<value>"` assignment matching `pattern` in `text`.
fn gradle_assignment(text: &str, pattern: &str) -> Option<String> {
    Regex::new(&format!(r#"(?m)^\s*{pattern}\s*=\s*"([^"]*)""#))
        .expect("regex")
        .captures(text)
        .map(|captures| captures[1].to_owned())
}

/// `product-env.ts` is what the renderer and the main process read: the API
/// base it calls, the product name that namespaces its user data, the daemon
/// socket name, and the scheme the main process answers deep links on.
#[test]
fn the_desktop_typescript_table_is_the_crates() {
    let source = repo_file(PRODUCT_ENV_TS);
    for env in ALL {
        let block = js_env_block(&source, env, PRODUCT_ENV_TS);
        let name = env.name();
        // The TypeScript table carries a trailing slash: its consumers append
        // relative paths.
        assert_eq!(
            js_string(block, "apiBaseUrl", PRODUCT_ENV_TS),
            format!("{}/", env.api_url()),
            "{name}: apiBaseUrl"
        );
        assert_eq!(
            js_string(block, "displayName", PRODUCT_ENV_TS),
            env.display_name(),
            "{name}: displayName"
        );
        assert_eq!(
            js_string(block, "unixProductDir", PRODUCT_ENV_TS),
            env.unix_product_dir(),
            "{name}: unixProductDir"
        );
        assert_eq!(
            js_string(block, "deepLinkScheme", PRODUCT_ENV_TS),
            env.deep_link_scheme(),
            "{name}: deepLinkScheme"
        );
    }
}

/// `distribution.cjs` decides the packaged identity: the bundle id the OS
/// keys the install on, the product name, the executable and package name,
/// and the scheme the installer registers with the OS.
#[test]
fn the_desktop_packaging_table_is_the_crates() {
    let source = repo_file(DISTRIBUTION_CJS);
    for env in ALL {
        let block = js_env_block(&source, env, DISTRIBUTION_CJS);
        let name = env.name();
        assert_eq!(
            js_string(block, "appId", DISTRIBUTION_CJS),
            env.application_id(),
            "{name}: appId"
        );
        assert_eq!(
            js_string(block, "productName", DISTRIBUTION_CJS),
            env.display_name(),
            "{name}: productName"
        );
        assert_eq!(
            js_string(block, "packageName", DISTRIBUTION_CJS),
            env.unix_product_dir(),
            "{name}: packageName"
        );
        assert_eq!(
            js_string(block, "deepLinkScheme", DISTRIBUTION_CJS),
            env.deep_link_scheme(),
            "{name}: deepLinkScheme"
        );
    }
}

/// The Android flavors spell the scheme twice (the manifest placeholder the
/// intent filter is built from, and the `BuildConfig` field the parser
/// checks), the application id as a suffix on the default one, and the API
/// host as the debug override slot, empty for prod because the prod host is
/// the one the Rust crate compiles in.
#[test]
fn the_android_flavor_table_is_the_crates() {
    let source = repo_file(BUILD_GRADLE);
    let base_application_id = gradle_assignment(&source, "applicationId")
        .unwrap_or_else(|| panic!("no default `applicationId` in {BUILD_GRADLE}"));
    for env in ALL {
        let block = gradle_flavor_block(&source, env);
        let name = env.name();
        assert_eq!(
            gradle_build_config_string(block, "DEEP_LINK_SCHEME"),
            env.deep_link_scheme(),
            "{name}: DEEP_LINK_SCHEME"
        );
        assert_eq!(
            gradle_assignment(block, r#"manifestPlaceholders\["warrenDeepLinkScheme"\]"#)
                .unwrap_or_else(|| panic!("{name}: no warrenDeepLinkScheme placeholder")),
            env.deep_link_scheme(),
            "{name}: manifest placeholder"
        );
        let suffix = gradle_assignment(block, "applicationIdSuffix").unwrap_or_default();
        assert_eq!(
            format!("{base_application_id}{suffix}"),
            env.application_id(),
            "{name}: applicationId"
        );
        let expected_endpoint = if env == ProductEnv::Prod {
            ""
        } else {
            env.api_host()
        };
        assert_eq!(
            gradle_build_config_string(block, "API_ENDPOINT"),
            expected_endpoint,
            "{name}: API_ENDPOINT"
        );
    }
}
