//
//  WarrenRustRuntime.h
//  WarrenRustRuntime
//
//  Originally created by Marco Nikic on 2024-06-20 for the Mullvad fork.
//  Renamed during the Warren rebrand (C.2 + C.4.0 cleanup, 2026-05-21).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Umbrella header for the WarrenRustRuntime framework. Re-exports the
//  cbindgen-generated `warren_rust_runtime.h` C ABI surface (Warren's
//  Rust FFI : warren_wallet_ffi + warren_tunnel_ffi + warren_multihop_ffi +
//  warren_natpmp_ffi). Imported by Swift as `WarrenRustRuntimeProxy` via
//  the private modulemap.
//

#import <Foundation/Foundation.h>
#import "warren_rust_runtime.h"
