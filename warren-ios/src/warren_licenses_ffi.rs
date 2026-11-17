//! What the licences screen shows: the text of the licence the app is
//! distributed under, and the revisions its source links point at. All three
//! are baked into the library when it is built, so the text ships inside the
//! binary and the links name the code that binary was built from.

use std::ffi::c_char;

static LICENSE_TEXT: &str = concat!(include_str!("../../LICENSE.md"), "\0");
static SOURCE_COMMIT: &str = concat!(env!("WARREN_SOURCE_COMMIT"), "\0");
static ENGINE_COMMIT: &str = concat!(env!("WARREN_ENGINE_COMMIT"), "\0");

/// The GNU General Public License, version 3, as the repository ships it.
/// A static NUL-terminated UTF-8 string: never freed.
#[unsafe(no_mangle)]
pub extern "C" fn warren_license_text() -> *const c_char {
    LICENSE_TEXT.as_ptr().cast()
}

/// The commit of this repository the library was built from: 40 lowercase hex
/// characters, or empty when the build had no git checkout. A static
/// NUL-terminated string: never freed.
#[unsafe(no_mangle)]
pub extern "C" fn warren_source_commit() -> *const c_char {
    SOURCE_COMMIT.as_ptr().cast()
}

/// The tunnel engine commit this build pins (`.warrenguard-version`), or
/// empty when the pin file was missing. A static NUL-terminated string: never
/// freed.
#[unsafe(no_mangle)]
pub extern "C" fn warren_engine_commit() -> *const c_char {
    ENGINE_COMMIT.as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    fn read(ptr: *const std::ffi::c_char) -> &'static str {
        // SAFETY: every function under test returns a pointer into a static
        // NUL-terminated UTF-8 string.
        unsafe { CStr::from_ptr(ptr) }.to_str().expect("UTF-8")
    }

    #[test]
    fn the_licence_text_is_the_gpl_the_repository_ships() {
        let text = read(super::warren_license_text());
        let shipped = include_str!("../../LICENSE.md");

        assert_eq!(text, shipped);
        assert!(text.contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(text.contains("Version 3, 29 June 2007"));
    }

    #[test]
    fn the_engine_commit_is_the_pinned_one() {
        let pin = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../.warrenguard-version"
        ))
        .expect("pin file");

        assert_eq!(read(super::warren_engine_commit()), pin.trim());
    }

    #[test]
    fn the_source_commit_is_a_full_commit_or_nothing() {
        let commit = read(super::warren_source_commit());

        assert!(
            commit.is_empty()
                || (commit.len() == 40
                    && commit
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))),
            "{commit:?}"
        );
    }

    /// The build script records the checkout's HEAD, so a build from a git
    /// checkout links the exact code.
    #[test]
    fn a_build_from_a_checkout_records_its_head() {
        let head = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
        let Some(head) = head else {
            // Built outside a checkout: nothing to compare with.
            return;
        };

        assert_eq!(read(super::warren_source_commit()), head);
    }
}
