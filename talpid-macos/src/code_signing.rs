//! Whether a program carries a signature a macOS privacy grant can outlive
//! an update on.
//!
//! A Full Disk Access grant is recorded against the program's designated
//! requirement. For a Developer ID signature that is the team and the
//! identifier, so the grant survives an update; for an ad-hoc signature
//! (every local `cargo build`) it is a hash of the binary, so it is lost at
//! the next build and the split tunnel stops working without a word.

use std::{path::Path, str::FromStr};

use core_foundation::url::CFURL;
use security_framework::os::macos::code_signing::{Flags, SecCode, SecRequirement, SecStaticCode};

/// A certificate chain that ends at Apple's root: Developer ID, App Store,
/// Apple's own code. An ad-hoc or self-signed signature has no such chain.
const APPLE_ANCHORED: &str = "anchor apple generic";

fn apple_anchored() -> Option<SecRequirement> {
    SecRequirement::from_str(APPLE_ANCHORED).ok()
}

/// Whether the running program carries a valid signature anchored at Apple.
/// `false` for an ad-hoc signature, no signature, or a signature that does
/// not verify.
pub fn running_code_is_apple_anchored() -> bool {
    let Some(requirement) = apple_anchored() else {
        return false;
    };
    SecCode::for_self(Flags::NONE)
        .and_then(|code| code.check_validity(Flags::NONE, &requirement))
        .is_ok()
}

/// Whether the program at `path` carries a valid signature anchored at
/// Apple.
pub fn code_at_is_apple_anchored(path: &Path) -> bool {
    let Some(requirement) = apple_anchored() else {
        return false;
    };
    let Some(url) = CFURL::from_path(path, false) else {
        return false;
    };
    SecStaticCode::from_path(&url, Flags::NONE)
        .and_then(|code| code.check_validity(Flags::NONE, &requirement))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The linker signs every arm64 test binary ad hoc.
    #[test]
    fn an_ad_hoc_signed_program_is_not_apple_anchored() {
        let this_test = std::env::current_exe().unwrap();

        assert!(!running_code_is_apple_anchored());
        assert!(!code_at_is_apple_anchored(&this_test));
    }

    #[test]
    fn an_apple_signed_program_is_apple_anchored() {
        assert!(code_at_is_apple_anchored(Path::new("/usr/bin/eslogger")));
    }

    #[test]
    fn a_missing_program_is_not_apple_anchored() {
        assert!(!code_at_is_apple_anchored(Path::new(
            "/nonexistent/program"
        )));
    }
}
