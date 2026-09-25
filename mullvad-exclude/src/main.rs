fn main() {
    #[cfg(target_os = "linux")]
    mullvad_exclude::main(mullvad_exclude::Launch::Exclude);
}
