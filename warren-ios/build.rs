use std::path::Path;
use std::process::Command;

fn main() {
    // Any rerun-if-changed line replaces cargo's default of rerunning on any
    // change in the package, so the sources the header is generated from are
    // named too.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    record_source_revisions();

    #[cfg(target_os = "macos")]
    match std::env::var("TARGET").unwrap().as_str() {
        "aarch64-apple-ios" | "aarch64-apple-ios-sim" => {
            let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            cbindgen::Builder::new()
                .with_autogen_warning("// This file is generated automatically. To update it forcefully, run `cargo run -p warren-ios --target aarch64-apple-ios`.")
                .with_crate(crate_dir)
                .with_language(cbindgen::Language::C)
                .generate()
                .expect("failed to generate bindings")
                .write_to_file("../ios/WarrenRustRuntime/include/warren_rust_runtime.h");
        }
        &_ => (),
    }
}

/// The revisions the licences screen links to: the commit this library is
/// built from, and the engine commit `.warrenguard-version` pins. Each is
/// empty when unknown (no git checkout, no pin file), and the screen then
/// links the repository itself.
fn record_source_revisions() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_dir = Path::new(&crate_dir);

    let commit = git(crate_dir, &["rev-parse", "HEAD"])
        .filter(|commit| commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|commit| commit.to_ascii_lowercase())
        .unwrap_or_default();
    println!("cargo:rustc-env=WARREN_SOURCE_COMMIT={commit}");
    // HEAD moves on every commit and checkout; the ref it points at moves on
    // every commit while HEAD itself stays put.
    for path in ["HEAD", "refs/heads"] {
        if let Some(path) = git(crate_dir, &["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={}", crate_dir.join(path).display());
        }
    }

    let pin_path = crate_dir.join("../.warrenguard-version");
    let engine = std::fs::read_to_string(&pin_path)
        .map(|pin| pin.trim().to_owned())
        .unwrap_or_default();
    println!("cargo:rustc-env=WARREN_ENGINE_COMMIT={engine}");
    println!("cargo:rerun-if-changed={}", pin_path.display());
    println!("cargo:rerun-if-changed=../LICENSE.md");
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
