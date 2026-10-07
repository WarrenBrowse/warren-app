//! `warren-setup reset-firewall` is what the uninstaller and the deadman run to
//! lift a block no daemon will lift any more, and both read its exit status.
#![cfg(windows)]

use std::process::Command;

#[test]
#[ignore = "elevated, with no daemon running: removes every Warren WFP object on the machine"]
fn reset_firewall_exits_cleanly() {
    let status = Command::new(env!("CARGO_BIN_EXE_warren-setup"))
        .arg("reset-firewall")
        .status()
        .expect("warren-setup should start");

    assert!(
        status.success(),
        "warren-setup reset-firewall exited with {status}"
    );
}
