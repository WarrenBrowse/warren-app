//! Warren wallet FFI for iOS (BIP39 mnemonic + Ed25519 signing).
//!
//! Skeleton: the implementations land in the C.3 deep-work follow-up,
//! once the warren-core path-dep wiring is in place. The Swift side
//! (`ios/WarrenRustRuntime/Sources/WarrenRustRuntime/`) will wrap these
//! exports behind idiomatic Swift APIs (e.g. `WarrenWallet.generate()`,
//! `WarrenWallet.fromMnemonic(_:)`, `WarrenWallet.signRequest(_:)`).
//!
//! Intended exports:
//! - `warren_wallet_generate_mnemonic() -> *mut c_char` (12-word BIP39)
//! - `warren_wallet_mnemonic_to_seed(mnemonic, out_seed: [u8; 32])`
//! - `warren_wallet_derive_pubkey(seed: [u8; 32], out_pubkey: [u8; 32])`
//! - `warren_wallet_sign_canonical_message(seed, msg, msg_len, out_sig: [u8; 64])`
//!
//! Underlying crates (path-deps to add when wiring):
//! - `warren-identity` (BIP39 + HKDF + Ed25519 + canonical-message signing)
//! - `warren-config` (HKDF salt / info constants)
//!
//! Storage: iOS Keychain is owned by the Swift side
//! (`kSecClassGenericPassword` + `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`).
//! Rust only sees the mnemonic transiently and zeroizes its buffers.
