//! The handlers of the account's device registry: list, remove, rename.

use crate::*;

impl Daemon {
    /// The client of the logged-in wallet, and this install's device id,
    /// `None` without a wallet.
    pub(crate) async fn warren_devices_client(
        &self,
    ) -> Option<(
        warren_sdk_client::SharedWarrenApiClient,
        Option<warren_api::DeviceId>,
    )> {
        let api_url = self.warren_api_url_for_incidents()?;
        let seed = self
            .parameters_generator
            .warren_seed_for_incidents()
            .await?;
        let this =
            warren_token_provider::install_device().map(|device| device.identity().device_id);
        Some((
            warren_sdk_client::SharedWarrenApiClient::new(api_url, seed),
            this,
        ))
    }

    pub(crate) async fn on_list_warren_devices(
        &mut self,
        tx: ResponseTx<warren_api::DeviceListResponse, warren_devices::DevicesError>,
    ) {
        let Some((client, this)) = self.warren_devices_client().await else {
            Self::oneshot_send(
                tx,
                Err(warren_devices::DevicesError::NoWallet),
                "list_devices response",
            );
            return;
        };
        let revoked_tx = self.tx.clone();
        tokio::spawn(async move {
            let result = warren_devices::list(&client, this).await;
            if let Err(warren_devices::DevicesError::Revoked { verified }) = result {
                let _ = revoked_tx.send(InternalDaemonEvent::WarrenDeviceRevoked { verified });
            }
            Self::oneshot_send(tx, result, "list_devices response");
        });
    }

    pub(crate) async fn on_remove_warren_device(
        &mut self,
        tx: ResponseTx<(), warren_devices::DevicesError>,
        id: String,
    ) {
        let Some((client, this)) = self.warren_devices_client().await else {
            Self::oneshot_send(
                tx,
                Err(warren_devices::DevicesError::NoWallet),
                "remove_device response",
            );
            return;
        };
        let daemon_tx = self.tx.clone();
        tokio::spawn(async move {
            let result = warren_devices::revoke(&client, &id, this).await;
            match result {
                // Revoking this device is its logout:
                // the identity it registered under is dead, and the wallet goes
                // with it, as a user's own sign-out does.
                Ok(true) => {
                    if let Some(device) = warren_token_provider::install_device()
                        && let Err(error) = device.forget()
                    {
                        log::error!("Failed to forget the logged out device identity: {error}");
                    }
                    let (logout_tx, _) = oneshot::channel();
                    let _ = daemon_tx.send(InternalDaemonEvent::Command(
                        DaemonCommand::LogoutAccount(logout_tx, true),
                    ));
                }
                Err(warren_devices::DevicesError::Revoked { verified }) => {
                    let _ = daemon_tx.send(InternalDaemonEvent::WarrenDeviceRevoked { verified });
                }
                _ => {}
            }
            Self::oneshot_send(tx, result.map(|_| ()), "remove_device response");
        });
    }

    pub(crate) async fn on_rename_warren_device(
        &mut self,
        tx: ResponseTx<(), warren_devices::DevicesError>,
        id: String,
        name: String,
    ) {
        let Some((client, this)) = self.warren_devices_client().await else {
            Self::oneshot_send(
                tx,
                Err(warren_devices::DevicesError::NoWallet),
                "rename_device response",
            );
            return;
        };
        let daemon_tx = self.tx.clone();
        tokio::spawn(async move {
            let result = warren_devices::rename(&client, &id, &name, this).await;
            if let Err(warren_devices::DevicesError::Revoked { verified }) = result {
                let _ = daemon_tx.send(InternalDaemonEvent::WarrenDeviceRevoked { verified });
            }
            Self::oneshot_send(tx, result, "rename_device response");
        });
    }
}
