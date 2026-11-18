#[expect(clippy::allow_attributes)]
mod proto {
    pub mod management_interface {
        tonic::include_proto!("mullvad_daemon.management_interface");
    }
    pub use management_interface::*;
}
pub use proto::*;

mod conversions;

pub use prost_types::{Duration, Timestamp};

pub use conversions::*;
