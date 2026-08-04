//! Minimal device REST proxy retained solely for the iOS FFI
//! (`warren-ios/src/api_client/device.rs`), which still drives the legacy
//! account/device login on iOS pending its migration to the wallet-identity
//! model used by the daemon, desktop, and Android clients.
//!
//! The daemon no longer manages Mullvad devices (the `Device` model and the
//! WireGuard key machinery were removed when login collapsed to
//! wallet-identity-only). This shim therefore exposes ONLY the raw request
//! builders the FFI needs (`*_response` + `remove`); it deliberately omits the
//! typed `Device`-returning helpers so none of the removed device machinery is
//! reintroduced. It lives in this crate (not in `warren-ios`) because building
//! a request requires `MullvadRestHandle::service`, which is `pub(crate)`.

use hyper::StatusCode;
use hyper::body::Incoming;
use mullvad_types::account::AccountNumber;
use std::future::Future;
use talpid_types::net::wireguard;

use crate::ACCOUNTS_URL_PREFIX;
use crate::rest;

#[derive(Clone)]
pub struct DevicesProxy {
    handle: rest::MullvadRestHandle,
}

impl DevicesProxy {
    pub fn new(handle: rest::MullvadRestHandle) -> Self {
        Self { handle }
    }

    pub fn get_response(
        &self,
        account: AccountNumber,
        id: String,
    ) -> impl Future<Output = Result<rest::Response<Incoming>, rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .get_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/devices/{id}"))?
                .expected_status(&[StatusCode::OK])
                .account(account)?;
            service.request(request).await
        }
    }

    pub fn list_response(
        &self,
        account: AccountNumber,
    ) -> impl Future<Output = Result<rest::Response<Incoming>, rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .get_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/devices"))?
                .expected_status(&[StatusCode::OK])
                .account(account)?;
            service.request(request).await
        }
    }

    pub fn remove(
        &self,
        account: AccountNumber,
        id: String,
    ) -> impl Future<Output = Result<(), rest::Error>> + use<> {
        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();
        async move {
            let request = factory
                .delete_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/devices/{id}"))?
                .expected_status(&[StatusCode::NO_CONTENT])
                .account(account)?;
            service.request(request).await?;
            Ok(())
        }
    }

    pub fn replace_wg_key_response(
        &self,
        account: AccountNumber,
        id: String,
        pubkey: wireguard::PublicKey,
    ) -> impl Future<Output = Result<rest::Response<Incoming>, rest::Error>> + use<> {
        #[derive(serde::Serialize)]
        struct RotateDevicePubkey {
            pubkey: wireguard::PublicKey,
        }
        let req_body = RotateDevicePubkey { pubkey };

        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .put_json_or_signed(
                    &format!("{ACCOUNTS_URL_PREFIX}/devices/{id}/pubkey"),
                    &req_body,
                )?
                .expected_status(&[StatusCode::OK])
                .account(account)?;
            service.request(request).await
        }
    }

    pub fn create_response(
        &self,
        account: AccountNumber,
        pubkey: wireguard::PublicKey,
    ) -> impl Future<Output = Result<rest::Response<Incoming>, rest::Error>> + use<> {
        #[derive(serde::Serialize)]
        struct DeviceSubmission {
            pubkey: wireguard::PublicKey,
            hijack_dns: bool,
        }

        let submission = DeviceSubmission {
            pubkey,
            hijack_dns: false,
        };

        let service = self.handle.service.clone();
        let factory = self.handle.factory.clone();

        async move {
            let request = factory
                .post_json_or_signed(&format!("{ACCOUNTS_URL_PREFIX}/devices"), &submission)?
                .account(account)?
                .expected_status(&[StatusCode::CREATED]);
            service.request(request).await
        }
    }
}
