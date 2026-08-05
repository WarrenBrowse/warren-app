#[cfg(has_version)]
mod inner {
    use mullvad_version::{PreStableType, Version};
    use std::env::VarError;
    use std::{env, process::exit};

    const ANDROID_VERSION: &str =
        include_str!(concat!(env!("OUT_DIR"), "/android-version-name.txt"));

    pub fn main() {
        let android_version_env = env::var("ANDROID_VERSION");
        if matches!(android_version_env, Err(VarError::NotUnicode(_))) {
            eprintln!("ANDROID_VERSION is not valid unicode.");
            exit(1);
        }
        let android_version = android_version_env.unwrap_or(ANDROID_VERSION.to_string());

        let command = env::args().nth(1);
        match command.as_deref() {
            None => println!("{}", mullvad_version::VERSION),
            Some("semver") => println!("{}", to_semver(mullvad_version::VERSION)),
            Some("version.h") => println!("{}", to_windows_h_format(mullvad_version::VERSION)),
            Some("versionName") => println!("{android_version}"),
            Some("versionCode") => println!("{}", to_android_version_code(&android_version)),
            Some(command) => {
                eprintln!("Unknown command: {command}");
                exit(1);
            }
        }
    }

    /// Ensures the version string carries a semver patch component.
    ///
    /// Two-component versions (`x.y[-z]`) become `x.y.0[-z]`; versions that
    /// already carry a patch (`x.y.z[-w]`, e.g. `1.0.0`) are returned unchanged.
    fn to_semver(version: &str) -> String {
        let mut parts = version.splitn(2, '-');

        let core = parts.next().expect("version core component");
        let remainder = parts.next().map(|s| format!("-{s}")).unwrap_or_default();
        assert_eq!(parts.next(), None);

        // `core` is either `major.minor` or `major.minor.patch`.
        if core.matches('.').count() >= 2 {
            format!("{core}{remainder}")
        } else {
            format!("{core}.0{remainder}")
        }
    }

    /// Takes a version in the normal Mullvad VPN app version format and returns the Android
    /// `versionCode` formatted version.
    ///
    /// The format of the code is:                    YYVVXZZZ
    ///   Last two digits of the year (major)---------^^
    ///   Incrementing version (minor)------------------^^
    ///   Build type (0=alpha, 1=beta, 9=stable/dev)------^
    ///   Build number (000 if stable/dev)-----------------^^^
    ///
    /// # Examples
    ///
    /// Version: 2021.1-alpha1
    /// versionCode: 21010001
    ///
    /// Version: 2021.34-beta5
    /// versionCode: 21341005
    ///
    /// Version: 2021.34
    /// versionCode: 21349000
    ///
    /// Version: 2021.34-dev
    /// versionCode: 21349000
    fn to_android_version_code(version: &str) -> String {
        let version: Version = version.parse().unwrap();

        // The stable build-number slot doubles as the semver patch: the layout has
        // no field of its own for it, and without this every patch release shares
        // its minor's versionCode, so Android refuses the APK as an upgrade. The
        // calendar versions carry no patch, so their codes are unchanged.
        let patch = version.patch.unwrap_or(0).to_string();

        let (build_type, build_number) = if version.dev.is_some() {
            ("9", patch)
        } else {
            match &version.pre_stable {
                Some(PreStableType::Alpha(v)) => ("0", v.to_string()),
                Some(PreStableType::Beta(v)) => ("1", v.to_string()),
                // Stable version
                None => ("9", patch),
            }
        };

        let major_last_two_digits = version.major % 100;

        format!(
            "{}{:0>2}{}{:0>3}",
            major_last_two_digits, version.minor, build_type, build_number,
        )
    }

    fn to_windows_h_format(version_str: &str) -> String {
        let version: Version = version_str.parse().unwrap();

        let Version {
            major,
            minor,
            patch,
            ..
        } = version;

        format!(
            "#define MAJOR_VERSION {major}
    #define MINOR_VERSION {minor}
    #define PATCH_VERSION {}
    #define PRODUCT_VERSION \"{version_str}\"",
            patch.unwrap_or(0)
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_version_code() {
            assert_eq!("21349000", to_android_version_code("2021.34"));
        }

        #[test]
        fn test_version_code_alpha() {
            assert_eq!("21010001", to_android_version_code("2021.1-alpha1"));
        }

        #[test]
        fn test_version_code_beta() {
            assert_eq!("21341005", to_android_version_code("2021.34-beta5"));
        }

        #[test]
        fn test_version_code_dev() {
            assert_eq!("21349000", to_android_version_code("2021.34-dev-be846a5f0"));
        }

        #[test]
        fn test_windows_version_h() {
            let version_h = to_windows_h_format("2025.4-beta2-dev-abcdef");
            let expected_version_h = "#define MAJOR_VERSION 2025
    #define MINOR_VERSION 4
    #define PATCH_VERSION 0
    #define PRODUCT_VERSION \"2025.4-beta2-dev-abcdef\"";
            assert_eq!(expected_version_h, version_h);
        }

        #[test]
        fn test_windows_version_h_semver() {
            let version_h = to_windows_h_format("1.2.3");
            let expected_version_h = "#define MAJOR_VERSION 1
    #define MINOR_VERSION 2
    #define PATCH_VERSION 3
    #define PRODUCT_VERSION \"1.2.3\"";
            assert_eq!(expected_version_h, version_h);
        }

        #[test]
        fn test_semver_passthrough() {
            // Two-component versions get a `.0` patch appended.
            assert_eq!("2025.4.0", to_semver("2025.4"));
            assert_eq!("2025.4.0-beta2", to_semver("2025.4-beta2"));
            // Versions that already carry a patch are returned unchanged.
            assert_eq!("1.0.0", to_semver("1.0.0"));
            assert_eq!("1.2.3-beta1", to_semver("1.2.3-beta1"));
        }

        #[test]
        fn test_version_code_semver() {
            // 1.0.0 stable: major%100=1, minor=0, stable build type 9, build 000.
            assert_eq!("1009000", to_android_version_code("1.0.0"));
        }

        #[test]
        fn test_version_code_semver_patch_is_monotonic() {
            // A patch release must outrank the release it patches, or Android
            // refuses the APK as an upgrade. The .0 code stays what it was.
            assert_eq!("1019000", to_android_version_code("1.1.0"));
            assert_eq!("1019001", to_android_version_code("1.1.1"));
            assert!(to_android_version_code("1.1.1") > to_android_version_code("1.1.0"));
        }

        #[test]
        fn test_version_code_semver_patch_prerelease_precedes_stable() {
            // A beta of a patch still sorts below the stable it leads to.
            assert!(to_android_version_code("1.1.1-beta1") < to_android_version_code("1.1.1"));
        }
    }
}

#[cfg(not(has_version))]
mod inner {
    pub fn main() {}
}

fn main() {
    inner::main();
}
