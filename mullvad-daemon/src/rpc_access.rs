//! The class of every management RPC, and the gate that admits each call.
//!
//! One table names every method of both services with its [`RpcClass`]. A
//! method missing from it is refused to everyone but administrators, and the
//! test at the bottom fails the build of a service definition that grows a
//! method without a class.

use std::sync::Arc;

use mullvad_management_interface::{
    Bytes, CallExtensions, Code, PeerCredentials, RpcGate, Status,
    types::{management_service_server, relay_selector_service_server},
};

use crate::wallet_access::{Admission, Refusal, RpcClass, WalletAccessControl};

/// Declares the methods of one gRPC service with their classes.
macro_rules! rpc_classes {
    ($(#[$meta:meta])* $name:ident { $($method:ident => $class:ident,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($method,)* }

        impl $name {
            #[cfg(test)]
            const ALL: &[$name] = &[$($name::$method,)*];

            fn from_method(method: &str) -> Option<Self> {
                match method {
                    $(stringify!($method) => Some(Self::$method),)*
                    _ => None,
                }
            }

            #[must_use]
            pub const fn class(self) -> RpcClass {
                match self {
                    $(Self::$method => RpcClass::$class,)*
                }
            }
        }
    };
}

rpc_classes! {
    /// Every method of `ManagementService`.
    ManagementRpc {
        // Tunnel state and control.
        ConnectTunnel => ControlMachine,
        DisconnectTunnel => ControlMachine,
        ReconnectTunnel => ControlMachine,
        GetTunnelState => ReadPublic,
        ClearEnvYield => ControlMachine,

        // The daemon itself.
        EventsListen => ReadPublic,
        PrepareRestart => ControlMachine,
        PrepareRestartV2 => ControlMachine,
        FactoryReset => ControlMachine,
        GetCurrentVersion => ReadPublic,
        GetVersionInfo => ReadPublic,
        IsPerformingPostUpgrade => ReadPublic,
        SetLogFilter => ControlMachine,
        // The daemon log narrates the owner's use of the VPN.
        LogListen => Identity,
        GetFeatureIndicators => ReadPublic,
        GetMigrationEvent => ReadPublic,
        ClearMigrationMessage => ControlMachine,

        // Relays and constraints.
        UpdateRelayLocations => ControlMachine,
        GetRelayLocations => ReadPublic,
        SetRelaySettings => ControlMachine,
        SetObfuscationSettings => ControlMachine,
        GetBridges => ReadPublic,
        SetRelayOverride => ControlMachine,
        ClearAllRelayOverrides => ControlMachine,
        DisableRelay => ControlMachine,
        EnableRelay => ControlMachine,

        // Settings. Secrets in the settings are withheld from non-owners.
        GetSettings => ReadPublic,
        ResetSettings => ControlMachine,
        SetAllowLan => ControlMachine,
        SetShowBetaReleases => ControlMachine,
        SetLockdownMode => ControlMachine,
        SetAutoConnect => ControlMachine,
        SetWireguardMtu => ControlMachine,
        SetWireguardAllowedIps => ControlMachine,
        SetEnableIpv6 => ControlMachine,
        SetQuantumResistantTunnel => ControlMachine,
        SetEnableDaita => ControlMachine,
        SetDaitaDirectOnly => ControlMachine,
        SetDaitaSettings => ControlMachine,
        SetDnsOptions => ControlMachine,
        SetEnableRecents => ControlMachine,
        SetUserspaceWireguard => ControlMachine,
        ApplyJsonSettings => ControlMachine,
        ExportJsonSettings => ReadPublic,

        // Warren tunnel settings and status.
        SetWarrenApiUrl => ControlMachine,
        SetWarrenNConnections => ControlMachine,
        SetWarrenMaxRateBps => ControlMachine,
        GetWarrenDiagnostics => ReadPublic,
        GetWarrenMultiHopSettings => ReadPublic,
        SetWarrenMultiHopSettings => ControlMachine,
        SetWarrenCustomExit => ControlMachine,
        GetWarrenStatus => ReadPublic,
        WarrenStatusUpdates => ReadPublic,
        TrustNewExitKey => ControlMachine,
        ResetPinnedExitKeys => ControlMachine,
        DismissPubkeyMismatch => ControlMachine,
        ReportPubkeyMismatch => ControlMachine,
        GetNatPmpSettings => ReadPublic,
        SetNatPmpSettings => ControlMachine,
        NatPmpStatusUpdates => ReadPublic,

        // The wallet and the forum signatures made with its key.
        GetWarrenMnemonic => Identity,
        SetWarrenMnemonic => InstallWallet,
        SignForumLogin => Identity,
        SignForumNotifications => Identity,
        SignForumNotificationsSeen => Identity,
        SignForumAttachLogs => Identity,
        SignForumReport => Identity,

        // Account and device.
        CreateNewAccount => InstallWallet,
        LoginAccount => InstallWallet,
        LogoutAccount => Identity,
        GetAccountData => Identity,
        GetAccountHistory => Identity,
        ClearAccountHistory => Identity,
        GetWwwAuthToken => Identity,
        SubmitVoucher => Identity,
        DeleteAccount => Identity,
        GetDevice => Identity,
        UpdateDevice => Identity,
        ListDevices => Identity,
        RemoveDevice => Identity,
        SetWireguardRotationInterval => ControlMachine,
        ResetWireguardRotationInterval => ControlMachine,
        RotateWireguardKey => ControlMachine,
        GetWireguardKey => Identity,
        InitPlayPurchase => Identity,
        VerifyPlayPurchase => Identity,

        // Custom lists.
        CreateCustomList => ControlMachine,
        DeleteCustomList => ControlMachine,
        UpdateCustomList => ControlMachine,
        ClearCustomLists => ControlMachine,

        // API access methods: a custom proxy opens a path through the
        // firewall, and its credentials are secrets.
        AddApiAccessMethod => ControlMachine,
        RemoveApiAccessMethod => ControlMachine,
        SetApiAccessMethod => ControlMachine,
        UpdateApiAccessMethod => ControlMachine,
        ClearCustomApiAccessMethods => ControlMachine,
        GetCurrentApiAccessMethod => Identity,
        TestCustomApiAccessMethod => ControlMachine,
        TestApiAccessMethodById => ControlMachine,

        // Split tunneling.
        GetSplitTunnelProcesses => ReadPublic,
        AddSplitTunnelProcess => ControlMachine,
        RemoveSplitTunnelProcess => ControlMachine,
        ClearSplitTunnelProcesses => ControlMachine,
        SplitTunnelIsSupported => ReadPublic,
        AddSplitTunnelApp => ControlMachine,
        RemoveSplitTunnelApp => ControlMachine,
        SetSplitTunnelState => ControlMachine,
        ClearSplitTunnelApps => ControlMachine,
        GetExcludedProcesses => ReadPublic,
        NeedFullDiskPermissions => ReadPublic,
        CheckVolumes => ControlMachine,

        // Updates.
        GetRolloutThreshold => ReadPublic,
        RegenerateRolloutThreshold => ControlMachine,
        SetRolloutThresholdSeed => ControlMachine,
        AppUpgrade => ControlMachine,
        AppUpgradeAbort => ControlMachine,
        AppUpgradeEventsListen => ReadPublic,
        GetAppUpgradeCacheDir => ReadPublic,
    }
}

rpc_classes! {
    /// Every method of `RelaySelectorService`.
    RelaySelectorRpc {
        PartitionRelays => ReadPublic,
    }
}

/// The class of the method at a full gRPC path, `None` for a method this
/// daemon does not know.
#[must_use]
pub fn classify(path: &str) -> Option<RpcClass> {
    let (service, method) = path.strip_prefix('/')?.split_once('/')?;
    match service {
        management_service_server::SERVICE_NAME => {
            ManagementRpc::from_method(method).map(ManagementRpc::class)
        }
        relay_selector_service_server::SERVICE_NAME => {
            RelaySelectorRpc::from_method(method).map(RelaySelectorRpc::class)
        }
        _ => None,
    }
}

/// Admits each management call against the wallet's ownership.
#[derive(Clone)]
pub struct DaemonRpcGate {
    access: Arc<WalletAccessControl>,
    status_cache: crate::warren_status::WarrenStatusCache,
}

impl DaemonRpcGate {
    pub fn new(
        access: Arc<WalletAccessControl>,
        status_cache: crate::warren_status::WarrenStatusCache,
    ) -> Self {
        Self {
            access,
            status_cache,
        }
    }
}

impl RpcGate for DaemonRpcGate {
    fn admit(
        &self,
        method: &str,
        peer: Option<&PeerCredentials>,
        extensions: &mut CallExtensions,
    ) -> Result<(), Status> {
        let Some(class) = classify(method) else {
            // A method this table does not name is one only an administrator
            // may reach, whatever it turns out to do.
            if peer.is_some_and(|peer| peer.privileged) {
                return Ok(());
            }
            log::warn!("Refused a management call to a method without a class");
            return Err(Status::permission_denied("unknown method"));
        };
        match self.access.admit(peer, class) {
            Ok(admission) => {
                if admission == Admission::Claimed {
                    // Until now the owner was unknown, so every status sent to
                    // this peer had its account-bound content withheld.
                    self.status_cache.republish();
                }
                extensions.insert(AdmittedClass(class));
                Ok(())
            }
            Err(refusal) => {
                log::info!("Refused a management call ({class:?}): {refusal}");
                Err(refusal_status(refusal))
            }
        }
    }
}

/// The class the gate admitted a call as, which the method decides on again
/// when it acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedClass(pub RpcClass);

/// The status a refused call gets: the reason in words for people, and its
/// [`Refusal::code`] as the details, for clients to choose their own words.
#[must_use]
pub fn refusal_status(refusal: Refusal) -> Status {
    Status::with_details(
        Code::PermissionDenied,
        refusal.to_string(),
        Bytes::from_static(refusal.code().as_bytes()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet_access::{Principal, test_support};

    /// Every `rpc` line of a service block in a proto file.
    fn proto_methods(proto: &str, service: &str) -> Vec<String> {
        let block = proto
            .split_once(&format!("service {service} {{"))
            .unwrap_or_else(|| panic!("no service {service}"))
            .1;
        let block = &block[..block.find("\n}").expect("end of service block")];
        block
            .lines()
            .filter_map(|line| line.trim().strip_prefix("rpc "))
            .map(|rest| rest.split('(').next().unwrap().trim().to_owned())
            .collect()
    }

    /// Both directions: every method the service definition declares has a
    /// class, and the table names nothing the definition does not declare.
    fn assert_table_matches(proto: &str, service: &str, table: &[&str], full_name: &str) {
        let declared = proto_methods(proto, service);
        assert!(!declared.is_empty());
        for method in &declared {
            assert!(
                classify(&format!("/{full_name}/{method}")).is_some(),
                "{service}.{method} has no class: add it to the table in rpc_access.rs"
            );
        }
        let mut declared = declared;
        declared.sort();
        let mut table: Vec<String> = table.iter().map(|m| (*m).to_owned()).collect();
        table.sort();
        assert_eq!(
            table, declared,
            "the {service} table and the proto disagree"
        );
    }

    #[test]
    fn every_management_method_has_a_class() {
        let table: Vec<String> = ManagementRpc::ALL
            .iter()
            .map(|m| format!("{m:?}"))
            .collect();
        let table: Vec<&str> = table.iter().map(String::as_str).collect();
        assert_table_matches(
            include_str!("../../mullvad-management-interface/proto/management_interface.proto"),
            "ManagementService",
            &table,
            management_service_server::SERVICE_NAME,
        );
    }

    #[test]
    fn every_relay_selector_method_has_a_class() {
        let table: Vec<String> = RelaySelectorRpc::ALL
            .iter()
            .map(|m| format!("{m:?}"))
            .collect();
        let table: Vec<&str> = table.iter().map(String::as_str).collect();
        assert_table_matches(
            include_str!("../../mullvad-management-interface/proto/relay_selector.proto"),
            "RelaySelectorService",
            &table,
            relay_selector_service_server::SERVICE_NAME,
        );
    }

    /// The anchors of the policy, each named once so a reclassification is a
    /// visible decision rather than a slip in a long table.
    #[test]
    fn the_sensitive_methods_keep_their_class() {
        let class = |method: &str| {
            classify(&format!(
                "/{}/{method}",
                management_service_server::SERVICE_NAME
            ))
        };
        assert_eq!(class("GetTunnelState"), Some(RpcClass::ReadPublic));
        assert_eq!(class("ConnectTunnel"), Some(RpcClass::ControlMachine));
        assert_eq!(class("SetLockdownMode"), Some(RpcClass::ControlMachine));
        assert_eq!(
            class("AddSplitTunnelProcess"),
            Some(RpcClass::ControlMachine)
        );
        assert_eq!(class("AddApiAccessMethod"), Some(RpcClass::ControlMachine));
        assert_eq!(class("GetWarrenMnemonic"), Some(RpcClass::Identity));
        assert_eq!(class("SignForumLogin"), Some(RpcClass::Identity));
        assert_eq!(class("GetDevice"), Some(RpcClass::Identity));
        assert_eq!(class("SetWarrenMnemonic"), Some(RpcClass::InstallWallet));
        assert_eq!(class("CreateNewAccount"), Some(RpcClass::InstallWallet));
    }

    fn gate(owner: u32) -> (DaemonRpcGate, test_support::Scratch) {
        let scratch = test_support::Scratch::new("gate");
        scratch.store().save(&Principal::Uid(owner)).unwrap();
        let access =
            WalletAccessControl::new(scratch.store(), || true, false, test_support::NoConsole);
        let gate = DaemonRpcGate::new(
            Arc::new(access),
            crate::warren_status::WarrenStatusCache::new(),
        );
        (gate, scratch)
    }

    fn path(method: &str) -> String {
        format!("/{}/{method}", management_service_server::SERVICE_NAME)
    }

    #[expect(
        clippy::result_large_err,
        reason = "the gate's own result type, returned as the gate returns it"
    )]
    fn admit(
        gate: &DaemonRpcGate,
        method: &str,
        peer: Option<&PeerCredentials>,
    ) -> Result<CallExtensions, Status> {
        let mut extensions = CallExtensions::new();
        gate.admit(&path(method), peer, &mut extensions)?;
        Ok(extensions)
    }

    /// The gate is the path from a method's name to the policy: a non-owner is
    /// refused a control call, with words for people and a code for clients,
    /// and still reads the tunnel state.
    #[test]
    fn the_gate_applies_the_class_of_the_method_called() {
        let (gate, _scratch) = gate(1000);
        let other = PeerCredentials::unix(1001, 0);

        let refused = admit(&gate, "ConnectTunnel", Some(&other)).unwrap_err();
        assert_eq!(refused.code(), Code::PermissionDenied);
        assert_eq!(
            refused.message(),
            "Warren is set up by another account on this computer"
        );
        assert_eq!(refused.details(), b"owned_by_another_account");
        assert!(admit(&gate, "GetTunnelState", Some(&other)).is_ok());
        assert!(
            admit(
                &gate,
                "ConnectTunnel",
                Some(&PeerCredentials::unix(1000, 0))
            )
            .is_ok()
        );
    }

    /// What the method decides on again when it acts is the class the gate
    /// admitted it as.
    #[test]
    fn an_admitted_call_carries_its_class_to_the_method() {
        let (gate, _scratch) = gate(1000);
        let owner = PeerCredentials::unix(1000, 0);

        let extensions = admit(&gate, "GetWarrenMnemonic", Some(&owner)).unwrap();

        assert_eq!(
            extensions.get::<AdmittedClass>(),
            Some(&AdmittedClass(RpcClass::Identity))
        );
    }

    #[test]
    fn a_method_without_a_class_is_for_administrators_only() {
        let (gate, _scratch) = gate(1000);

        assert!(admit(&gate, "NoSuchMethod", Some(&PeerCredentials::unix(1000, 0))).is_err());
        assert!(admit(&gate, "NoSuchMethod", None).is_err());
        assert!(admit(&gate, "NoSuchMethod", Some(&PeerCredentials::unix(0, 0))).is_ok());
    }

    #[test]
    fn an_unknown_method_or_service_has_no_class() {
        let service = management_service_server::SERVICE_NAME;
        assert_eq!(classify(&format!("/{service}/NoSuchMethod")), None);
        assert_eq!(classify("/other.Service/GetTunnelState"), None);
        assert_eq!(classify("GetTunnelState"), None);
    }
}
