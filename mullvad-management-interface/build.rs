fn main() {
    // Compile both proto files together so they can reference each other
    tonic_build::configure()
        // `Settings` is by far the largest event, and boxing it keeps `DaemonEvent` within
        // clippy's `enum-variant-size-threshold`.
        .boxed(".mullvad_daemon.management_interface.DaemonEvent.event.settings")
        .compile_protos(
            &[
                "proto/management_interface.proto",
                "proto/relay_selector.proto",
            ],
            &["proto/"],
        )
        .unwrap();

    // Enable DAITA by default on desktop and android
    println!("cargo::rustc-check-cfg=cfg(daita)");
    println!(r#"cargo::rustc-cfg=daita"#);
}
