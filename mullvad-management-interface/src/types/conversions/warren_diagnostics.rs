use crate::types::{FromProtobufTypeError, proto};
use mullvad_types::warren_diagnostics::{
    CarrierVerdictKind, CarrierVerdictReport, WarrenDiagnostics,
};

impl From<WarrenDiagnostics> for proto::WarrenDiagnostics {
    fn from(diagnostics: WarrenDiagnostics) -> Self {
        proto::WarrenDiagnostics {
            requested_n_connections: u32::from(diagnostics.requested_n_connections),
            carrier_verdict: diagnostics.carrier_verdict.map(proto::CarrierVerdict::from),
            dual_homed_interfaces: diagnostics.dual_homed_interfaces,
        }
    }
}

impl From<CarrierVerdictReport> for proto::CarrierVerdict {
    fn from(report: CarrierVerdictReport) -> Self {
        proto::CarrierVerdict {
            kind: i32::from(match report.kind {
                CarrierVerdictKind::BindOk => proto::CarrierVerdictKind::CarrierBindOk,
                CarrierVerdictKind::RouteOnly => proto::CarrierVerdictKind::CarrierRouteOnly,
            }),
            age_seconds: report.age_seconds,
            ttl_seconds: report.ttl_seconds,
        }
    }
}

impl TryFrom<proto::WarrenDiagnostics> for WarrenDiagnostics {
    type Error = FromProtobufTypeError;

    fn try_from(diagnostics: proto::WarrenDiagnostics) -> Result<Self, Self::Error> {
        Ok(WarrenDiagnostics {
            // The daemon resolves the count against a 1..=16 range before
            // sending, so anything wider is a corrupt peer rather than a value
            // to clamp into something plausible.
            requested_n_connections: u8::try_from(diagnostics.requested_n_connections).map_err(
                |_| FromProtobufTypeError::invalid_argument("n_connections out of range"),
            )?,
            carrier_verdict: diagnostics
                .carrier_verdict
                .map(CarrierVerdictReport::try_from)
                .transpose()?,
            dual_homed_interfaces: diagnostics.dual_homed_interfaces,
        })
    }
}

impl TryFrom<proto::CarrierVerdict> for CarrierVerdictReport {
    type Error = FromProtobufTypeError;

    fn try_from(verdict: proto::CarrierVerdict) -> Result<Self, Self::Error> {
        let kind = match proto::CarrierVerdictKind::try_from(verdict.kind) {
            Ok(proto::CarrierVerdictKind::CarrierBindOk) => CarrierVerdictKind::BindOk,
            Ok(proto::CarrierVerdictKind::CarrierRouteOnly) => CarrierVerdictKind::RouteOnly,
            // A verdict kind this client cannot name must not be rendered as
            // one it can: the whole point of the row is that it is trustworthy.
            Err(_) => {
                return Err(FromProtobufTypeError::invalid_argument(
                    "unknown carrier verdict kind",
                ));
            }
        };
        Ok(CarrierVerdictReport {
            kind,
            age_seconds: verdict.age_seconds,
            ttl_seconds: verdict.ttl_seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostics(carrier_verdict: Option<CarrierVerdictReport>) -> WarrenDiagnostics {
        WarrenDiagnostics {
            requested_n_connections: 8,
            carrier_verdict,
            dual_homed_interfaces: vec!["en0".to_owned(), "en5".to_owned()],
        }
    }

    #[test]
    fn diagnostics_survive_the_proto_roundtrip() {
        let original = diagnostics(Some(CarrierVerdictReport {
            kind: CarrierVerdictKind::RouteOnly,
            age_seconds: 3_600,
            ttl_seconds: 604_800,
        }));
        let restored =
            WarrenDiagnostics::try_from(proto::WarrenDiagnostics::from(original.clone())).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn a_platform_without_a_carrier_guard_roundtrips_as_no_verdict() {
        let original = diagnostics(None);
        let restored =
            WarrenDiagnostics::try_from(proto::WarrenDiagnostics::from(original.clone())).unwrap();
        assert_eq!(restored.carrier_verdict, None);
        assert_eq!(restored, original);
    }

    #[test]
    fn an_unknown_verdict_kind_is_rejected_rather_than_renamed() {
        let mut wire = proto::WarrenDiagnostics::from(diagnostics(Some(CarrierVerdictReport {
            kind: CarrierVerdictKind::BindOk,
            age_seconds: 1,
            ttl_seconds: 2,
        })));
        wire.carrier_verdict.as_mut().unwrap().kind = 99;
        assert!(WarrenDiagnostics::try_from(wire).is_err());
    }
}
