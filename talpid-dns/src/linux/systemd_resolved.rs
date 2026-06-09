use std::net::IpAddr;
use talpid_dbus::systemd_resolved::{AsyncHandle, SystemdResolved as DbusInterface};
use talpid_routing::RouteManagerHandle;
use talpid_types::ErrorExt;

pub(crate) use talpid_dbus::systemd_resolved::Error as SystemdDbusError;

use crate::imp::interface::{IfaceIndexLookupError, iface_index};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("systemd-resolved operation failed")]
    SystemdResolvedError(#[from] SystemdDbusError),

    #[error("Failed to resolve interface index with error {0}")]
    InterfaceNameError(#[from] IfaceIndexLookupError),
}

pub struct SystemdResolved {
    pub dbus_interface: AsyncHandle,
    tunnel_index: u32,
}

impl SystemdResolved {
    pub fn new() -> Result<Self> {
        let dbus_interface = DbusInterface::new()?.async_handle();

        // No startup stale-resolver repair needed (unlike macOS / resolvconf):
        // systemd-resolved keys DNS per network link, and an unclean exit
        // destroys the tunnel link, so its per-link config is dropped with it.
        // `tunnel_index` of 0 is a "never set" sentinel (no real interface has
        // index 0); `reset` short-circuits on it so a reset before any `set_dns`
        // cannot act on the global link 0.
        let systemd_resolved = SystemdResolved {
            dbus_interface,
            tunnel_index: 0,
        };

        Ok(systemd_resolved)
    }

    pub async fn set_dns(
        &mut self,
        _route_manager: RouteManagerHandle,
        interface_name: &str,
        servers: &[IpAddr],
    ) -> Result<()> {
        let tunnel_index = iface_index(interface_name)?;
        self.tunnel_index = tunnel_index;

        if let Err(error) = self.dbus_interface.disable_dot(self.tunnel_index).await {
            log::error!("Failed to disable DoT: {}", error.display_chain());
        }

        if let Err(error) = self
            .dbus_interface
            .set_domains(tunnel_index, &[(".", true)])
            .await
        {
            log::error!("Failed to set search domains: {}", error.display_chain());
        }

        let _ = self
            .dbus_interface
            .set_dns(self.tunnel_index, servers.to_vec())
            .await?;

        Ok(())
    }

    pub async fn reset(&mut self) -> Result<()> {
        if self.tunnel_index == 0 {
            // Never configured (or already reset): acting on index 0 would
            // target the global link, not our tunnel.
            return Ok(());
        }

        if let Err(error) = self
            .dbus_interface
            .set_domains(self.tunnel_index, &[])
            .await
        {
            log::error!("Failed to set search domains: {}", error.display_chain());
        }

        let result = self.dbus_interface.set_dns(self.tunnel_index, vec![]).await;
        self.tunnel_index = 0;
        let _ = result?;

        Ok(())
    }
}
