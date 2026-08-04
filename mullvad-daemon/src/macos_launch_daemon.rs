//! Provides functions to handle or query the status of the Warren launch
//! daemon/system service on macOS.
//!
//! If the service exists but needs to be approved by the user, this status
//! must be checked so that the user can be directed to approve the launch
//! daemon in the system settings.

use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo, NSURL, ns_string};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Path to the plist that defines the Warren launch daemon. Must stay
/// in sync with the `DAEMON_PLIST_PATH` literal in
/// `dist-assets/pkg-scripts/{pre,post}install` (the postinstall script
/// writes the plist here, the preinstall unloads it). A drift between
/// the two paths surfaces as `LaunchDaemonStatus::NotFound` on every
/// boot - the UI then prompts the user to re-approve a daemon that
/// actually is running, an unrecoverable loop until one path is fixed.
/// Non-prod environments use their own label (matching their bundle id) so a
/// beta daemon registers next to the prod one; their pkg scripts must write
/// the matching plist.
const DAEMON_PLIST_PATH: &str = match warren_product_env::CURRENT {
    warren_product_env::ProductEnv::Prod => {
        "/Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist"
    }
    warren_product_env::ProductEnv::Staging => {
        "/Library/LaunchDaemons/com.warrenbrowse.vpn.staging.daemon.plist"
    }
    warren_product_env::ProductEnv::Beta => {
        "/Library/LaunchDaemons/com.warrenbrowse.vpn.beta.daemon.plist"
    }
};

/// Authorization status of the Warren daemon.
#[repr(i32)]
pub enum LaunchDaemonStatus {
    Ok = 0,
    NotFound = 1,
    NotAuthorized = 2,
    Unknown = 3,
}

/// Return whether the Warren daemon is running, not found, or is not
/// authorized. NOTE: On macos < 13, this function always returns
/// `LaunchDaemonStatus::Ok`.
pub fn get_status() -> LaunchDaemonStatus {
    // `SMAppService` does not exist if the major version is less than 13.
    let os_version = get_os_version();
    if os_version.majorVersion < 13 {
        return LaunchDaemonStatus::Ok;
    }
    // SAFETY: daemon_plist_path is not an empty path & it is a valid system path.
    // https://developer.apple.com/documentation/foundation/nsurl/fileurl(withpath:)#parameters
    let daemon_plist_url = NSURL::fileURLWithPath(ns_string!(DAEMON_PLIST_PATH));
    get_status_for_url(&daemon_plist_url)
}

fn get_status_for_url(url: &NSURL) -> LaunchDaemonStatus {
    // SAFETY: Apple does not state *anything* regarding safety requirements of this function:
    // https://developer.apple.com/documentation/servicemanagement/smappservice/statusforlegacyplist(at:)
    // But using a bit of reasoning & the [guidelines of objc2](https://github.com/madsmtm/objc2/blob/master/crates/header-translator/README.md#what-is-required-for-a-method-to-be-safe):
    // """
    // What is required for a method to be safe?
    // 1. The method must not take a raw pointer; one could trivially pass ptr::invalid() and cause UB with that.
    // 2. Any extra requirements that the method states in its documentation must be upheld.
    // """
    // we can conclude that:
    // (1.) is upheld by the virtue of url being a reference, since references are always valid.
    // (2.) is trivially upheld since Apple does not state safety requirements.
    let status = unsafe { SMAppService::statusForLegacyURL(url) };
    let log_status = || log::debug!("SMAppService::statusForLegacyUrl returned {status:?}");
    match status {
        SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound => {
            log_status();
            LaunchDaemonStatus::NotFound
        }
        SMAppServiceStatus::Enabled => LaunchDaemonStatus::Ok,
        SMAppServiceStatus::RequiresApproval => LaunchDaemonStatus::NotAuthorized,
        // Unknown status
        _ => {
            log_status();
            LaunchDaemonStatus::Unknown
        }
    }
}

fn get_os_version() -> NSOperatingSystemVersion {
    let process_info = NSProcessInfo::processInfo();
    process_info.operatingSystemVersion()
}
