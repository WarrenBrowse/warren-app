use crate::types::{FromProtobufTypeError, proto};
use std::str::FromStr;

impl TryFrom<proto::DeviceState> for mullvad_types::device::DeviceState {
    type Error = FromProtobufTypeError;

    fn try_from(state: proto::DeviceState) -> Result<Self, FromProtobufTypeError> {
        let state_type = proto::device_state::State::try_from(state.state)
            .map_err(|_| FromProtobufTypeError::invalid_argument("invalid device state"))?;

        match state_type {
            proto::device_state::State::LoggedIn => {
                let account = state.device.ok_or(FromProtobufTypeError::invalid_argument(
                    "missing account data",
                ))?;

                // The proto `account_number` field carries the Warren
                // SS58 address (`wb…`); parse it into a `WarrenPubKey`.
                // A client pushing a non-Warren format receives
                // `invalid_argument`.
                let pubkey =
                    mullvad_types::warren_pubkey::WarrenPubKey::from_str(&account.account_number)
                        .map_err(|_| {
                        FromProtobufTypeError::invalid_argument(
                            "account_number must be a valid Warren SS58 address (wb…)",
                        )
                    })?;
                Ok(mullvad_types::device::DeviceState::LoggedIn(
                    mullvad_types::warren_identity::WarrenIdentity { pubkey },
                ))
            }
            proto::device_state::State::Revoked => Ok(mullvad_types::device::DeviceState::Revoked),
            proto::device_state::State::LoggedOut => {
                Ok(mullvad_types::device::DeviceState::LoggedOut)
            }
        }
    }
}

impl From<mullvad_types::device::DeviceState> for proto::DeviceState {
    fn from(state: mullvad_types::device::DeviceState) -> Self {
        proto::DeviceState {
            state: proto::device_state::State::from(&state) as i32,
            // Emit the SS58 pubkey in the proto `account_number` field.
            device: state.logged_in().map(|client| proto::AccountAndDevice {
                account_number: client.pubkey.as_str().to_owned(),
                device: None,
            }),
        }
    }
}

impl From<&mullvad_types::device::DeviceState> for proto::device_state::State {
    fn from(state: &mullvad_types::device::DeviceState) -> Self {
        use mullvad_types::device::DeviceState as MullvadState;
        match state {
            MullvadState::LoggedIn(_) => proto::device_state::State::LoggedIn,
            MullvadState::LoggedOut => proto::device_state::State::LoggedOut,
            MullvadState::Revoked => proto::device_state::State::Revoked,
        }
    }
}

impl From<mullvad_types::device::DeviceEvent> for proto::DeviceEvent {
    fn from(event: mullvad_types::device::DeviceEvent) -> Self {
        proto::DeviceEvent {
            cause: i32::from(proto::device_event::Cause::from(event.cause)),
            new_state: Some(proto::DeviceState::from(event.new_state)),
        }
    }
}

impl TryFrom<proto::DeviceEvent> for mullvad_types::device::DeviceEvent {
    type Error = FromProtobufTypeError;

    fn try_from(event: proto::DeviceEvent) -> Result<Self, Self::Error> {
        let cause = proto::device_event::Cause::try_from(event.cause)
            .map_err(|_| FromProtobufTypeError::invalid_argument("invalid event"))?;
        let cause = mullvad_types::device::DeviceEventCause::from(cause);

        let new_state = mullvad_types::device::DeviceState::try_from(event.new_state.ok_or(
            FromProtobufTypeError::invalid_argument("missing device state"),
        )?)?;

        Ok(mullvad_types::device::DeviceEvent { cause, new_state })
    }
}

impl From<mullvad_types::device::DeviceEventCause> for proto::device_event::Cause {
    fn from(cause: mullvad_types::device::DeviceEventCause) -> Self {
        use mullvad_types::device::DeviceEventCause as MullvadEvent;
        match cause {
            MullvadEvent::LoggedIn => proto::device_event::Cause::LoggedIn,
            MullvadEvent::LoggedOut => proto::device_event::Cause::LoggedOut,
            MullvadEvent::Revoked => proto::device_event::Cause::Revoked,
        }
    }
}

impl From<proto::device_event::Cause> for mullvad_types::device::DeviceEventCause {
    fn from(event: proto::device_event::Cause) -> Self {
        use mullvad_types::device::DeviceEventCause as MullvadEvent;
        match event {
            proto::device_event::Cause::LoggedIn => MullvadEvent::LoggedIn,
            proto::device_event::Cause::LoggedOut => MullvadEvent::LoggedOut,
            proto::device_event::Cause::Revoked => MullvadEvent::Revoked,
            // The `Updated` and `RotatedKey` proto causes have no domain
            // equivalent; treat them as a login-state refresh.
            proto::device_event::Cause::Updated | proto::device_event::Cause::RotatedKey => {
                MullvadEvent::LoggedIn
            }
        }
    }
}
