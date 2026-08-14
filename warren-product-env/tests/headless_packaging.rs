//! The headless (daemon + CLI) Linux packages are described by static TOML in
//! `mullvad-daemon/Cargo.toml`, which no compiler checks against this crate.
//! Two properties are load-bearing and were both wrong at some point, so they
//! are pinned here rather than left to review:
//!
//! * the packages must carry the resources the daemon reads at boot, and
//! * a machine must never end up running two Warren daemons that share a
//!   runtime directory, a management socket and a firewall id.
//!
//! Reading the manifest as text keeps this crate dependency-free, the same way
//! the WFP-salt and dev-launcher drift tests read their shell scripts.

use warren_product_env::ALL;

const MANIFEST: &str = include_str!("../../mullvad-daemon/Cargo.toml");

/// Body of a `[header]` table, up to the next table header in column 0.
fn table<'a>(manifest: &'a str, header: &str) -> &'a str {
    let opener = format!("\n{header}\n");
    let start = manifest
        .find(&opener)
        .unwrap_or_else(|| panic!("no `{header}` table in mullvad-daemon/Cargo.toml"))
        + opener.len();
    let body = &manifest[start..];
    match body.find("\n[") {
        Some(end) => &body[..end],
        None => body,
    }
}

/// The `source = "..."` of every entry of an `assets = [...]` array.
fn asset_sources(table_body: &str) -> Vec<&str> {
    table_body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("{ source = \""))
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

/// Every package named on a `conflicts = [...]` line, or as a key of a
/// `[...conflicts]` table (cargo-deb and cargo-generate-rpm spell it
/// differently).
fn conflicts(manifest: &str) -> (Vec<String>, Vec<String>) {
    let deb_line = table(manifest, "[package.metadata.deb]")
        .lines()
        .find(|line| line.trim_start().starts_with("conflicts ="))
        .expect("the deb package must declare `conflicts`");
    let deb = deb_line
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();

    let rpm = table(manifest, "[package.metadata.generate-rpm.conflicts]")
        .lines()
        .filter_map(|line| line.split('=').next())
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .map(str::to_owned)
        .collect();

    (deb, rpm)
}

/// The daemon resolves its exit list from `resource_dir/warren-relays.json` at
/// boot. The GUI packages have always shipped it; the headless ones did not,
/// so a fresh server install started with an empty exit list and could not
/// connect until its first successful fetch from the API.
#[test]
fn the_headless_packages_ship_the_warren_exit_bootstrap() {
    for header in [
        "[package.metadata.deb]",
        "[package.metadata.generate-rpm]",
    ] {
        let sources = asset_sources(table(MANIFEST, header));
        assert!(
            sources.contains(&"../build/warren-relays.json"),
            "{header} does not package the Warren exit bootstrap; \
             a fresh install would start with no exits"
        );
    }
}

/// One asset list is edited and the other forgotten: the .deb and the .rpm
/// then install different files, and only users of the forgotten format hit
/// the missing one.
#[test]
fn the_deb_and_the_rpm_package_the_same_files() {
    let deb = asset_sources(table(MANIFEST, "[package.metadata.deb]"));
    let rpm = asset_sources(table(MANIFEST, "[package.metadata.generate-rpm]"));

    let mut missing_from_rpm: Vec<_> = deb.iter().filter(|s| !rpm.contains(s)).collect();
    let mut missing_from_deb: Vec<_> = rpm.iter().filter(|s| !deb.contains(s)).collect();
    missing_from_rpm.sort_unstable();
    missing_from_deb.sort_unstable();

    assert!(
        missing_from_rpm.is_empty() && missing_from_deb.is_empty(),
        "the headless asset lists drifted: absent from the rpm {missing_from_rpm:?}, \
         absent from the deb {missing_from_deb:?}"
    );
}

/// The headless package installs a daemon compiled for ONE environment, and
/// every environment's GUI package installs another daemon that claims the
/// same runtime directory, management socket and firewall id as the headless
/// build of that environment. Both are installable side by side unless the
/// packages say otherwise, and the loser of that race is whichever daemon
/// starts second, on a machine whose kill switch is armed.
#[test]
fn the_headless_package_conflicts_with_every_warren_gui_package() {
    let (deb, rpm) = conflicts(MANIFEST);

    for env in ALL {
        // The GUI package name of an environment is its unix product dir
        // (warren-vpn, warren-vpn-beta, warren-vpn-staging), so the set stays
        // complete on its own when an environment is added.
        let gui_package = env.unix_product_dir();
        assert!(
            deb.iter().any(|name| name == gui_package),
            "the .deb does not conflict with {gui_package}, so both daemons can be installed"
        );
        assert!(
            rpm.iter().any(|name| name == gui_package),
            "the .rpm does not conflict with {gui_package}, so both daemons can be installed"
        );
    }

    // The Mullvad package this fork descends from installs to the same paths.
    assert!(deb.iter().any(|name| name == "mullvad-vpn"));
    assert!(rpm.iter().any(|name| name == "mullvad-vpn"));
}
