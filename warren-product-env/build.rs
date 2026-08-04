//! Validates the `WARREN_PRODUCT_ENV` build-time selector and forwards it to
//! the crate as a rustc env so `lib.rs` can resolve it at compile time.
//! Unset or empty means prod, so a plain `cargo build` is always a prod build.

fn main() {
    println!("cargo:rerun-if-env-changed=WARREN_PRODUCT_ENV");

    let raw = std::env::var("WARREN_PRODUCT_ENV").unwrap_or_default();
    let resolved = match raw.trim() {
        "" | "prod" => "prod",
        "staging" => "staging",
        "beta" => "beta",
        other => panic!(
            "WARREN_PRODUCT_ENV must be one of prod|staging|beta (or unset for prod), got {other:?}"
        ),
    };
    println!("cargo:rustc-env=WARREN_PRODUCT_ENV_RESOLVED={resolved}");
}
