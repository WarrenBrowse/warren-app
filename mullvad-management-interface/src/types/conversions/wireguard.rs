use super::FromProtobufTypeError;
use crate::types::proto;

// The `QuantumResistantState` and `DaitaSettings` conversions feed the
// Quinn tunnel settings.

impl From<mullvad_types::wireguard::QuantumResistantState> for proto::QuantumResistantState {
    fn from(state: mullvad_types::wireguard::QuantumResistantState) -> Self {
        match state {
            mullvad_types::wireguard::QuantumResistantState::On => proto::QuantumResistantState {
                state: i32::from(proto::quantum_resistant_state::State::On),
            },
            mullvad_types::wireguard::QuantumResistantState::Off => proto::QuantumResistantState {
                state: i32::from(proto::quantum_resistant_state::State::Off),
            },
        }
    }
}

impl TryFrom<proto::QuantumResistantState> for mullvad_types::wireguard::QuantumResistantState {
    type Error = FromProtobufTypeError;

    fn try_from(state: proto::QuantumResistantState) -> Result<Self, Self::Error> {
        match proto::quantum_resistant_state::State::try_from(state.state) {
            Ok(proto::quantum_resistant_state::State::On) => {
                Ok(mullvad_types::wireguard::QuantumResistantState::On)
            }
            Ok(proto::quantum_resistant_state::State::Off) => {
                Ok(mullvad_types::wireguard::QuantumResistantState::Off)
            }
            Err(_) => Err(FromProtobufTypeError::invalid_argument(
                "invalid quantum resistance state",
            )),
        }
    }
}

#[cfg(daita)]
impl From<mullvad_types::wireguard::DaitaSettings> for proto::DaitaSettings {
    fn from(settings: mullvad_types::wireguard::DaitaSettings) -> Self {
        proto::DaitaSettings {
            enabled: settings.enabled,
            direct_only: !settings.use_multihop_if_necessary,
        }
    }
}

#[cfg(daita)]
impl From<proto::DaitaSettings> for mullvad_types::wireguard::DaitaSettings {
    fn from(settings: proto::DaitaSettings) -> Self {
        mullvad_types::wireguard::DaitaSettings {
            enabled: settings.enabled,
            use_multihop_if_necessary: !settings.direct_only,
        }
    }
}
