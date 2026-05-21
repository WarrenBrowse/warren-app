// Warren VPN Android JNI bridge - crate root.
//
// The crate is laid out as two modules:
//
//   - [`wallet`] - pure-rust BIP39 + Ed25519 primitives wrapping
//     `warren-identity`. Always compiled, unit-tested on host.
//   - [`android_jni`] (target_os = "android" only) - the actual JNI exports
//     (`Java_com_warrenbrowse_vpn_jni_WarrenJni_*`). Calls into `wallet`
//     for the mnemonic / signing surface; stubs out the tunnel surface
//     until D.4 lands `warren_tunnel::PacketDevice::from_fd`.

pub mod wallet;

#[cfg(target_os = "android")]
mod android_jni;
