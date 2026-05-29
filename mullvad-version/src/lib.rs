use std::cmp::Ordering;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::LazyLock;

use regex_lite::Regex;

/// The Warren VPN app product version
#[cfg(has_version)]
pub const VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/product-version.txt"));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    /// Optional semver patch component. `None` for legacy two-component
    /// `major.minor` versions (e.g. `2025.2`), `Some` for full semver
    /// `major.minor.patch` versions (e.g. `1.0.0`).
    pub patch: Option<u32>,
    /// A version can have an optional pre-stable type, e.g. alpha or beta.
    pub pre_stable: Option<PreStableType>,
    /// All versions may have an optional -dev-[commit hash] suffix.
    pub dev: Option<Hash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hash(String);

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum PreStableType {
    Alpha(u32),
    Beta(u32),
}

impl Ord for PreStableType {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PreStableType::Alpha(a), PreStableType::Alpha(b)) => a.cmp(b),
            (PreStableType::Beta(a), PreStableType::Beta(b)) => a.cmp(b),
            (PreStableType::Alpha(_), PreStableType::Beta(_)) => Ordering::Less,
            (PreStableType::Beta(_), PreStableType::Alpha(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for PreStableType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Version {
    /// Returns true if this version is a stable version.
    pub const fn is_stable(&self) -> bool {
        self.pre_stable.is_none() && !self.is_dev()
    }

    /// Returns true if this version is a beta version.
    pub const fn is_beta(&self) -> bool {
        matches!(self.pre_stable, Some(PreStableType::Beta(_)))
    }

    /// Returns true if this version has a -dev suffix, e.g. 1.0.0-beta1-dev-123abc
    pub const fn is_dev(&self) -> bool {
        self.dev.is_some()
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let type_ordering = match (&self.pre_stable, &other.pre_stable) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(self_pre_stable), Some(other_pre_stable)) => {
                self_pre_stable.cmp(other_pre_stable)
            }
        };

        // The dev vs non-dev ordering.
        let dev_ordering = match (&self.dev, &other.dev) {
            // All else being equal, a dev version is greater than a non-dev version
            (Some(_), None) => Some(Ordering::Greater),
            (None, Some(_)) => Some(Ordering::Less),

            // Dev-suffixes are not ordered, but they can be equal.
            (Some(a), Some(b)) if a != b => None,
            (Some(_), Some(_)) => Some(Ordering::Equal),

            (None, None) => Some(Ordering::Equal),
        };

        // A missing patch component is treated as 0, so `1.0` == `1.0.0`.
        let release_ordering = (self.major.cmp(&other.major))
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.unwrap_or(0).cmp(&other.patch.unwrap_or(0)))
            .then(type_ordering);

        match release_ordering {
            Ordering::Equal => dev_ordering,
            _ => Some(release_ordering),
        }
    }
}

impl Display for Version {
    /// Format Version as a string: major.minor[.patch]-{alpha|beta}-{dev}
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Version {
            major,
            minor,
            patch,
            pre_stable,
            dev,
        } = &self;

        write!(f, "{major}.{minor}")?;

        if let Some(patch) = patch {
            write!(f, ".{patch}")?;
        }

        match pre_stable {
            Some(PreStableType::Alpha(version)) => write!(f, "-alpha{version}")?,
            Some(PreStableType::Beta(version)) => write!(f, "-beta{version}")?,
            None => (),
        };

        if let Some(commit_hash) = dev {
            write!(f, "-dev-{commit_hash}")?;
        }

        Ok(())
    }
}

static VERSION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)                                     # enable insignificant whitespace mode
                (?<major>[1-9]\d{0,3})\.           # major component (1-9999; semver major or legacy year)
                (?<minor>0|[1-9]\d?)               # minor component (0-99)
                (?:\.(?<patch>0|[1-9]\d?))?        # (optional) semver patch component (0-99)
                (?:                                # (optional) alpha or beta
                  -alpha(?<alpha>[1-9]\d?\d?)|
                  -beta(?<beta>[1-9]\d?\d?)
                )?
                (?:                                # (optional) dev suffix
                  -dev-(?<dev>[0-9a-f]+)
                )?$
                ",
    )
    .unwrap()
});

impl FromStr for Version {
    type Err = String;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let captures = VERSION_REGEX
            .captures(version)
            .ok_or_else(|| format!("Version does not match expected format: {version}"))?;

        let major = captures.name("major").unwrap().as_str().parse().unwrap();

        let minor = captures.name("minor").unwrap().as_str().parse().unwrap();

        let patch = captures.name("patch").map(|m| m.as_str().parse().unwrap());

        let alpha = captures.name("alpha").map(|m| m.as_str().parse().unwrap());
        let beta = captures.name("beta").map(|m| m.as_str().parse().unwrap());
        let dev = captures
            .name("dev")
            .map(|m| m.as_str().to_owned())
            .map(Hash);

        let pre_stable = match (alpha, beta) {
            (None, None) => None,
            (Some(v), None) => Some(PreStableType::Alpha(v)),
            (None, Some(v)) => Some(PreStableType::Beta(v)),
            _ => return Err(format!("Invalid version: {version}")),
        };

        Ok(Version {
            major,
            minor,
            patch,
            pre_stable,
            dev,
        })
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "arbitrary")]
pub mod arbitrary {
    use super::*;

    use prop::option;
    use prop::string;
    use proptest::prelude::*;

    prop_compose! {
        /// Generate an arbitrary [Version].
        pub fn arb_version()
            (major in arb_major(), minor in arb_minor(), patch in option::of(arb_patch()), pre_stable in option::of(arb_pre_stable()), dev in option::of(arb_hash()))
            -> Version {
                Version { major, minor, patch, pre_stable, dev }
        }
    }

    /// Generate an arbitrary Warren App version major component.
    fn arb_major() -> impl Strategy<Value = u32> {
        1u32..=9999
    }

    /// Generate an arbitrary Warren App version minor component.
    fn arb_minor() -> impl Strategy<Value = u32> {
        0u32..=99
    }

    /// Generate an arbitrary Warren App version patch component.
    fn arb_patch() -> impl Strategy<Value = u32> {
        0u32..=99
    }

    /// Generate an arbitrary Warren App version pre-stable type.
    fn arb_pre_stable() -> impl Strategy<Value = PreStableType> {
        let alpha = |number| Just(PreStableType::Alpha(number));
        let beta = |number| Just(PreStableType::Beta(number));
        (1u32..999).prop_flat_map(move |number| prop_oneof![alpha(number), beta(number)])
    }

    /// Generate an arbitrary git short-hash.
    fn arb_hash() -> impl Strategy<Value = Hash> {
        string::string_regex("([0-9a-f]+)").unwrap().prop_map(Hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use arbitrary::arb_version;
    use proptest::prelude::*;

    // Helper to parse a version string
    fn parse(version: &str) -> Version {
        version.parse().unwrap()
    }

    #[test]
    fn test_product_version() {
        parse(VERSION);
    }

    #[test]
    fn test_version_ordering() {
        // Test year comparison
        assert!(parse("2022.1") > parse("2021.1"),);

        // Test incremental comparison
        assert!(parse("2021.2") > parse("2021.1"),);

        // Test stable vs pre-release
        assert!(parse("2021.1") > parse("2021.1-beta1"),);
        assert!(parse("2021.1") > parse("2021.1-alpha1"),);

        // Test beta vs alpha
        assert!(parse("2021.1-beta1") > parse("2021.1-alpha1"),);
        assert!(parse("2021.1-beta1") > parse("2021.1-alpha2"),);
        assert!(parse("2021.2-alpha1") > parse("2021.1-beta2"),);

        // Test version numbers within same type
        assert!(parse("2021.1-beta2") > parse("2021.1-beta1"),);
        assert!(parse("2021.1-alpha2") > parse("2021.1-alpha1"),);

        // Test dev versions
        assert!(parse("2021.1-dev-abc") > parse("2021.1"),);
        assert!(parse("2021.2") > parse("2021.1-dev-abc"),);
        assert!(parse("2021.1-dev-abc") > parse("2021.1-beta1"),);
        assert!(parse("2021.1-dev-abc") > parse("2021.1-alpha1"),);
        assert!(parse("2025.1-dev-abc") > parse("2025.1-beta1-dev-abc"),);
        assert!(parse("2025.1-dev-abc") > parse("2025.1-beta2-dev-abc"),);
        assert!(parse("2025.1-dev-abc") > parse("2025.1-alpha2-dev-abc"),);
        assert!(parse("2025.1-beta1-dev-abc") > parse("2025.1-alpha7-dev-abc"),);
        assert!(parse("2025.2-alpha1-dev-abc") > parse("2025.1-beta7-dev-abc"),);

        // Test version equality
        assert_eq!(parse("2021.1"), parse("2021.1"));
        assert_eq!(parse("2021.1-beta1"), parse("2021.1-beta1"));
        assert_eq!(parse("2021.1-alpha7"), parse("2021.1-alpha7"));
        assert_eq!(parse("2021.1-dev-abc123"), parse("2021.1-dev-abc123"));
        assert_ne!(parse("2021.1-dev-abc123"), parse("2021.1-dev-def123"));
    }

    #[test]
    fn test_semver_ordering() {
        // Semver patch / minor / major ordering
        assert!(parse("1.0.1") > parse("1.0.0"));
        assert!(parse("1.1.0") > parse("1.0.9"));
        assert!(parse("2.0.0") > parse("1.9.9"));

        // Stable beats its own pre-releases and dev builds
        assert!(parse("1.0.0") > parse("1.0.0-beta1"));
        assert!(parse("1.0.0") > parse("1.0.0-alpha1"));
        assert!(parse("1.0.0-dev-abc") > parse("1.0.0"));

        // A missing patch is treated as 0 in ordering (1.0 ranks equal to 1.0.0)
        assert_eq!(
            parse("1.0").partial_cmp(&parse("1.0.0")),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn test_version_ordering_and_equality() {
        let v = parse("2021.3");

        // A version is equal to itself
        assert_eq!(v, v);
        assert_eq!(v.partial_cmp(&v), Some(Ordering::Equal));
    }

    #[test]
    fn test_version_ordering_and_equality_dev() {
        let v1 = parse("2021.3-dev-abc");
        let v2 = parse("2021.3-dev-def");

        // A dev version is equal to itself
        assert_eq!(v1, v1);
        assert_eq!(v1.partial_cmp(&v1), Some(Ordering::Equal));

        // Equal down to the dev suffix are not equal, and has no ordering
        assert_ne!(v1, v2);
        assert!(v1.partial_cmp(&v2).is_none());
    }

    #[test]
    fn test_parse() {
        assert_eq!(
            parse("2021.34"),
            Version {
                major: 2021,
                minor: 34,
                patch: None,
                pre_stable: None,
                dev: None,
            }
        );
    }

    #[test]
    fn test_parse_semver() {
        assert_eq!(
            parse("1.0.0"),
            Version {
                major: 1,
                minor: 0,
                patch: Some(0),
                pre_stable: None,
                dev: None,
            }
        );

        assert_eq!(
            parse("1.2.3-beta4-dev-e5483d"),
            Version {
                major: 1,
                minor: 2,
                patch: Some(3),
                pre_stable: Some(PreStableType::Beta(4)),
                dev: Some(Hash("e5483d".to_string())),
            }
        );
    }

    #[test]
    fn test_parse_with_alpha() {
        assert_eq!(
            parse("2023.1-alpha77"),
            Version {
                major: 2023,
                minor: 1,
                patch: None,
                pre_stable: Some(PreStableType::Alpha(77)),
                dev: None,
            }
        );

        assert_eq!(
            parse("2021.34-alpha777"),
            Version {
                major: 2021,
                minor: 34,
                patch: None,
                pre_stable: Some(PreStableType::Alpha(777)),
                dev: None,
            }
        );
    }

    #[test]
    fn test_parse_with_beta() {
        assert_eq!(
            parse("2021.34-beta5"),
            Version {
                major: 2021,
                minor: 34,
                patch: None,
                pre_stable: Some(PreStableType::Beta(5)),
                dev: None,
            }
        );

        assert_eq!(
            parse("2021.34-beta453"),
            Version {
                major: 2021,
                minor: 34,
                patch: None,
                pre_stable: Some(PreStableType::Beta(453)),
                dev: None,
            }
        );
    }

    #[test]
    fn test_parse_with_dev() {
        assert_eq!(
            parse("2021.34-dev-0b60e4d87"),
            Version {
                major: 2021,
                minor: 34,
                patch: None,
                pre_stable: None,
                dev: Some(Hash("0b60e4d87".to_string())),
            }
        );
    }

    #[test]
    fn test_parse_both_beta_and_dev() {
        assert_eq!(
            parse("2024.8-beta1-dev-e5483d"),
            Version {
                major: 2024,
                minor: 8,
                patch: None,
                pre_stable: Some(PreStableType::Beta(1)),
                dev: Some(Hash("e5483d".to_string())),
            }
        );
    }

    #[test]
    fn test_returns_error_on_invalid_version() {
        assert!("2021".parse::<Version>().is_err());
        assert!("not-a-version".parse::<Version>().is_err());
        assert!("".parse::<Version>().is_err());
    }

    #[test]
    fn test_returns_error_on_invalid_incremental() {
        assert!("2021.2a".parse::<Version>().is_err());
    }

    #[test]
    fn test_returns_error_on_invalid_version_type() {
        assert!("2021.2-omega".parse::<Version>().is_err());
    }

    #[test]
    fn test_returns_error_on_invalid_version_type_number() {
        assert!("2021.1-beta001".parse::<Version>().is_err());
    }

    #[test]
    fn test_returns_error_on_alpha_and_beta_in_same_version() {
        assert!("2021.1-beta5-alpha2".parse::<Version>().is_err());
    }

    #[test]
    fn test_returns_error_on_dev_without_commit_hash() {
        assert!("2021.1-dev".parse::<Version>().is_err())
    }

    #[test]
    fn test_version_display() {
        let assert_same_display = |version: &str| {
            let parsed = Version::from_str(version).unwrap();
            assert_eq!(parsed.to_string(), version);
        };

        assert_same_display("2024.8-beta1-dev-e5483d");
        assert_same_display("2024.8-beta1");
        assert_same_display("2024.8-alpha77-dev-85483d");
        assert_same_display("2024.12");
        assert_same_display("2045.2-dev-123");

        // Full semver forms round-trip too.
        assert_same_display("1.0.0");
        assert_same_display("1.2.3");
        assert_same_display("1.0.0-beta1");
        assert_same_display("1.2.3-alpha4-dev-abcdef");
    }

    proptest! {
        #[test]
        fn parse_all_version_numbers(version in arb_version()) {
            parse(&version.to_string());
        }
    }
}
