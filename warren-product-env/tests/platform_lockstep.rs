//! The per-environment product anchors are spelled again wherever a build
//! tool needs them before any Rust runs: the desktop TypeScript table
//! (`src/shared/constants/product-env.ts`), the Electron packaging config
//! (`tasks/distribution.cjs`), the Android flavors
//! (`android/app/build.gradle.kts`) and the iOS build settings
//! (`ios/Configurations/ProductEnv.xcconfig`). Each copy is read here as text and held
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
const IOS_PRODUCT_ENV_XCCONFIG: &str = "ios/Configurations/ProductEnv.xcconfig";
const IOS_INFO_PLIST: &str = "ios/WarrenVPN/Supporting Files/Info.plist";
const IOS_BASE_XCCONFIG: &str = "ios/Configurations/Base.xcconfig.template";
const IOS_API_XCCONFIG: &str = "ios/Configurations/Api.xcconfig.template";
const IOS_PBXPROJ: &str = "ios/WarrenVPN.xcodeproj/project.pbxproj";
const IOS_ASSET_CATALOG: &str = "ios/WarrenVPN/Supporting Files/Assets.xcassets";
const WARREN_CHECKS_WORKFLOW: &str = ".github/workflows/warren-checks.yml";

/// Every copy of the product table this suite holds, so the workflow filter
/// can be asked whether an edit to one of them runs the suite at all.
const COPIES_READ_BY_THIS_SUITE: [&str; 8] = [
    PRODUCT_ENV_TS,
    DISTRIBUTION_CJS,
    BUILD_GRADLE,
    IOS_PRODUCT_ENV_XCCONFIG,
    IOS_INFO_PLIST,
    IOS_BASE_XCCONFIG,
    IOS_API_XCCONFIG,
    IOS_PBXPROJ,
];

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

/// The value of the one `<name>_<env> = <value>` line of the iOS xcconfig
/// table (a build setting value runs to the end of its line, unquoted).
///
/// Read line by line rather than with a multiline regex: `\s` matches a
/// newline, so a pattern ending in `\s*$` walks past an EMPTY value onto the
/// next line and reports that line's value instead. Prod's empty app-id suffix
/// is exactly such a value.
fn xcconfig_value(source: &str, name: &str, env: ProductEnv) -> String {
    let key = format!("{name}_{}", env.name());
    let mut values = source.lines().filter_map(|line| {
        let rest = line.strip_prefix(&key)?;
        let rest = rest.trim_start_matches([' ', '\t']);
        Some(rest.strip_prefix('=')?.trim().to_owned())
    });
    let first = values
        .next()
        .unwrap_or_else(|| panic!("no `{key}` line in {IOS_PRODUCT_ENV_XCCONFIG}"));
    assert!(
        values.next().is_none(),
        "`{key}` is set twice in {IOS_PRODUCT_ENV_XCCONFIG}"
    );
    first
}

/// The iOS build cannot read the crate before Xcode resolves its settings, so
/// `ProductEnv.xcconfig` spells the scheme, the API host and the display name
/// per environment and selects the row with `WARREN_PRODUCT_ENV`, the same
/// selector `build-rust-library.sh` hands cargo. The three tables must be the
/// crate's, and each selector must resolve through the environment rather
/// than through a build configuration of its own.
#[test]
fn the_ios_xcconfig_table_is_the_crates() {
    let source = repo_file(IOS_PRODUCT_ENV_XCCONFIG);
    for env in ALL {
        let name = env.name();
        assert_eq!(
            xcconfig_value(&source, "WARREN_DEEP_LINK_SCHEME", env),
            env.deep_link_scheme(),
            "{name}: WARREN_DEEP_LINK_SCHEME"
        );
        assert_eq!(
            xcconfig_value(&source, "WARREN_API_HOST", env),
            env.api_host(),
            "{name}: WARREN_API_HOST"
        );
        assert_eq!(
            xcconfig_value(&source, "WARREN_DISPLAY_NAME", env),
            env.display_name(),
            "{name}: WARREN_DISPLAY_NAME"
        );
    }
    for selector in [
        "WARREN_DEEP_LINK_SCHEME",
        "WARREN_API_HOST",
        "WARREN_DISPLAY_NAME",
        "WARREN_APP_ID_SUFFIX",
        "WARREN_API_ENDPOINT",
        "WARREN_APPICON_NAME",
    ] {
        let line = format!("\n{selector} = $({selector}_$(WARREN_PRODUCT_ENV))\n");
        assert!(
            source.contains(&line),
            "{selector} must resolve through WARREN_PRODUCT_ENV"
        );
    }
    // Prod is the default everywhere (rule 50); no other line may set it
    // unconditionally.
    assert!(source.contains("\nWARREN_PRODUCT_ENV = prod\n"));
}

/// A beta build is a separate product or it is not a beta build: the bundle
/// id and the app group follow the environment, so it installs beside the prod
/// app instead of over it, and reads its own Keychain rather than the prod
/// wallet and the prod forum identity. The per-environment URL scheme exists
/// precisely so two installs on one device never fight over the registration,
/// which they cannot do while both claim one install slot.
#[test]
fn the_ios_bundle_identity_follows_the_product_environment() {
    let product_env = repo_file(IOS_PRODUCT_ENV_XCCONFIG);
    let base = repo_file(IOS_BASE_XCCONFIG);

    let mut suffixes: Vec<String> = Vec::new();
    for env in ALL {
        let suffix = xcconfig_value(&product_env, "WARREN_APP_ID_SUFFIX", env);
        let name = env.name();
        if env == ProductEnv::Prod {
            assert!(
                suffix.is_empty(),
                "prod keeps the shipped identity, so its suffix is empty"
            );
        } else {
            assert_eq!(suffix, format!(".{name}"), "{name}: WARREN_APP_ID_SUFFIX");
        }
        assert!(!suffixes.contains(&suffix), "{name}: suffix is not its own");
        suffixes.push(suffix);
    }

    for key in ["APPLICATION_IDENTIFIER", "SECURITY_GROUP_IDENTIFIER"] {
        let line = base
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{key} ")))
            .unwrap_or_else(|| panic!("no `{key}` line in {IOS_BASE_XCCONFIG}"));
        assert!(
            line.ends_with("$(WARREN_APP_ID_SUFFIX)"),
            "{key} must resolve through WARREN_APP_ID_SUFFIX, not a fixed id: {line}"
        );
        assert!(
            !base.contains(&format!("{key}[config=")),
            "{key} must not be overridden per build configuration"
        );
    }
}

/// The address-cache seed the iOS resolver dials before it self-populates has
/// to be a live IP of the CONFIGURED API host: it has no system-DNS fallback,
/// so a seed keyed on the build configuration sends a beta build to the
/// production box. Rule 50 forbids exactly that, and it stays invisible for as
/// long as the two names answer alike.
#[test]
fn the_ios_api_seed_follows_the_product_environment() {
    let api = repo_file(IOS_API_XCCONFIG);
    let product_env = repo_file(IOS_PRODUCT_ENV_XCCONFIG);

    assert!(
        !api.contains("API_ENDPOINT[config="),
        "API_ENDPOINT must not be keyed on a build configuration"
    );
    assert!(
        api.contains("\nAPI_ENDPOINT = $(WARREN_API_ENDPOINT)\n"),
        "API_ENDPOINT must resolve through WARREN_API_ENDPOINT"
    );

    let mut seeds: Vec<String> = Vec::new();
    for env in ALL {
        let seed = xcconfig_value(&product_env, "WARREN_API_ENDPOINT", env);
        let name = env.name();
        let (host, port) = seed.rsplit_once(':').unwrap_or_else(|| {
            panic!("{name}: WARREN_API_ENDPOINT is `<ip>:<port>`, got `{seed}`")
        });
        assert_eq!(port, "443", "{name}: the API is dialed on 443");
        assert!(
            host.split('.').count() == 4 && host.split('.').all(|o| o.parse::<u8>().is_ok()),
            "{name}: the seed is a literal IPv4 of the environment's API host, got `{host}`"
        );
        assert!(
            !seeds.contains(&seed),
            "{name}: shares the production seed, so a build of it dials the prod box"
        );
        seeds.push(seed);
    }
}

/// The URL scheme the OS registers for the app is the resolved selector, so
/// a beta build answers `warren-beta://` without anyone editing the plist; a
/// literal scheme in the plist would silently pin every build to one
/// environment again (a beta iOS install could not receive the beta broker's
/// link until 2026-09-03 for that reason).
#[test]
fn the_ios_url_scheme_is_the_xcconfig_selector_not_a_literal() {
    let plist = repo_file(IOS_INFO_PLIST);
    let start = plist
        .find("<key>CFBundleURLSchemes</key>")
        .expect("the app registers a URL scheme");
    let block = &plist[start..];
    let end = block.find("</array>").expect("the scheme list closes");
    let schemes: Vec<&str> = Regex::new(r"<string>([^<]*)</string>")
        .expect("regex")
        .captures_iter(&block[..end])
        .map(|c| c.get(1).expect("capture").as_str())
        .collect();
    assert_eq!(schemes, ["$(WARREN_DEEP_LINK_SCHEME)"]);
    for env in ALL {
        let literal = format!("<string>{}</string>", env.deep_link_scheme());
        assert!(
            !plist.contains(&literal),
            "{}: the scheme must not be spelled in the plist",
            env.name()
        );
    }
}

/// The quoted entries of the `paths:` sequence the workflow anchors as
/// `&warren_paths`, which its pull_request trigger reuses.
fn workflow_paths(workflow: &str) -> Vec<String> {
    const ANCHOR: &str = "    paths: &warren_paths\n";
    const ENTRY_INDENT: &str = "      ";
    let start = workflow
        .find(ANCHOR)
        .unwrap_or_else(|| panic!("no anchored `paths:` list in {WARREN_CHECKS_WORKFLOW}"))
        + ANCHOR.len();
    workflow[start..]
        .lines()
        .take_while(|line| line.starts_with(ENTRY_INDENT))
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .map(|entry| entry.trim_matches('"').to_owned())
        .collect()
}

/// Whether a GitHub `paths:` entry selects a file. The filter uses two shapes
/// only: a literal path, and a `<dir>/**` prefix.
fn path_selects(pattern: &str, path: &str) -> bool {
    match pattern.strip_suffix("/**") {
        Some(prefix) => path.starts_with(&format!("{prefix}/")),
        None => pattern == path,
    }
}

/// Every file this suite reads is a copy of the crate's table, and the suite
/// is the only thing holding that copy to it. The workflow's `paths:` filter
/// decides whether an edit runs the suite at all, so a copy missing from the
/// filter is a copy with no gate: `Info.plist` sat outside it while the URL
/// scheme test read it, and a literal scheme put back in the plist would have
/// reached main with nothing run.
#[test]
fn every_copy_this_suite_reads_triggers_the_workflow_that_runs_it() {
    let workflow = repo_file(WARREN_CHECKS_WORKFLOW);
    let patterns = workflow_paths(&workflow);
    assert!(
        patterns.len() > 1,
        "the `paths:` list of {WARREN_CHECKS_WORKFLOW} did not parse"
    );
    for copy in COPIES_READ_BY_THIS_SUITE {
        assert!(
            patterns.iter().any(|pattern| path_selects(pattern, copy)),
            "{copy} is read by this suite but no `paths:` entry of \
             {WARREN_CHECKS_WORKFLOW} selects it, so editing it runs no gate"
        );
    }
}

/// The bundle id lets a beta install sit beside a prod one; the icon is what
/// tells the two apart on the home screen and in the App Switcher, the only
/// place iOS shows the app before it is opened. The set is chosen by the
/// environment, and EVERY build configuration of the app target resolves the
/// same selector: leave one behind and a screenshot or UI-test build wears a
/// different icon from the one that ships.
#[test]
fn the_ios_app_icon_set_follows_the_product_environment() {
    let product_env = repo_file(IOS_PRODUCT_ENV_XCCONFIG);
    let pbxproj = repo_file(IOS_PBXPROJ);

    let settings: Vec<&str> = pbxproj
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("ASSETCATALOG_COMPILER_APPICON_NAME = ")
        })
        .map(|value| value.trim_end_matches(';'))
        .collect();
    assert!(
        !settings.is_empty(),
        "no ASSETCATALOG_COMPILER_APPICON_NAME in {IOS_PBXPROJ}"
    );
    for value in &settings {
        assert_eq!(
            *value, "\"$(WARREN_APPICON_NAME)\"",
            "every build configuration takes the icon set from the product \
             environment, not from a literal name"
        );
    }

    let prod_set = xcconfig_value(&product_env, "WARREN_APPICON_NAME", ProductEnv::Prod);
    for env in ALL {
        let name = env.name();
        let set = xcconfig_value(&product_env, "WARREN_APPICON_NAME", env);
        let icon = format!(
            "{}/../{IOS_ASSET_CATALOG}/{set}.appiconset/Icon-Light-1024x1024.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let artwork =
            std::fs::read(&icon).unwrap_or_else(|err| panic!("{name}: read {icon}: {err}"));
        if env == ProductEnv::Prod {
            continue;
        }
        assert_ne!(set, prod_set, "{name}: shares the production icon set");
        // A badged set that is a byte copy of the prod artwork tells a tester
        // nothing, which is the whole point of shipping a second one.
        let prod_icon = format!(
            "{}/../{IOS_ASSET_CATALOG}/{prod_set}.appiconset/Icon-Light-1024x1024.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let prod_artwork =
            std::fs::read(&prod_icon).unwrap_or_else(|err| panic!("read {prod_icon}: {err}"));
        assert_ne!(
            artwork, prod_artwork,
            "{name}: wears the production artwork, so nothing on the home \
             screen tells the two installs apart"
        );
    }
}
