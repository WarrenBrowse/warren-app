use mullvad_api::{
    AddressCache, AddressCacheBacking, AddressCacheError, ApiEndpoint,
    access_mode::AccessMethodResolver,
    proxy::{ApiConnectionMode, ProxyConfig},
};
use mullvad_types::access_method::{AccessMethod, BuiltInAccessMethod};
use talpid_types::net::{AllowedClients, AllowedEndpoint, Endpoint, TransportProtocol};
use tonic::async_trait;

use crate::api_client::swift_data::SwiftData;

unsafe extern "C" {
    pub fn swift_store_address_cache(data: *const u8, data_size: u64);

    pub fn swift_read_address_cache() -> SwiftData;
}

#[derive(Clone, Debug)]
pub struct IOSAddressCacheBacking {}

#[async_trait]
impl AddressCacheBacking for IOSAddressCacheBacking {
    async fn read(&self) -> Result<Vec<u8>, AddressCacheError> {
        // SAFETY: swift_read_address_cache is a synchronous Swift function
        // which always returns. its failure mode (in the absence of data)
        // is to return zero-length data
        let sd = unsafe { swift_read_address_cache() };
        Ok(sd.as_ref().to_vec())
    }

    async fn write(&self, data: &[u8]) -> Result<(), AddressCacheError> {
        // SAFETY: swift_store_address_cache always returns.
        // failures to store data to the settings manager are ignored
        unsafe { swift_store_address_cache(data.as_ptr(), data.len().try_into().unwrap()) };
        Ok(())
    }
}

#[derive(Debug)]
pub struct SwiftAccessMethodResolver {
    endpoint: ApiEndpoint,
    address_cache: AddressCache<IOSAddressCacheBacking>,
}

impl SwiftAccessMethodResolver {
    pub fn new(endpoint: ApiEndpoint, address_cache: AddressCache<IOSAddressCacheBacking>) -> Self {
        Self {
            endpoint,
            address_cache,
        }
    }
}

#[async_trait]
impl AccessMethodResolver for SwiftAccessMethodResolver {
    async fn resolve_access_method_setting(
        &mut self,
        access_method: &AccessMethod,
    ) -> Option<(AllowedEndpoint, ApiConnectionMode)> {
        let connection_mode = match access_method {
            AccessMethod::BuiltIn(BuiltInAccessMethod::Direct) => ApiConnectionMode::Direct,
            AccessMethod::Custom(config) => {
                ApiConnectionMode::Proxied(ProxyConfig::from(config.clone()))
            }
        };

        let allowed_endpoint = {
            let endpoint = connection_mode.get_endpoint().unwrap_or_else(|| {
                Endpoint::from_socket_address(
                    self.endpoint.address.unwrap(),
                    TransportProtocol::Tcp,
                )
            });
            let clients = AllowedClients::All;
            AllowedEndpoint { endpoint, clients }
        };
        let provenance = match access_method {
            AccessMethod::BuiltIn(_) => "Built-in",
            AccessMethod::Custom(_) => "Custom",
        };
        log::info!(
            "AccessMethodResolver: endpoint ({}): {:}, connection mode: {:}",
            provenance,
            allowed_endpoint,
            connection_mode
        );
        Some((allowed_endpoint, connection_mode))
    }

    async fn default_connection_mode(&self) -> AllowedEndpoint {
        let endpoint = Endpoint::from_socket_address(
            self.address_cache.get_address().await,
            TransportProtocol::Tcp,
        );

        AllowedEndpoint {
            endpoint,
            clients: AllowedClients::All,
        }
    }
}
