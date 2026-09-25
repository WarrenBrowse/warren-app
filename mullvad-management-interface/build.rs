fn main() {
    // Compile both proto files together so they can reference each other
    tonic_build::configure()
        // The settings dwarf every other event, so the event enum carries
        // them boxed rather than sized to them.
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
