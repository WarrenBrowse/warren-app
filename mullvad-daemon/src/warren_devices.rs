//! The devices of the logged-in account (warren-core doc 115), as the GUI
//! lists, renames and revokes them from one another: the wallet-signed
//! `GET /v1/devices`, `PATCH` and `DELETE /v1/devices/{id}`. The list names the account's installs and the days
//! they were used, so it goes to the account's own screens and never to a
//! log: neither a name nor an identifier is logged here.

use mullvad_management_interface::types;
use warren_api::{ClientError, DeviceId, DeviceListResponse, DevicePlatform, DeviceView};

use crate::warren_sdk_client::SharedWarrenApiClient;

/// Why the list or a revocation did not go through, in the terms the GUI
/// acts on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DevicesError {
    /// No wallet is logged in, so nothing can sign the request.
    #[error("no wallet is logged in")]
    NoWallet,
    /// The device id is not one the registry could have issued.
    #[error("the device id is malformed")]
    InvalidDevice,
    /// The name is not one the registry keeps: 1 to 40 characters once
    /// trimmed, none of them a control character.
    #[error("the device name is not 1 to 40 characters without control characters")]
    InvalidName,
    /// The account has no such active device (404).
    #[error("the account has no such device")]
    NotFound,
    /// This device was revoked from another device of the account.
    #[error("this device was revoked")]
    Revoked,
    /// The API could not be reached or answered otherwise; a retry can work.
    #[error("the device registry could not be reached, retry later")]
    Unavailable,
}

/// Maps the SDK error of a device route to the verdict the GUI shows. Only
/// the status and the typed refusal are read: a body is never logged.
pub fn classify(error: &ClientError) -> DevicesError {
    match error {
        ClientError::DeviceRevoked => DevicesError::Revoked,
        ClientError::ServerStatus { status: 404, .. } => DevicesError::NotFound,
        ClientError::ServerStatus { status: 400, .. } => DevicesError::InvalidDevice,
        _ => DevicesError::Unavailable,
    }
}

/// The platform as the GUI reads it, the registry's own spelling.
fn platform_name(platform: DevicePlatform) -> &'static str {
    match platform {
        DevicePlatform::Android => "android",
        DevicePlatform::Ios => "ios",
        DevicePlatform::Macos => "macos",
        DevicePlatform::Windows => "windows",
        DevicePlatform::Linux => "linux",
        DevicePlatform::Cli => "cli",
        DevicePlatform::Browser => "browser",
        _ => "other",
    }
}

fn device(view: DeviceView) -> types::Device {
    types::Device {
        id: String::from(view.device_id),
        name: view.name,
        platform: platform_name(view.platform).to_owned(),
        app_version: view.app_version,
        registered_on: view.registered_on,
        last_seen_on: view.last_seen_on,
        this_device: view.this_device,
        ..Default::default()
    }
}

/// The registry's answer as the management interface carries it.
pub fn device_list(response: DeviceListResponse) -> types::DeviceList {
    types::DeviceList {
        devices: response.devices.into_iter().map(device).collect(),
        older_client_seen_on: response.older_client_seen_on,
    }
}

/// The account's devices, this one (`this`) marked.
pub async fn list(
    client: &SharedWarrenApiClient,
    this: Option<DeviceId>,
) -> Result<DeviceListResponse, DevicesError> {
    if client.wallet().is_none() {
        return Err(DevicesError::NoWallet);
    }
    client
        .list_devices(this)
        .await
        .map_err(|error| classify(&error))
}

/// The device `id` names, or why it names none.
pub fn device_id(id: &str) -> Result<DeviceId, DevicesError> {
    DeviceId::try_from(id.to_owned()).map_err(|_| DevicesError::InvalidDevice)
}

/// Revokes the account's device `id`. Answers whether it was this device
/// (`this`), which is a logout: the caller then forgets its identity.
pub async fn revoke(
    client: &SharedWarrenApiClient,
    id: &str,
    this: Option<DeviceId>,
) -> Result<bool, DevicesError> {
    if client.wallet().is_none() {
        return Err(DevicesError::NoWallet);
    }
    let id = device_id(id)?;
    client
        .revoke_device(id)
        .await
        .map_err(|error| classify(&error))?;
    Ok(this == Some(id))
}

/// The longest name the registry keeps, in characters.
pub const DEVICE_NAME_MAX_CHARS: usize = 40;

/// The name as the registry keeps it (trimmed), or why it keeps none.
pub fn device_name(name: &str) -> Result<String, DevicesError> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > DEVICE_NAME_MAX_CHARS || name.chars().any(char::is_control) {
        return Err(DevicesError::InvalidName);
    }
    Ok(name.to_owned())
}

/// Renames the account's device `id`. A name the registry would refuse is
/// refused here, before any request.
pub async fn rename(
    client: &SharedWarrenApiClient,
    id: &str,
    name: &str,
) -> Result<(), DevicesError> {
    if client.wallet().is_none() {
        return Err(DevicesError::NoWallet);
    }
    let id = device_id(id)?;
    let name = device_name(name)?;
    client
        .rename_device(id, &name)
        .await
        .map_err(|error| classify(&error))
}

#[cfg(test)]
mod tests {
    use warren_api::DEVICE_ID_LEN;

    use super::*;

    fn status(status: u16) -> ClientError {
        ClientError::ServerStatus {
            status,
            body: String::from("never read"),
        }
    }

    fn view(byte: u8, platform: DevicePlatform, this_device: bool) -> DeviceView {
        DeviceView {
            device_id: DeviceId::from_bytes([byte; DEVICE_ID_LEN]),
            name: format!("device {byte}"),
            platform,
            app_version: "1.1.55".to_owned(),
            registered_on: "2026-10-01".to_owned(),
            last_seen_on: "2026-10-28".to_owned(),
            this_device,
        }
    }

    #[test]
    fn a_device_the_account_does_not_have_is_not_found() {
        assert_eq!(classify(&status(404)), DevicesError::NotFound);
    }

    #[test]
    fn a_malformed_device_id_is_invalid() {
        assert_eq!(classify(&status(400)), DevicesError::InvalidDevice);
        assert_eq!(device_id("not a device"), Err(DevicesError::InvalidDevice));
    }

    #[test]
    fn a_revoked_caller_is_told_so() {
        assert_eq!(classify(&ClientError::DeviceRevoked), DevicesError::Revoked);
    }

    #[test]
    fn any_other_failure_is_worth_a_retry() {
        assert_eq!(classify(&status(503)), DevicesError::Unavailable);
        assert_eq!(classify(&status(403)), DevicesError::Unavailable);
    }

    #[test]
    fn a_listed_device_carries_its_platform_version_days_and_whether_it_is_this_one() {
        let listed = device_list(DeviceListResponse {
            devices: vec![
                view(1, DevicePlatform::Macos, true),
                view(2, DevicePlatform::Android, false),
            ],
            older_client_seen_on: Some("2026-10-27".to_owned()),
        });

        assert_eq!(listed.devices.len(), 2);
        let mine = &listed.devices[0];
        assert_eq!(
            mine.id,
            String::from(DeviceId::from_bytes([1; DEVICE_ID_LEN]))
        );
        assert_eq!(mine.name, "device 1");
        assert_eq!(mine.platform, "macos");
        assert_eq!(mine.app_version, "1.1.55");
        assert_eq!(mine.registered_on, "2026-10-01");
        assert_eq!(mine.last_seen_on, "2026-10-28");
        assert!(mine.this_device);
        assert_eq!(listed.devices[1].platform, "android");
        assert!(!listed.devices[1].this_device);
        assert_eq!(listed.older_client_seen_on.as_deref(), Some("2026-10-27"));
    }

    #[test]
    fn every_platform_keeps_the_registrys_spelling() {
        for (platform, name) in [
            (DevicePlatform::Android, "android"),
            (DevicePlatform::Ios, "ios"),
            (DevicePlatform::Macos, "macos"),
            (DevicePlatform::Windows, "windows"),
            (DevicePlatform::Linux, "linux"),
            (DevicePlatform::Cli, "cli"),
            (DevicePlatform::Browser, "browser"),
            (DevicePlatform::Other, "other"),
        ] {
            assert_eq!(platform_name(platform), name);
            if platform != DevicePlatform::Other {
                assert_eq!(
                    serde_json::to_value(platform).unwrap(),
                    serde_json::Value::from(name)
                );
            }
        }
    }

    /// A client of a real wallet against `server`.
    fn client_of(server: &mockito::ServerGuard) -> SharedWarrenApiClient {
        SharedWarrenApiClient::new(
            server.url(),
            std::sync::Arc::new(std::sync::RwLock::new(zeroize::Zeroizing::new([0x51; 32]))),
        )
    }

    fn id(byte: u8) -> DeviceId {
        DeviceId::from_bytes([byte; DEVICE_ID_LEN])
    }

    #[tokio::test]
    async fn revoking_this_device_says_so_and_another_does_not() {
        let mut server = mockito::Server::new_async().await;
        let path = |byte| format!("/v1/devices/{}", String::from(id(byte)));
        let mine = server
            .mock("DELETE", path(1).as_str())
            .with_status(204)
            .create();
        let other = server
            .mock("DELETE", path(2).as_str())
            .with_status(204)
            .create();
        let client = client_of(&server);

        assert_eq!(
            revoke(&client, &String::from(id(1)), Some(id(1))).await,
            Ok(true)
        );
        assert_eq!(
            revoke(&client, &String::from(id(2)), Some(id(1))).await,
            Ok(false)
        );
        mine.assert();
        other.assert();
    }

    #[tokio::test]
    async fn a_revocation_the_registry_refuses_names_why() {
        let mut server = mockito::Server::new_async().await;
        let _gone = server
            .mock("DELETE", mockito::Matcher::Any)
            .with_status(404)
            .create();

        assert_eq!(
            revoke(&client_of(&server), &String::from(id(3)), None).await,
            Err(DevicesError::NotFound)
        );
    }

    #[tokio::test]
    async fn a_malformed_id_is_refused_before_any_request() {
        let mut server = mockito::Server::new_async().await;
        let never = server
            .mock("DELETE", mockito::Matcher::Any)
            .expect(0)
            .create();

        assert_eq!(
            revoke(&client_of(&server), "short", None).await,
            Err(DevicesError::InvalidDevice)
        );
        never.assert();
    }

    #[tokio::test]
    async fn nothing_is_asked_without_a_wallet() {
        let mut server = mockito::Server::new_async().await;
        let never = server.mock("GET", mockito::Matcher::Any).expect(0).create();
        let logged_out = SharedWarrenApiClient::new(
            server.url(),
            std::sync::Arc::new(std::sync::RwLock::new(
                crate::warren_sdk_client::sentinel_seed(),
            )),
        );

        assert_eq!(list(&logged_out, None).await, Err(DevicesError::NoWallet));
        never.assert();
    }

    #[tokio::test]
    async fn the_list_marks_this_device_through_the_query() {
        let mut server = mockito::Server::new_async().await;
        let listed = server
            .mock("GET", "/v1/devices")
            .match_query(mockito::Matcher::UrlEncoded(
                "device_id".into(),
                String::from(id(1)),
            ))
            .with_status(200)
            .with_body(r#"{"devices":[]}"#)
            .create();

        let answer = list(&client_of(&server), Some(id(1))).await;

        assert_eq!(answer.map(|list| list.devices.len()), Ok(0));
        listed.assert();
    }

    #[tokio::test]
    async fn a_rename_sends_the_trimmed_name_for_that_device() {
        let mut server = mockito::Server::new_async().await;
        let renamed = server
            .mock(
                "PATCH",
                format!("/v1/devices/{}", String::from(id(4))).as_str(),
            )
            .match_body(mockito::Matcher::Json(
                serde_json::json!({"name": "Blue fox"}),
            ))
            .with_status(204)
            .create();

        assert_eq!(
            rename(&client_of(&server), &String::from(id(4)), "  Blue fox ").await,
            Ok(())
        );
        renamed.assert();
    }

    #[tokio::test]
    async fn a_name_the_registry_would_refuse_is_refused_before_any_request() {
        let mut server = mockito::Server::new_async().await;
        let never = server
            .mock("PATCH", mockito::Matcher::Any)
            .expect(0)
            .create();
        let client = client_of(&server);
        let device = String::from(id(5));

        for name in ["", "   ", &"x".repeat(41), "tab\there", "bell\u{7}"] {
            assert_eq!(
                rename(&client, &device, name).await,
                Err(DevicesError::InvalidName),
                "{name:?}"
            );
        }
        never.assert();
    }

    #[test]
    fn a_name_of_forty_characters_of_any_script_is_kept() {
        assert_eq!(device_name(&"\u{e9}".repeat(40)), Ok("\u{e9}".repeat(40)));
    }

    #[tokio::test]
    async fn a_rename_the_registry_refuses_names_why() {
        let mut server = mockito::Server::new_async().await;
        let _gone = server
            .mock("PATCH", mockito::Matcher::Any)
            .with_status(404)
            .create();

        assert_eq!(
            rename(&client_of(&server), &String::from(id(6)), "Blue fox").await,
            Err(DevicesError::NotFound)
        );
    }

    #[tokio::test]
    async fn nothing_is_renamed_without_a_wallet() {
        let mut server = mockito::Server::new_async().await;
        let never = server
            .mock("PATCH", mockito::Matcher::Any)
            .expect(0)
            .create();
        let logged_out = SharedWarrenApiClient::new(
            server.url(),
            std::sync::Arc::new(std::sync::RwLock::new(
                crate::warren_sdk_client::sentinel_seed(),
            )),
        );

        assert_eq!(
            rename(&logged_out, &String::from(id(7)), "Blue fox").await,
            Err(DevicesError::NoWallet)
        );
        never.assert();
    }

    #[test]
    fn an_account_with_no_older_client_says_none() {
        let listed = device_list(DeviceListResponse {
            devices: Vec::new(),
            older_client_seen_on: None,
        });

        assert!(listed.devices.is_empty());
        assert_eq!(listed.older_client_seen_on, None);
    }
}
