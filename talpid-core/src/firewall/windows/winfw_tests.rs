//! winfw against the real filtering engine.
//!
//! Ignored by default: they need an elevated process and a `winfw.dll` on the
//! DLL search path, and they briefly apply a blocked policy to the machine
//! (LAN allowed, so a remote session over the LAN survives it). Run them on a
//! Windows test machine with
//! `cargo test -p talpid-core --lib winfw_tests -- --ignored --test-threads=1`,
//! against a winfw built for a non-production environment: in production
//! the private and shared sublayer keys only differ by the private flip.

use std::{
    ffi::OsString,
    io,
    net::{SocketAddr, TcpStream},
    ptr,
    sync::Mutex,
    time::Duration,
};

use windows_sys::{
    Win32::{
        Foundation::{FWP_E_SUBLAYER_NOT_FOUND, HANDLE},
        NetworkManagement::WindowsFilteringPlatform::*,
        System::Rpc::RPC_C_AUTHN_DEFAULT,
    },
    core::GUID,
};

use super::winfw::{self, WinFwCleanupPolicy, WinFwSettings};

/// winfw is one context per process.
static WINFW: Mutex<()> = Mutex::new(());

/// The keys the split tunnel driver adds its filters to
/// (win-split-tunnel, firewall/identifiers.h).
const SHARED_BASELINE: GUID = GUID::from_u128(0xc78056ff_2bc1_4211_aadd_7f358def202d);
const SHARED_DNS: GUID = GUID::from_u128(0x60090787_cca1_4937_aace_51256ef481f3);

/// A provider standing for another product environment.
const FOREIGN_PROVIDER: GUID = GUID::from_u128(0x5e1f0a57_1c2d_4b8e_9f00_7e57c0ffee01);

/// Not routed anywhere, and inside the private ranges the LAN permit covers.
const LAN_PROBE: &str = "10.255.255.1:9";

fn our_provider() -> GUID {
    let base = GUID::from_u128(0x21e1dab8_b9db_43c0_b343_eb9365c7bdd2);
    GUID {
        data1: base.data1 ^ warren_product_env::CURRENT.guid_salt(),
        ..base
    }
}

/// A dynamic session: whatever the test adds through it is gone once it
/// is dropped.
struct Engine(HANDLE);

impl Engine {
    fn open() -> Self {
        let session = FWPM_SESSION0 {
            flags: FWPM_SESSION_FLAG_DYNAMIC,
            ..Default::default()
        };
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: every pointer is valid for the call.
        let status = unsafe {
            FwpmEngineOpen0(
                ptr::null(),
                RPC_C_AUTHN_DEFAULT as u32,
                ptr::null(),
                &raw const session,
                &raw mut handle,
            )
        };
        assert_eq!(status, 0, "FwpmEngineOpen0");
        Engine(handle)
    }

    /// The provider owning the sublayer at `key`: `None` when there is no
    /// such sublayer, `Some(None)` when it belongs to no provider.
    fn sublayer_owner(&self, key: GUID) -> Option<Option<u128>> {
        let mut sublayer: *mut FWPM_SUBLAYER0 = ptr::null_mut();
        // SAFETY: valid handle and out pointer.
        let status = unsafe { FwpmSubLayerGetByKey0(self.0, &raw const key, &raw mut sublayer) };
        if status == FWP_E_SUBLAYER_NOT_FOUND as u32 {
            return None;
        }
        assert_eq!(status, 0, "FwpmSubLayerGetByKey0");
        // SAFETY: the engine returned a valid sublayer, freed right after.
        let owner = unsafe {
            let owner = (*sublayer).providerKey.as_ref().copied().map(id);
            FwpmFreeMemory0((&raw mut sublayer).cast());
            owner
        };
        Some(owner)
    }

    /// The sublayer of every filter `provider` owns.
    fn sublayers_of_filters(&self, provider: GUID) -> Vec<u128> {
        let mut found = Vec::new();
        let mut enum_handle: HANDLE = ptr::null_mut();
        // SAFETY: valid handle and out pointer.
        let status =
            unsafe { FwpmFilterCreateEnumHandle0(self.0, ptr::null(), &raw mut enum_handle) };
        assert_eq!(status, 0, "FwpmFilterCreateEnumHandle0");
        loop {
            let mut entries: *mut *mut FWPM_FILTER0 = ptr::null_mut();
            let mut returned = 0u32;
            // SAFETY: valid handles and out pointers.
            let status = unsafe {
                FwpmFilterEnum0(
                    self.0,
                    enum_handle,
                    100,
                    &raw mut entries,
                    &raw mut returned,
                )
            };
            assert_eq!(status, 0, "FwpmFilterEnum0");
            for i in 0..returned as usize {
                // SAFETY: the engine returned `returned` valid entries.
                let filter = unsafe { &**entries.add(i) };
                // SAFETY: a non-null provider key points to a GUID.
                if unsafe { filter.providerKey.as_ref() }.copied().map(id) == Some(id(provider)) {
                    found.push(id(filter.subLayerKey));
                }
            }
            if !entries.is_null() {
                // SAFETY: allocated by the engine.
                unsafe { FwpmFreeMemory0((&raw mut entries).cast()) };
            }
            if returned < 100 {
                break;
            }
        }
        // SAFETY: valid handles.
        unsafe { FwpmFilterDestroyEnumHandle0(self.0, enum_handle) };
        found
    }

    /// The names of the filters `provider` owns.
    fn filter_names(&self, provider: GUID) -> Vec<String> {
        let mut names = Vec::new();
        let mut enum_handle: HANDLE = ptr::null_mut();
        // SAFETY: valid handle and out pointer.
        let status =
            unsafe { FwpmFilterCreateEnumHandle0(self.0, ptr::null(), &raw mut enum_handle) };
        assert_eq!(status, 0, "FwpmFilterCreateEnumHandle0");
        loop {
            let mut entries: *mut *mut FWPM_FILTER0 = ptr::null_mut();
            let mut returned = 0u32;
            // SAFETY: valid handles and out pointers.
            let status = unsafe {
                FwpmFilterEnum0(
                    self.0,
                    enum_handle,
                    100,
                    &raw mut entries,
                    &raw mut returned,
                )
            };
            assert_eq!(status, 0, "FwpmFilterEnum0");
            for i in 0..returned as usize {
                // SAFETY: the engine returned `returned` valid entries.
                let filter = unsafe { &**entries.add(i) };
                // SAFETY: a non-null provider key points to a GUID.
                if unsafe { filter.providerKey.as_ref() }.copied().map(id) == Some(id(provider))
                    && !filter.displayData.name.is_null()
                {
                    // SAFETY: a filter's non-null name is a null-terminated wide string.
                    let name = unsafe {
                        widestring::U16CStr::from_ptr_str(filter.displayData.name.cast_const())
                    };
                    names.push(name.to_string_lossy());
                }
            }
            if !entries.is_null() {
                // SAFETY: allocated by the engine.
                unsafe { FwpmFreeMemory0((&raw mut entries).cast()) };
            }
            if returned < 100 {
                break;
            }
        }
        // SAFETY: valid handles.
        unsafe { FwpmFilterDestroyEnumHandle0(self.0, enum_handle) };
        names
    }

    fn add_foreign_provider(&self) {
        let mut name = wide("winfw test: another environment");
        let provider = FWPM_PROVIDER0 {
            providerKey: FOREIGN_PROVIDER,
            displayData: FWPM_DISPLAY_DATA0 {
                name: name.as_mut_ptr(),
                description: ptr::null_mut(),
            },
            ..Default::default()
        };
        // SAFETY: valid handle and provider.
        let status = unsafe { FwpmProviderAdd0(self.0, &raw const provider, ptr::null_mut()) };
        assert_eq!(status, 0, "FwpmProviderAdd0");
    }

    /// The shared baseline sublayer as a sharing build creates it: owned by
    /// no provider.
    fn add_shared_baseline(&self) {
        let mut name = wide("winfw test: shared baseline");
        let sublayer = FWPM_SUBLAYER0 {
            subLayerKey: SHARED_BASELINE,
            displayData: FWPM_DISPLAY_DATA0 {
                name: name.as_mut_ptr(),
                description: ptr::null_mut(),
            },
            weight: u16::MAX,
            ..Default::default()
        };
        // SAFETY: valid handle and sublayer.
        let status = unsafe { FwpmSubLayerAdd0(self.0, &raw const sublayer, ptr::null_mut()) };
        assert_eq!(status, 0, "FwpmSubLayerAdd0");
    }

    /// An inert filter of the foreign provider in `sublayer`: it only
    /// permits what goes to port 9, which nothing here uses.
    fn add_foreign_filter(&self, sublayer: GUID) {
        let mut name = wide("winfw test: foreign filter");
        let mut condition = FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_IP_REMOTE_PORT,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0 {
                r#type: FWP_UINT16,
                Anonymous: FWP_CONDITION_VALUE0_0 { uint16: 9 },
            },
        };
        let mut provider = FOREIGN_PROVIDER;
        let filter = FWPM_FILTER0 {
            displayData: FWPM_DISPLAY_DATA0 {
                name: name.as_mut_ptr(),
                description: ptr::null_mut(),
            },
            providerKey: &raw mut provider,
            layerKey: FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            subLayerKey: sublayer,
            weight: FWP_VALUE0 {
                r#type: FWP_EMPTY,
                ..Default::default()
            },
            numFilterConditions: 1,
            filterCondition: &raw mut condition,
            action: FWPM_ACTION0 {
                r#type: FWP_ACTION_PERMIT,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut id = 0u64;
        // SAFETY: valid handle; every pointer in `filter` outlives the call.
        let status =
            unsafe { FwpmFilterAdd0(self.0, &raw const filter, ptr::null_mut(), &raw mut id) };
        assert_eq!(status, 0, "FwpmFilterAdd0");
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the handle came from FwpmEngineOpen0.
        unsafe { FwpmEngineClose0(self.0) };
    }
}

/// windows-sys GUIDs do not compare, their `u128` form does.
fn id(guid: GUID) -> u128 {
    let mut tail = 0u128;
    for byte in guid.data4 {
        tail = (tail << 8) | u128::from(byte);
    }
    (u128::from(guid.data1) << 96)
        | (u128::from(guid.data2) << 80)
        | (u128::from(guid.data3) << 64)
        | tail
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Initializes winfw and tears it down (policy reset, no block left
/// behind) when dropped, even when the test fails.
struct Winfw;

impl Winfw {
    fn init() -> Self {
        winfw::initialize().expect("winfw initializes");
        Winfw
    }

    fn block_with_lan(&self) {
        winfw::apply_policy_blocked(&WinFwSettings::new(true), None).expect("blocked policy");
    }
}

impl Drop for Winfw {
    fn drop(&mut self) {
        let _ = winfw::set_included_apps(&[]);
        let _ = winfw::reset();
        let _ = winfw::deinit(WinFwCleanupPolicy::ResetFirewall);
    }
}

fn blocked_by_the_filtering_engine(addr: &str) -> bool {
    let addr: SocketAddr = addr.parse().unwrap();
    match TcpStream::connect_timeout(&addr, Duration::from_millis(1500)) {
        Err(error) => error.kind() == io::ErrorKind::PermissionDenied,
        Ok(_) => false,
    }
}

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a blocked policy"]
fn the_baseline_and_dns_filters_share_the_sublayers_the_driver_adds_its_filters_to() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::open();
    let fw = Winfw::init();

    fw.block_with_lan();

    let sublayers = engine.sublayers_of_filters(our_provider());
    assert!(
        sublayers.contains(&id(SHARED_BASELINE)),
        "no filter in the shared baseline sublayer"
    );
    assert!(
        sublayers.contains(&id(SHARED_DNS)),
        "no filter in the shared DNS sublayer"
    );
    assert_eq!(engine.sublayer_owner(SHARED_BASELINE), Some(None));
    assert!(winfw::split_tunnel_sublayers_shared());

    drop(fw);
    assert_eq!(
        engine.sublayer_owner(SHARED_BASELINE),
        None,
        "left behind unused"
    );
}

#[test]
#[ignore = "needs an elevated process and winfw.dll"]
fn the_shared_sublayers_carry_our_claim_between_policies() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::open();
    let _fw = Winfw::init();

    let sublayers = engine.sublayers_of_filters(our_provider());

    assert!(
        sublayers.contains(&id(SHARED_BASELINE)),
        "baseline claim missing"
    );
    assert!(sublayers.contains(&id(SHARED_DNS)), "DNS claim missing");
}

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a blocked policy"]
fn a_sweep_of_other_environments_leaves_the_live_shared_sublayers_alone() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::open();
    let fw = Winfw::init();
    let others: Vec<u32> = warren_product_env::ALL
        .iter()
        .filter(|env| **env != warren_product_env::CURRENT)
        .map(|env| env.guid_salt())
        .collect();

    winfw::sweep_foreign_generations(&others).expect("sweep");
    let blocked = winfw::apply_policy_blocked(&WinFwSettings::new(true), None);

    assert!(
        blocked.is_ok(),
        "the sweep left the policy without its sublayers"
    );
    assert!(
        engine
            .sublayers_of_filters(our_provider())
            .contains(&id(SHARED_BASELINE))
    );
    drop(fw);
}

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a blocked policy"]
fn a_live_foreign_policy_keeps_the_shared_sublayers_to_itself() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::open();
    engine.add_foreign_provider();
    engine.add_shared_baseline();
    engine.add_foreign_filter(SHARED_BASELINE);
    let fw = Winfw::init();

    fw.block_with_lan();

    assert!(!winfw::split_tunnel_sublayers_shared());
    let sublayers = engine.sublayers_of_filters(our_provider());
    assert!(
        !sublayers.is_empty(),
        "the blocked policy installed nothing"
    );
    assert!(
        !sublayers.contains(&id(SHARED_BASELINE)) && !sublayers.contains(&id(SHARED_DNS)),
        "our policy was mixed into a foreign one"
    );

    drop(fw);
    assert_eq!(
        engine.sublayers_of_filters(FOREIGN_PROVIDER),
        vec![id(SHARED_BASELINE)]
    );
}

#[test]
#[ignore = "needs an elevated process and winfw.dll"]
fn teardown_leaves_a_shared_sublayer_another_filter_still_uses() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::open();
    engine.add_foreign_provider();
    winfw::initialize().expect("winfw initializes");
    engine.add_foreign_filter(SHARED_BASELINE);

    let reset = winfw::reset();
    let deinit = winfw::deinit(WinFwCleanupPolicy::ResetFirewall);

    assert!(
        reset.is_ok() && deinit.is_ok(),
        "teardown failed on the shared sublayer"
    );
    assert_eq!(engine.sublayer_owner(SHARED_BASELINE), Some(None));
    assert_eq!(
        engine.sublayers_of_filters(FOREIGN_PROVIDER),
        vec![id(SHARED_BASELINE)]
    );

    // The foreign filter goes with the session; the next initialization or
    // teardown removes the sublayer it leaves unused.
    drop(engine);
    drop(Winfw::init());
    assert_eq!(Engine::open().sublayer_owner(SHARED_BASELINE), None);
}

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a blocked policy"]
fn an_included_app_is_held_off_what_the_policy_permits_outside_the_tunnel() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let fw = Winfw::init();
    fw.block_with_lan();
    assert!(
        !blocked_by_the_filtering_engine(LAN_PROBE),
        "the LAN permit should let the probe out"
    );

    let this_test: OsString = std::env::current_exe().unwrap().into();
    winfw::set_included_apps(&[this_test]).expect("hold installed");
    let held = blocked_by_the_filtering_engine(LAN_PROBE);
    winfw::set_included_apps(&[]).expect("hold lifted");
    let released = !blocked_by_the_filtering_engine(LAN_PROBE);

    assert!(held, "an included app reached the LAN outside the tunnel");
    assert!(released, "the hold outlived the included apps");
}

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a blocked policy"]
fn a_process_named_by_its_device_path_is_held_too() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let fw = Winfw::init();
    fw.block_with_lan();

    winfw::set_included_apps(&[device_path(&std::env::current_exe().unwrap())])
        .expect("hold installed");

    assert!(
        blocked_by_the_filtering_engine(LAN_PROBE),
        "the driver's device path of an included process did not hold it"
    );
}

/// `C:\dir\x.exe` as the split tunnel driver names it,
/// `\Device\HarddiskVolumeN\dir\x.exe`.
fn device_path(path: &std::path::Path) -> OsString {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::QueryDosDeviceW;

    let text: Vec<u16> = path.as_os_str().encode_wide().collect();
    let drive = [text[0], u16::from(b':'), 0];
    let mut target = vec![0u16; 1024];
    // SAFETY: `drive` is null-terminated and `target` writable for its length.
    let len = unsafe { QueryDosDeviceW(drive.as_ptr(), target.as_mut_ptr(), target.len() as u32) };
    assert_ne!(len, 0, "QueryDosDeviceW");
    let end = target.iter().position(|c| *c == 0).unwrap();
    let mut device = target[..end].to_vec();
    device.extend_from_slice(&text[2..]);
    OsString::from_wide(&device)
}

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a blocked policy"]
fn an_included_app_that_does_not_resolve_holds_nothing_back() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let fw = Winfw::init();
    fw.block_with_lan();

    winfw::set_included_apps(&[OsString::from(r"C:\warren-winfw-test\absent.exe")])
        .expect("hold installed");

    assert!(
        !blocked_by_the_filtering_engine(LAN_PROBE),
        "a hold with no app to match blocked every app"
    );
}

/// A connected policy on the loopback interface, standing in for the tunnel.
fn apply_connected(include_only: bool) {
    use talpid_types::net::{AllowedClients, AllowedEndpoint, Endpoint, TransportProtocol};

    let relay = AllowedEndpoint {
        endpoint: Endpoint {
            address: "192.0.2.1:443".parse().unwrap(),
            protocol: TransportProtocol::Udp,
        },
        clients: AllowedClients::from(vec![std::env::current_exe().unwrap()]),
    };
    let dns = talpid_dns::DnsConfig::default().resolve(&["10.64.0.1".parse().unwrap()]);
    winfw::apply_policy_connected(
        &[relay],
        None,
        &WinFwSettings::connected(true, include_only),
        "Loopback Pseudo-Interface 1",
        &dns,
    )
    .expect("connected policy");
}

const RESOLVER_BLOCK: &str = "Block the system resolver's encrypted DNS outside the tunnel";

#[test]
#[ignore = "needs an elevated process and winfw.dll; applies a connected policy"]
fn include_only_keeps_the_system_resolvers_encrypted_dns_in_the_tunnel() {
    let _lock = WINFW.lock().unwrap_or_else(|poison| poison.into_inner());
    let engine = Engine::open();
    let _fw = Winfw::init();

    apply_connected(false);
    let full_tunnel = engine.filter_names(our_provider());
    apply_connected(true);
    let include_only = engine.filter_names(our_provider());

    assert_eq!(
        include_only
            .iter()
            .filter(|name| *name == RESOLVER_BLOCK)
            .count(),
        2,
        "no IPv4 and IPv6 block of the system resolver under include-only"
    );
    assert!(
        !full_tunnel.iter().any(|name| name == RESOLVER_BLOCK),
        "the full tunnel has nothing outside the tunnel to keep it from"
    );
}
