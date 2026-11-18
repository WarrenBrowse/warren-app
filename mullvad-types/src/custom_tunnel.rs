use serde::{Deserialize, Serialize};
use std::fmt;
use talpid_types::net::{Endpoint, wireguard::ConnectionConfig};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CustomTunnelEndpoint {
    pub host: String,
    pub config: ConnectionConfig,
}

impl CustomTunnelEndpoint {
    pub fn new(host: String, config: ConnectionConfig) -> Self {
        Self { host, config }
    }

    pub fn endpoint(&self) -> Endpoint {
        self.config.get_endpoint()
    }
}

impl fmt::Display for CustomTunnelEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WireGuard relay - {}:{} with public key {}",
            self.host,
            self.endpoint().address.port(),
            self.config.peer.public_key
        )
    }
}
