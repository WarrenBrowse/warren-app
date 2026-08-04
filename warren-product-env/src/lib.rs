//! Compile-time Warren product environment (prod | staging | beta).
//!
//! One binary is compiled for exactly one environment, selected with the
//! `WARREN_PRODUCT_ENV` env var at BUILD time (unset means prod). Every
//! product anchor that differs between environments (API URL, update
//! channel, on-disk product names) resolves through [`CURRENT`], so a beta
//! build is a fully separate product that coexists with a prod install:
//! distinct API host, distinct update channel, distinct settings/cache/RPC
//! paths.
//!
//! The server signing pubkey is intentionally NOT here: all environments
//! are signed by the same key, and its canonical anchor stays
//! `warren_contract::product::SERVER_PUBKEY_HEX` (plus the app-local
//! lockstep copies next to their consumers).
//!
//! Runtime overrides (the `WARREN_API_URL` env var, the persisted
//! `Settings::warren_api_url`, `WARREN_UPDATE_URL`, ...) still apply on
//! top: this crate only supplies the compiled DEFAULTS those chains fall
//! back to.

/// The Warren product environment a binary is compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductEnv {
    Prod,
    Staging,
    Beta,
}

impl ProductEnv {
    /// Stable lowercase name, for UI/telemetry and build plumbing.
    pub const fn name(self) -> &'static str {
        match self {
            ProductEnv::Prod => "prod",
            ProductEnv::Staging => "staging",
            ProductEnv::Beta => "beta",
        }
    }

    /// Base URL of the environment's warren-api deployment.
    pub const fn api_url(self) -> &'static str {
        match self {
            ProductEnv::Prod => "https://api.warrenbrowse.com",
            ProductEnv::Staging => "https://api.staging.warrenbrowse.com",
            ProductEnv::Beta => "https://api.beta.warrenbrowse.com",
        }
    }

    /// Host component of [`Self::api_url`].
    pub const fn api_host(self) -> &'static str {
        match self {
            ProductEnv::Prod => "api.warrenbrowse.com",
            ProductEnv::Staging => "api.staging.warrenbrowse.com",
            ProductEnv::Beta => "api.beta.warrenbrowse.com",
        }
    }

    /// Base URL of the desktop signed-manifest update channel
    /// (`{this}/{platform}.json`), mirroring the prod layout under each
    /// environment's API host.
    pub const fn desktop_update_url(self) -> &'static str {
        match self {
            ProductEnv::Prod => "https://api.warrenbrowse.com/updates/desktop",
            ProductEnv::Staging => "https://api.staging.warrenbrowse.com/updates/desktop",
            ProductEnv::Beta => "https://api.beta.warrenbrowse.com/updates/desktop",
        }
    }

    /// Per-install directory name on unix-like systems (settings, cache,
    /// logs, RPC socket). Distinct per environment so two installed
    /// environments never share state or fight over the same socket.
    pub const fn unix_product_dir(self) -> &'static str {
        match self {
            ProductEnv::Prod => "warren-vpn",
            ProductEnv::Staging => "warren-vpn-staging",
            ProductEnv::Beta => "warren-vpn-beta",
        }
    }

    /// Identifier this environment claims on the machine's single firewall
    /// namespace: the nftables table on Linux, the pf anchor on macOS. Those
    /// are one-per-machine resources whose teardown is name-scoped, so two
    /// environments sharing a name means either one flushing the other's
    /// kill-switch rules out from under it.
    #[must_use]
    pub const fn firewall_id(self) -> &'static str {
        match self {
            ProductEnv::Prod => "warren",
            ProductEnv::Staging => "warren-staging",
            ProductEnv::Beta => "warren-beta",
        }
    }

    /// D-Bus name this environment's NetworkManager VPN service plugin owns,
    /// which is also the `service-type` its connection profiles carry.
    ///
    /// NetworkManager keys its plugin registry on this name and a bus name
    /// has a single owner per machine, so environments must not share one.
    /// A dash is legal in a bus name element (unlike an interface name).
    #[must_use]
    pub const fn nm_vpn_service(self) -> &'static str {
        match self {
            ProductEnv::Prod => "org.freedesktop.NetworkManager.warren",
            ProductEnv::Staging => "org.freedesktop.NetworkManager.warren-staging",
            ProductEnv::Beta => "org.freedesktop.NetworkManager.warren-beta",
        }
    }

    /// User-facing product name, also the per-install directory name on
    /// Windows (ProgramData subdir) and macOS (Application Support subdir).
    pub const fn display_name(self) -> &'static str {
        match self {
            ProductEnv::Prod => "Warren VPN",
            ProductEnv::Staging => "Warren VPN Staging",
            ProductEnv::Beta => "Warren VPN Beta",
        }
    }

    /// Salt XORed into this environment's Windows WFP object keys, so each
    /// environment owns a disjoint set of filters, sublayers and providers in
    /// the machine-wide WFP namespace. Prod is 0, which leaves its keys bit
    /// for bit unchanged.
    ///
    /// Must stay in lockstep with `warren_fw_guid_salt()` in
    /// `build-windows-modules.sh`, which is what actually reaches the C++
    /// compiler. The drift gate in this crate's tests fails otherwise.
    #[must_use]
    pub const fn guid_salt(self) -> u32 {
        match self {
            ProductEnv::Prod => 0x0,
            ProductEnv::Staging => 0x57A6_1009,
            ProductEnv::Beta => 0x5BE7_A001,
        }
    }
}

/// Every environment this product has ever shipped, newest naming first.
///
/// A machine can carry firewall state from an environment it no longer runs:
/// a kill switch outlives the process that armed it, and the identity of that
/// state (WFP object keys, nftables table, pf anchor) is per-environment. So
/// recovery has to sweep the whole set, not just the compiled one, otherwise
/// an install that changed environment leaves blocking rules nothing can
/// remove. Any new environment must be added here, and the tests enforce that
/// the set stays complete.
pub const ALL: [ProductEnv; 3] = [ProductEnv::Prod, ProductEnv::Staging, ProductEnv::Beta];

const fn parse(name: &str) -> ProductEnv {
    // const-compatible string compare; build.rs already validated the value.
    const fn eq(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    if eq(name, "beta") {
        ProductEnv::Beta
    } else if eq(name, "staging") {
        ProductEnv::Staging
    } else {
        ProductEnv::Prod
    }
}

/// The environment this binary is compiled for.
pub const CURRENT: ProductEnv = parse(env!("WARREN_PRODUCT_ENV_RESOLVED"));

/// Stable lowercase name of [`CURRENT`], for UI/telemetry.
pub const ENV_NAME: &str = CURRENT.name();

/// Compiled default warren-api base URL for [`CURRENT`].
pub const API_URL: &str = CURRENT.api_url();

/// Host component of [`API_URL`].
pub const API_HOST: &str = CURRENT.api_host();

/// Compiled default desktop update-channel base URL for [`CURRENT`].
pub const DESKTOP_UPDATE_URL: &str = CURRENT.desktop_update_url();

/// Per-install directory name on unix-like systems for [`CURRENT`].
pub const UNIX_PRODUCT_DIR: &str = CURRENT.unix_product_dir();

/// [`ProductEnv::firewall_id`] of the compiled environment.
pub const FIREWALL_ID: &str = CURRENT.firewall_id();

/// [`ProductEnv::nm_vpn_service`] of the compiled environment.
pub const NM_VPN_SERVICE: &str = CURRENT.nm_vpn_service();

/// User-facing product name (and Windows/macOS per-install directory name)
/// for [`CURRENT`].
pub const DISPLAY_NAME: &str = CURRENT.display_name();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firewall_ids_are_unique_per_environment() {
        // The firewall namespace is one per machine and its teardown is
        // name-scoped, so a shared id means one environment can flush
        // another's kill-switch rules. Prod keeps the historical name.
        let ids = [
            ProductEnv::Prod.firewall_id(),
            ProductEnv::Staging.firewall_id(),
            ProductEnv::Beta.firewall_id(),
        ];
        assert_eq!(ProductEnv::Prod.firewall_id(), "warren");
        for (i, a) in ids.iter().enumerate() {
            assert!(!a.is_empty(), "an empty id would claim the default anchor");
            for b in &ids[i + 1..] {
                assert_ne!(a, b, "two environments must never share a firewall id");
            }
        }
    }

    /// The per-environment anchor table. Prod values are the canonical
    /// product anchors (lockstep with `warren_contract::product`); beta and
    /// staging mirror the prod layout under their own API host. Covering
    /// all three environments here keeps the mapping pinned regardless of
    /// which environment the test binary itself was compiled for.
    #[test]
    fn per_environment_anchor_table() {
        let cases = [
            (
                ProductEnv::Prod,
                "prod",
                "https://api.warrenbrowse.com",
                "api.warrenbrowse.com",
                "https://api.warrenbrowse.com/updates/desktop",
                "warren-vpn",
                "Warren VPN",
            ),
            (
                ProductEnv::Staging,
                "staging",
                "https://api.staging.warrenbrowse.com",
                "api.staging.warrenbrowse.com",
                "https://api.staging.warrenbrowse.com/updates/desktop",
                "warren-vpn-staging",
                "Warren VPN Staging",
            ),
            (
                ProductEnv::Beta,
                "beta",
                "https://api.beta.warrenbrowse.com",
                "api.beta.warrenbrowse.com",
                "https://api.beta.warrenbrowse.com/updates/desktop",
                "warren-vpn-beta",
                "Warren VPN Beta",
            ),
        ];
        for (env, name, api_url, api_host, update_url, unix_dir, display) in cases {
            assert_eq!(env.name(), name);
            assert_eq!(env.api_url(), api_url);
            assert_eq!(env.api_host(), api_host);
            assert_eq!(env.desktop_update_url(), update_url);
            assert_eq!(env.unix_product_dir(), unix_dir);
            assert_eq!(env.display_name(), display);
        }
    }

    /// NetworkManager keys its VPN plugin registry on the service name, and
    /// a bus name has a single owner machine-wide. Two environments sharing
    /// one would leave the second install unable to claim it, and the
    /// desktop would credit one environment's tunnel to the other.
    #[test]
    fn nm_vpn_service_names_are_unique_per_environment() {
        let names = [
            ProductEnv::Prod.nm_vpn_service(),
            ProductEnv::Staging.nm_vpn_service(),
            ProductEnv::Beta.nm_vpn_service(),
        ];
        assert_eq!(
            ProductEnv::Prod.nm_vpn_service(),
            "org.freedesktop.NetworkManager.warren"
        );
        for (i, a) in names.iter().enumerate() {
            for b in &names[i + 1..] {
                assert_ne!(a, b, "two environments would fight over one bus name");
            }
        }
    }

    /// The compiled selection must be internally coherent: every exported
    /// const is exactly the [`CURRENT`] row of the anchor table.
    #[test]
    fn current_consts_match_selected_environment() {
        assert_eq!(NM_VPN_SERVICE, CURRENT.nm_vpn_service());
        assert_eq!(ENV_NAME, CURRENT.name());
        assert_eq!(API_URL, CURRENT.api_url());
        assert_eq!(API_HOST, CURRENT.api_host());
        assert_eq!(DESKTOP_UPDATE_URL, CURRENT.desktop_update_url());
        assert_eq!(UNIX_PRODUCT_DIR, CURRENT.unix_product_dir());
        assert_eq!(DISPLAY_NAME, CURRENT.display_name());
    }

    /// Non-prod environments must differ from prod on every axis that
    /// makes them a separately installable product. A value accidentally
    /// equal to prod's would silently share state or traffic with prod.
    #[test]
    fn non_prod_environments_never_collide_with_prod() {
        for env in [ProductEnv::Staging, ProductEnv::Beta] {
            assert_ne!(env.api_url(), ProductEnv::Prod.api_url());
            assert_ne!(env.api_host(), ProductEnv::Prod.api_host());
            assert_ne!(
                env.desktop_update_url(),
                ProductEnv::Prod.desktop_update_url()
            );
            assert_ne!(env.unix_product_dir(), ProductEnv::Prod.unix_product_dir());
            assert_ne!(env.display_name(), ProductEnv::Prod.display_name());
        }
        assert_ne!(
            ProductEnv::Beta.unix_product_dir(),
            ProductEnv::Staging.unix_product_dir()
        );
    }

    /// The recovery sweep is only as complete as this list. A new environment
    /// added to the enum but forgotten here would ship a kill switch that no
    /// recovery path can find.
    #[test]
    fn the_generation_inventory_covers_every_environment() {
        for env in [ProductEnv::Prod, ProductEnv::Staging, ProductEnv::Beta] {
            assert!(
                super::ALL.contains(&env),
                "{env:?} is missing from ALL, so recovery would never sweep its firewall state"
            );
        }
    }

    /// Two environments sharing a salt would share WFP object keys, and one
    /// would tear down the other's kill switch.
    #[test]
    fn guid_salts_are_unique_per_environment() {
        assert_eq!(
            ProductEnv::Prod.guid_salt(),
            0,
            "prod keys must be unsalted"
        );
        for (i, a) in super::ALL.iter().enumerate() {
            for b in &super::ALL[i + 1..] {
                assert_ne!(
                    a.guid_salt(),
                    b.guid_salt(),
                    "{a:?} and {b:?} would claim the same WFP objects"
                );
            }
        }
    }

    /// The value that actually reaches the C++ compiler lives in the build
    /// script, so this table is a copy. Read the script and fail on drift:
    /// a silent mismatch would make the daemon purge object keys that no
    /// build ever created, leaving the real ones stranded on user machines.
    #[test]
    fn guid_salts_match_the_windows_build_script() {
        const SCRIPT_PATH: &str =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../build-windows-modules.sh");
        let script = std::fs::read_to_string(SCRIPT_PATH)
            .expect("build-windows-modules.sh must exist at the expected path");

        for env in super::ALL {
            // Matches e.g. `        beta)    echo "0x5BE7A001" ;;`
            let line = script
                .lines()
                .map(str::trim)
                .find(|line| line.starts_with(&format!("{})", env.name())))
                .unwrap_or_else(|| {
                    panic!(
                        "no salt case for `{}` in build-windows-modules.sh",
                        env.name()
                    )
                });
            let script_salt = line
                .split('"')
                .nth(1)
                .and_then(|hex| u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok())
                .unwrap_or_else(|| panic!("unparsable salt for `{}`: {line}", env.name()));

            assert_eq!(
                script_salt,
                env.guid_salt(),
                "the {} WFP salt drifted between build-windows-modules.sh and this crate",
                env.name()
            );
        }
    }

    /// Body of a `name() {` shell function, up to the closing brace in
    /// column 0.
    fn shell_function_body<'a>(script: &'a str, name: &str) -> &'a str {
        let opener = format!("\n{name}() {{\n");
        let start = script
            .find(&opener)
            .unwrap_or_else(|| panic!("no `{name}` function in the shell helper"))
            + opener.len();
        let body = &script[start..];
        let end = body
            .find("\n}")
            .unwrap_or_else(|| panic!("`{name}` is never closed in column 0"));
        &body[..end]
    }

    /// Value of a `<label>) echo "<value>" ;;` arm of a shell `case`.
    fn shell_case_value(body: &str, label: &str) -> String {
        let line = body
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(&format!("{label})")))
            .unwrap_or_else(|| panic!("no `{label}` case arm in:\n{body}"));
        line.split('"')
            .nth(1)
            .unwrap_or_else(|| panic!("unquoted case value: {line}"))
            .to_owned()
    }

    /// The dev launchers pick an environment in shell, and have to agree with
    /// what the build they trigger compiles in: the API host a build talks to,
    /// and the product directory that names the daemon's socket. Their table
    /// is a copy, so read it and fail on drift. A launcher one row out of step
    /// builds one environment and then drives another one's daemon, and both
    /// halves look healthy while doing it.
    #[test]
    fn dev_launcher_tables_match_the_shared_shell_helper() {
        const SCRIPT_PATH: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/utils/product-env.sh"
        );
        let script = std::fs::read_to_string(SCRIPT_PATH)
            .expect("scripts/utils/product-env.sh must exist at the expected path");

        let table: [(&str, fn(ProductEnv) -> &'static str); 2] = [
            ("warren_env_api_host", ProductEnv::api_host),
            ("warren_env_product_dir", ProductEnv::unix_product_dir),
        ];
        for (function, anchor) in table {
            let body = shell_function_body(&script, function);
            for env in super::ALL {
                assert_eq!(
                    shell_case_value(body, env.name()),
                    anchor(env),
                    "`{function}` drifted from this crate for the {} environment",
                    env.name()
                );
            }
        }
    }

    /// No environment may point at Mullvad upstream infrastructure.
    #[test]
    fn no_environment_references_mullvad() {
        for env in [ProductEnv::Prod, ProductEnv::Staging, ProductEnv::Beta] {
            for value in [env.api_url(), env.desktop_update_url()] {
                assert!(
                    !value.contains("mullvad"),
                    "{value} must not point at Mullvad infrastructure"
                );
            }
        }
    }
}
