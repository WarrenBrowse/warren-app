use crate::types::proto;

impl From<mullvad_types::features::FeatureIndicator> for proto::FeatureIndicator {
    fn from(feature: mullvad_types::features::FeatureIndicator) -> Self {
        use proto::FeatureIndicator::*;
        match feature {
            mullvad_types::features::FeatureIndicator::QuantumResistance => QuantumResistance,
            mullvad_types::features::FeatureIndicator::Multihop => Multihop,
            mullvad_types::features::FeatureIndicator::SplitTunneling => SplitTunneling,
            mullvad_types::features::FeatureIndicator::LockdownMode => LockdownMode,
            mullvad_types::features::FeatureIndicator::LanSharing => LanSharing,
            mullvad_types::features::FeatureIndicator::DnsContentBlockers => DnsContentBlockers,
            mullvad_types::features::FeatureIndicator::CustomDns => CustomDns,
            mullvad_types::features::FeatureIndicator::AllowExternalDns => AllowExternalDns,
            mullvad_types::features::FeatureIndicator::ServerIpOverride => ServerIpOverride,
            mullvad_types::features::FeatureIndicator::Daita => Daita,
            mullvad_types::features::FeatureIndicator::DaitaMultihop => DaitaMultihop,
            mullvad_types::features::FeatureIndicator::DaitaUnavailable => DaitaUnavailable,
            mullvad_types::features::FeatureIndicator::ReducedMtu => ReducedMtu,
            mullvad_types::features::FeatureIndicator::DegradedBond => DegradedBond,
            mullvad_types::features::FeatureIndicator::IncludeOnly => IncludeOnly,
            mullvad_types::features::FeatureIndicator::AppExits => AppExits,
        }
    }
}

impl From<proto::FeatureIndicator> for mullvad_types::features::FeatureIndicator {
    fn from(feature: proto::FeatureIndicator) -> Self {
        match feature {
            proto::FeatureIndicator::QuantumResistance => Self::QuantumResistance,
            proto::FeatureIndicator::Multihop => Self::Multihop,
            proto::FeatureIndicator::SplitTunneling => Self::SplitTunneling,
            proto::FeatureIndicator::LockdownMode => Self::LockdownMode,
            proto::FeatureIndicator::LanSharing => Self::LanSharing,
            proto::FeatureIndicator::DnsContentBlockers => Self::DnsContentBlockers,
            proto::FeatureIndicator::CustomDns => Self::CustomDns,
            proto::FeatureIndicator::AllowExternalDns => Self::AllowExternalDns,
            proto::FeatureIndicator::ServerIpOverride => Self::ServerIpOverride,
            proto::FeatureIndicator::Daita => Self::Daita,
            proto::FeatureIndicator::DaitaMultihop => Self::DaitaMultihop,
            proto::FeatureIndicator::DaitaUnavailable => Self::DaitaUnavailable,
            proto::FeatureIndicator::ReducedMtu => Self::ReducedMtu,
            proto::FeatureIndicator::DegradedBond => Self::DegradedBond,
            proto::FeatureIndicator::IncludeOnly => Self::IncludeOnly,
            proto::FeatureIndicator::AppExits => Self::AppExits,
        }
    }
}

impl From<proto::FeatureIndicators> for mullvad_types::features::FeatureIndicators {
    fn from(features: proto::FeatureIndicators) -> Self {
        features
            .active_features()
            .map(mullvad_types::features::FeatureIndicator::from)
            .collect()
    }
}

impl From<mullvad_types::features::FeatureIndicators> for proto::FeatureIndicators {
    fn from(features: mullvad_types::features::FeatureIndicators) -> Self {
        let mut proto_features = Self::default();

        features
            .into_iter()
            .map(proto::FeatureIndicator::from)
            .for_each(|feature| proto_features.push_active_features(feature));

        proto_features
    }
}
