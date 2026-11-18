fn main() {
    tonic_build::configure()
        // The settings dwarf every other event, so the event enum carries
        // them boxed rather than sized to them.
        .boxed(".mullvad_daemon.management_interface.DaemonEvent.event.settings")
        .compile_protos(
            &["proto/management_interface.proto"],
            &["proto/"],
        )
        .unwrap();
}
