//! Compile-time Warren product environment (prod | staging | beta).
//!
//! One binary is compiled for exactly one environment, selected with the
//! `WARREN_PRODUCT_ENV` env var at BUILD time (unset means prod). Every
//! product anchor that differs between environments (API URL, update
//! channel, on-disk product names, deep-link scheme, application id)
//! resolves through [`CURRENT`], so a beta build is a fully separate product
//! that coexists with a prod install: distinct API host, distinct update
//! channel, distinct settings/cache/RPC paths, its own URL scheme. The two
//! anchors one broker and one forum serve for every stack (the connect host,
//! the forum origin) are rows of the same table, so a split is one edit here.
//!
//! This crate is the reference; the copies a build tool needs before any
//! Rust runs (the desktop TypeScript table, the Electron packaging config,
//! the Android flavors) are held to it by `tests/platform_lockstep.rs`, and
//! the whole table crosses the mobile FFI as [`ProductEnv::anchors_json`].
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

    /// Windows SCM service name of this environment's daemon, so two
    /// installed environments never collide on the service registration, and
    /// so any environment can ask whether another one is installed on the
    /// machine.
    #[must_use]
    pub const fn windows_service_name(self) -> &'static str {
        match self {
            ProductEnv::Prod => "WarrenVPN",
            ProductEnv::Staging => "WarrenVPNStaging",
            ProductEnv::Beta => "WarrenVPNBeta",
        }
    }

    /// URL scheme this environment registers with the OS for its deep links
    /// (`<scheme>://forum-login?...`). Per environment, so a beta and a prod
    /// install on one device never fight over the registration, and a link
    /// the beta broker mints cannot land in the prod app.
    ///
    /// The desktop TypeScript table, the Electron packaging config and the
    /// Android flavors spell this value again because their build tools
    /// cannot read this crate; `tests/platform_lockstep.rs` reads those files
    /// and fails on drift.
    #[must_use]
    pub const fn deep_link_scheme(self) -> &'static str {
        match self {
            ProductEnv::Prod => "warren",
            ProductEnv::Staging => "warren-staging",
            ProductEnv::Beta => "warren-beta",
        }
    }

    /// Application identifier of this environment: the Electron `appId`
    /// (macOS bundle id, Windows AppUserModelID) and the Android
    /// `applicationId`, which is what makes each environment a separately
    /// installable app.
    #[must_use]
    pub const fn application_id(self) -> &'static str {
        match self {
            ProductEnv::Prod => "com.warrenbrowse.vpn",
            ProductEnv::Staging => "com.warrenbrowse.vpn.staging",
            ProductEnv::Beta => "com.warrenbrowse.vpn.beta",
        }
    }

    /// Host of the wallet-to-forum identity broker (warren-connect): the only
    /// host a forum deep link may name, and where the in-app report and the
    /// typed sign-in code go. One broker serves every stack today, so the
    /// value is the same in every row; it is a row so a split is one edit.
    #[must_use]
    pub const fn connect_host(self) -> &'static str {
        match self {
            ProductEnv::Prod | ProductEnv::Staging | ProductEnv::Beta => "connect.warrenbrowse.com",
        }
    }

    /// Public origin of the community forum, the one origin a topic link the
    /// app vouches for may point at. A bare https origin (no path, no port):
    /// the forum crate takes its host out of it. One forum serves every
    /// stack today.
    #[must_use]
    pub const fn forum_public_url(self) -> &'static str {
        match self {
            ProductEnv::Prod | ProductEnv::Staging | ProductEnv::Beta => {
                "https://forum.warrenbrowse.com"
            }
        }
    }

    /// Every anchor of this environment as one JSON object: the table the
    /// mobile FFI hands Kotlin (`WarrenJni.productAnchorsJson`) and Swift
    /// (`warren_product_anchors`). Its keys are the columns of
    /// `fixtures/client-rules/product_env.json`, and the fixture replay pins
    /// the rendering to the row, so every decoder reads the shape the
    /// fixture documents.
    #[must_use]
    pub fn anchors_json(self) -> String {
        serde_json::json!({
            "name": self.name(),
            "api_url": self.api_url(),
            "api_host": self.api_host(),
            "desktop_update_url": self.desktop_update_url(),
            "display_name": self.display_name(),
            "unix_product_dir": self.unix_product_dir(),
            "application_id": self.application_id(),
            "deep_link_scheme": self.deep_link_scheme(),
            "connect_host": self.connect_host(),
            "forum_public_url": self.forum_public_url(),
        })
        .to_string()
    }
}

/// WFP GUID salts of every environment other than `current` whose product is
/// not installed on this machine, per `is_installed`.
///
/// Those are the generations whose firewall objects are orphans: a kill
/// switch outlives the daemon that armed it, its object keys are
/// per-environment, and no installed build answers for them, so nothing else
/// will ever remove them. A daemon salted for another environment shipped
/// once (beta-v1.1.9 carried production keys) and its persistent block-all
/// walled the machine invisibly; sweeping orphan generations at startup is
/// what makes that class self-healing. An environment that IS installed keeps
/// its objects: they are that daemon's live kill switch, not garbage.
///
/// FAIL-SAFE DIRECTION: unknown means PRESENT. `is_installed` answers `None`
/// when it could not tell (an SCM query that errored, for example), and an
/// unanswerable environment counts as INSTALLED and is never swept: wrongly
/// sweeping disarms a kill switch someone relies on, wrongly skipping only
/// defers the sweep to the next daemon start. The fold lives here, next to
/// its tests, so no caller can get its direction wrong.
///
/// [`environments_with_priority_over`] leans the other way (unknown means
/// ABSENT), so the two must never share an implementation.
pub fn orphan_generation_salts(
    current: ProductEnv,
    is_installed: impl Fn(ProductEnv) -> Option<bool>,
) -> Vec<u32> {
    ALL.iter()
        .filter(|env| **env != current && !is_installed(**env).unwrap_or(true))
        .map(|env| env.guid_salt())
        .collect()
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

/// Every environment ranked from the strongest claim on the machine to the
/// weakest: prod outranks staging, staging outranks beta.
///
/// Two environments installed side by side both want the machine's single
/// tunnel, and the arbitration is one-directional by design: the WEAKER one
/// observes the stronger and stands down by itself. The stronger one is never
/// modified, and never issues a command, because the management socket is
/// world-accessible and a push design would hand any local process a
/// documented way to disarm a kill switch.
///
/// The order is declared once, here, so no call site invents its own
/// comparison. Any new environment takes a place in this list.
pub const PRECEDENCE: [ProductEnv; 3] = [ProductEnv::Prod, ProductEnv::Staging, ProductEnv::Beta];

/// The environments that outrank `current`, strongest first.
///
/// A build asks this which foreign environments it must watch, and stands
/// down while any of them asserts the machine. Prod gets an empty list, so
/// prod never watches anything.
///
/// FAIL-SAFE DIRECTION: unknown means ABSENT. The probe a caller builds on
/// top of this list must treat an environment it cannot reach, or whose
/// socket the OS does not vouch for, as NOT asserting. Wrongly yielding
/// disarms this build's own kill switch on no evidence, while wrongly staying
/// up only leaves two idle daemons. That is the opposite of
/// [`orphan_generation_salts`], which treats an unanswerable environment as
/// present; keep the two folds separate or the inversion is lost.
#[must_use]
pub fn environments_with_priority_over(current: ProductEnv) -> Vec<ProductEnv> {
    PRECEDENCE
        .iter()
        .take_while(|env| **env != current)
        .copied()
        .collect()
}

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

/// [`ProductEnv::deep_link_scheme`] of the compiled environment.
pub const DEEP_LINK_SCHEME: &str = CURRENT.deep_link_scheme();

/// [`ProductEnv::application_id`] of the compiled environment.
pub const APPLICATION_ID: &str = CURRENT.application_id();

/// [`ProductEnv::connect_host`] of the compiled environment.
pub const CONNECT_HOST: &str = CURRENT.connect_host();

/// [`ProductEnv::forum_public_url`] of the compiled environment.
pub const FORUM_PUBLIC_URL: &str = CURRENT.forum_public_url();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_service_names_are_unique_per_environment() {
        // The SCM registration is machine-wide, and the orphan-generation
        // sweep uses these names to decide whether another environment is
        // installed, so a shared or empty name breaks both.
        let names = [
            ProductEnv::Prod.windows_service_name(),
            ProductEnv::Staging.windows_service_name(),
            ProductEnv::Beta.windows_service_name(),
        ];
        assert_eq!(ProductEnv::Prod.windows_service_name(), "WarrenVPN");
        for (i, a) in names.iter().enumerate() {
            assert!(!a.is_empty());
            for b in &names[i + 1..] {
                assert_ne!(a, b, "two environments must never share a service name");
            }
        }
    }

    #[test]
    fn orphan_salts_cover_exactly_the_uninstalled_other_environments() {
        // Beta build, prod installed alongside, staging absent: only staging
        // is an orphan generation. The current environment is never swept
        // (its objects are this daemon's own), and neither is an installed
        // one (its objects are that daemon's live kill switch).
        let salts = orphan_generation_salts(ProductEnv::Beta, |env| Some(env == ProductEnv::Prod));
        assert_eq!(salts, vec![ProductEnv::Staging.guid_salt()]);
    }

    #[test]
    fn orphan_salts_sweep_all_other_environments_when_nothing_else_is_installed() {
        let salts = orphan_generation_salts(ProductEnv::Beta, |_| Some(false));
        assert_eq!(
            salts,
            vec![
                ProductEnv::Prod.guid_salt(),
                ProductEnv::Staging.guid_salt(),
            ]
        );
    }

    #[test]
    fn orphan_salts_are_empty_when_every_other_environment_is_installed() {
        let salts = orphan_generation_salts(ProductEnv::Prod, |_| Some(true));
        assert!(salts.is_empty());
    }

    #[test]
    fn an_environment_the_probe_cannot_answer_for_is_never_swept() {
        // The fail-safe direction this whole mechanism leans on: a probe that
        // errors (`None`) must count as installed. Sweeping on unknown would
        // disarm a live kill switch on any machine whose SCM query fails;
        // skipping only defers the sweep to the next daemon start.
        let salts = orphan_generation_salts(ProductEnv::Beta, |_| None);
        assert!(salts.is_empty());

        // Mixed answers: prod unanswerable (kept), staging known absent
        // (swept).
        let salts = orphan_generation_salts(ProductEnv::Beta, |env| match env {
            ProductEnv::Prod => None,
            _ => Some(false),
        });
        assert_eq!(salts, vec![ProductEnv::Staging.guid_salt()]);
    }

    #[test]
    fn prod_yields_to_nobody() {
        // Prod is the top of the order, so it never stands down for another
        // environment. The whole coexistence design leans on this: prod is
        // never modified by a lower environment.
        assert!(environments_with_priority_over(ProductEnv::Prod).is_empty());
    }

    #[test]
    fn beta_yields_to_prod_then_staging() {
        // Highest first, so a caller that stops at the first asserting
        // environment reports the strongest one.
        assert_eq!(
            environments_with_priority_over(ProductEnv::Beta),
            vec![ProductEnv::Prod, ProductEnv::Staging]
        );
    }

    #[test]
    fn staging_yields_to_prod_only() {
        assert_eq!(
            environments_with_priority_over(ProductEnv::Staging),
            vec![ProductEnv::Prod]
        );
    }

    #[test]
    fn precedence_is_total_and_antisymmetric() {
        // Total: every environment this product ships is ranked, so no
        // environment is left without a place in the order.
        for env in ALL {
            assert!(
                PRECEDENCE.contains(&env),
                "{} is missing from PRECEDENCE",
                env.name()
            );
        }
        assert_eq!(PRECEDENCE.len(), ALL.len());

        // Antisymmetric and irreflexive: for any two distinct environments
        // exactly one outranks the other, and none outranks itself. Without
        // this two environments could each stand down for the other and the
        // machine would end up with no tunnel at all.
        for a in PRECEDENCE {
            assert!(
                !environments_with_priority_over(a).contains(&a),
                "{} outranks itself",
                a.name()
            );
            for b in PRECEDENCE {
                if a == b {
                    continue;
                }
                let a_over_b = environments_with_priority_over(b).contains(&a);
                let b_over_a = environments_with_priority_over(a).contains(&b);
                assert!(
                    a_over_b ^ b_over_a,
                    "{} and {} must be strictly ordered",
                    a.name(),
                    b.name()
                );
            }
        }
    }

    #[test]
    fn precedence_never_reorders_the_orphan_sweep() {
        // The two folds have opposite fail-safe directions and must stay
        // separate: the sweep keeps an unanswerable environment, the yield
        // ignores one. Sharing an implementation is how that inversion gets
        // lost, so pin that the yield fold ignores installation entirely.
        assert_eq!(
            environments_with_priority_over(ProductEnv::Beta),
            vec![ProductEnv::Prod, ProductEnv::Staging]
        );
        assert!(orphan_generation_salts(ProductEnv::Beta, |_| None).is_empty());
    }

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
        struct Row {
            env: ProductEnv,
            name: &'static str,
            api_url: &'static str,
            api_host: &'static str,
            update_url: &'static str,
            unix_dir: &'static str,
            display: &'static str,
            scheme: &'static str,
            app_id: &'static str,
        }
        let rows = [
            Row {
                env: ProductEnv::Prod,
                name: "prod",
                api_url: "https://api.warrenbrowse.com",
                api_host: "api.warrenbrowse.com",
                update_url: "https://api.warrenbrowse.com/updates/desktop",
                unix_dir: "warren-vpn",
                display: "Warren VPN",
                scheme: "warren",
                app_id: "com.warrenbrowse.vpn",
            },
            Row {
                env: ProductEnv::Staging,
                name: "staging",
                api_url: "https://api.staging.warrenbrowse.com",
                api_host: "api.staging.warrenbrowse.com",
                update_url: "https://api.staging.warrenbrowse.com/updates/desktop",
                unix_dir: "warren-vpn-staging",
                display: "Warren VPN Staging",
                scheme: "warren-staging",
                app_id: "com.warrenbrowse.vpn.staging",
            },
            Row {
                env: ProductEnv::Beta,
                name: "beta",
                api_url: "https://api.beta.warrenbrowse.com",
                api_host: "api.beta.warrenbrowse.com",
                update_url: "https://api.beta.warrenbrowse.com/updates/desktop",
                unix_dir: "warren-vpn-beta",
                display: "Warren VPN Beta",
                scheme: "warren-beta",
                app_id: "com.warrenbrowse.vpn.beta",
            },
        ];
        for row in rows {
            let env = row.env;
            assert_eq!(env.name(), row.name);
            assert_eq!(env.api_url(), row.api_url);
            assert_eq!(env.api_host(), row.api_host);
            assert_eq!(env.desktop_update_url(), row.update_url);
            assert_eq!(env.unix_product_dir(), row.unix_dir);
            assert_eq!(env.display_name(), row.display);
            assert_eq!(env.deep_link_scheme(), row.scheme);
            assert_eq!(env.application_id(), row.app_id);
        }
    }

    /// One broker and one forum serve every stack today, so the two are the
    /// same in every row; the forum origin is a bare https origin because the
    /// forum crate derives the one host a vouched-for topic link may name from
    /// it, and a path or a port there would leak into that comparison.
    #[test]
    fn the_broker_and_the_forum_are_shared_by_every_environment() {
        for env in ALL {
            assert_eq!(env.connect_host(), "connect.warrenbrowse.com", "{env:?}");
            assert_eq!(
                env.forum_public_url(),
                "https://forum.warrenbrowse.com",
                "{env:?}"
            );
            let origin_host = env
                .forum_public_url()
                .strip_prefix("https://")
                .expect("the forum origin is https");
            assert!(
                !origin_host.is_empty() && !origin_host.contains(['/', ':', '?', '#']),
                "the forum origin is a bare host, got {origin_host}"
            );
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
        assert_eq!(DEEP_LINK_SCHEME, CURRENT.deep_link_scheme());
        assert_eq!(APPLICATION_ID, CURRENT.application_id());
        assert_eq!(CONNECT_HOST, CURRENT.connect_host());
        assert_eq!(FORUM_PUBLIC_URL, CURRENT.forum_public_url());
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
            // A shared scheme would hand one install's forum links to the
            // other; a shared application id is not two installs at all.
            assert_ne!(env.deep_link_scheme(), ProductEnv::Prod.deep_link_scheme());
            assert_ne!(env.application_id(), ProductEnv::Prod.application_id());
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

        type Anchor = fn(ProductEnv) -> &'static str;
        let table: [(&str, Anchor); 2] = [
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
            for value in [
                env.api_url(),
                env.desktop_update_url(),
                env.connect_host(),
                env.forum_public_url(),
            ] {
                assert!(
                    !value.contains("mullvad"),
                    "{value} must not point at Mullvad infrastructure"
                );
            }
        }
    }
}
