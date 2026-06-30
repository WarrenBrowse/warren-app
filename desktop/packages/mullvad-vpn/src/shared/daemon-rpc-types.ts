import { IChangelog } from './ipc-types';

export type DisconnectSource =
  | 'gui-disconnect-button'
  | 'gui-expired-account'
  | 'gui-login-unblock'
  | 'gui-device-revoked'
  | 'gui-quit-button'
  | 'tray-disconnect'
  | 'tray-disconnect-quit';

export type LogoutSource = 'gui-logout-button' | 'gui-device-revoked';

export interface IAccountData {
  expiry: string;
}

// `'no-subscription'` is Warren-specific: warren-api returns 404 on
// `get_account_data` when the current Warren identity has no active
// subscription yet (steady state for a freshly bootstrapped account
// until the user purchases a plan). The daemon maps that 404 to gRPC
// `Code::NotFound`, daemon-rpc.ts maps `NotFound` to this variant,
// and the account-data cache treats it as an expired account so
// `expiredState === 'expired'` in Redux and StateTriggeredNavigation
// routes the user to the "buy plan" screen rather than the main view
// (where a Connect click would otherwise trigger a doomed handshake).
export type AccountDataError = {
  type: 'error';
  error:
    | 'invalid-account'
    | 'too-many-devices'
    | 'list-devices'
    | 'communication'
    | 'no-subscription';
};

export type AccountDataResponse = ({ type: 'success' } & IAccountData) | AccountDataError;

export type AccountNumber = string;

/**
 * Warren wallet identity, encoded as an SS58 address (Substrate
 * address format, network prefix 13295). Such an address is 47-49
 * chars and starts with `wb`, e.g.
 * `wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB`. The underlying
 * key is an Ed25519 public key; the daemon encodes it to SS58 before
 * sending it to the renderer.
 *
 * Replaces the legacy Mullvad `AccountNumber` (10-16 digit) concept.
 * Validation: {@link isWarrenPubKey}. Display: {@link formatWarrenPubKey}.
 */
export type WarrenPubKey = string;
export type Ip = string;
export interface ILocation {
  ipv4?: string;
  ipv6?: string;
  country: string;
  city?: string;
  latitude: number;
  longitude: number;
  mullvadExitIp: boolean;
  hostname?: string;
  entryHostname?: string;
  provider?: string;
}

export enum FirewallPolicyErrorType {
  generic,
  locked,
}

export type FirewallPolicyError =
  | { type: FirewallPolicyErrorType.generic }
  | {
      type: FirewallPolicyErrorType.locked;
      name: string;
      pid: number;
    };

export enum ErrorStateCause {
  authFailed,
  ipv6Unavailable,
  setFirewallPolicyError,
  setDnsError,
  startTunnelError,
  createTunnelDeviceError,
  tunnelParameterError,
  isOffline,
  splitTunnelError,
  needFullDiskPermissions,
  warrenTunnelFlapping,
}

export enum AuthFailedError {
  unknown,
  invalidAccount,
  expiredAccount,
  tooManyConnections,
}

export enum TunnelParameterError {
  noMatchingRelay,
  noMatchingBridgeRelay,
  customTunnelHostResolutionError,
  ipv4Unavailable,
  ipv6Unavailable,
  warrenPubkeyMismatch,
}

export type ErrorStateDetails =
  | {
      cause:
        | ErrorStateCause.ipv6Unavailable
        | ErrorStateCause.setDnsError
        | ErrorStateCause.startTunnelError
        | ErrorStateCause.isOffline
        | ErrorStateCause.splitTunnelError
        | ErrorStateCause.needFullDiskPermissions
        | ErrorStateCause.warrenTunnelFlapping;
      blockingError?: FirewallPolicyError;
    }
  | {
      cause: ErrorStateCause.authFailed;
      blockingError?: FirewallPolicyError;
      authFailedError: AuthFailedError;
    }
  | {
      cause: ErrorStateCause.createTunnelDeviceError;
      blockingError?: FirewallPolicyError;
      osError?: number;
    }
  | {
      cause: ErrorStateCause.tunnelParameterError;
      blockingError?: FirewallPolicyError;
      parameterError: TunnelParameterError;
    }
  | {
      cause: ErrorStateCause.setFirewallPolicyError;
      blockingError?: FirewallPolicyError;
      policyError: FirewallPolicyError;
    };

export type AfterDisconnect = 'nothing' | 'block' | 'reconnect';

export type RelayProtocol = 'tcp' | 'udp';
export type EndpointObfuscationType = 'udp2tcp' | 'shadowsocks' | 'quic' | 'lwo';

export type Constraint<T> = 'any' | { only: T };
export type LiftedConstraint<T> = 'any' | T;

export function liftConstraint<T>(constraint: Constraint<T>): LiftedConstraint<T> {
  return constraint === 'any' ? constraint : constraint.only;
}
export function wrapConstraint<T>(
  constraint: LiftedConstraint<T> | undefined | null,
): Constraint<T> {
  if (constraint) {
    return constraint === 'any' ? 'any' : { only: constraint };
  }
  return 'any';
}

export type ProxyType = 'shadowsocks' | 'custom';

export enum Ownership {
  any,
  mullvadOwned,
  rented,
}

export type TunnelType = 'wireguard' | 'warren';

export interface ITunnelEndpoint {
  address: string;
  protocol: RelayProtocol;
  quantumResistant: boolean;
  obfuscationEndpoint?: IObfuscationEndpoint;
  entryEndpoint?: IEndpoint;
  daita: boolean;
  tunnelType: TunnelType;
}

export interface IEndpoint {
  address: string;
  transportProtocol: RelayProtocol;
}

export interface IObfuscationEndpoint {
  address: string;
  protocol: RelayProtocol;
  obfuscationType: EndpointObfuscationType;
}

export interface IProxyEndpoint {
  address: string;
  protocol: RelayProtocol;
  proxyType: ProxyType;
}

export type DaemonEvent =
  | { tunnelState: TunnelState }
  | { settings: ISettings }
  | { relayList: IRelayListWithEndpointData }
  | { appVersionInfo: IAppVersionInfo }
  | { device: DeviceEvent }
  | { accessMethodSetting: AccessMethodSetting };

export type DaemonAppUpgradeEventStatusDownloadStarted = {
  type: 'APP_UPGRADE_STATUS_DOWNLOAD_STARTED';
};

export type DaemonAppUpgradeEventStatusDownloadProgress = {
  type: 'APP_UPGRADE_STATUS_DOWNLOAD_PROGRESS';
  progress: number;
  server: string;
  timeLeft?: number;
};

export type DaemonAppUpgradeEventStatusAborted = {
  type: 'APP_UPGRADE_STATUS_ABORTED';
};

export type DaemonAppUpgradeEventStatusVerifyingInstaller = {
  type: 'APP_UPGRADE_STATUS_VERIFYING_INSTALLER';
};

export type DaemonAppUpgradeEventStatusVerifiedInstaller = {
  type: 'APP_UPGRADE_STATUS_VERIFIED_INSTALLER';
};

export type DaemonAppUpgradeError = 'DOWNLOAD_FAILED' | 'GENERAL_ERROR' | 'VERIFICATION_FAILED';

export type DaemonAppUpgradeEventError = {
  type: 'APP_UPGRADE_ERROR';
  error: DaemonAppUpgradeError;
};

export type DaemonAppUpgradeEventStatus =
  | DaemonAppUpgradeEventStatusDownloadStarted
  | DaemonAppUpgradeEventStatusDownloadProgress
  | DaemonAppUpgradeEventStatusAborted
  | DaemonAppUpgradeEventStatusVerifyingInstaller
  | DaemonAppUpgradeEventStatusVerifiedInstaller;

export type DaemonAppUpgradeEvent = DaemonAppUpgradeEventStatus | DaemonAppUpgradeEventError;

export interface ITunnelStateRelayInfo {
  endpoint: ITunnelEndpoint;
  location?: ILocation;
}

// The order of the variants match the priority order and can be sorted on.
export enum FeatureIndicator {
  daita,
  daitaMultihop,
  quantumResistance,
  multihop,
  splitTunneling,
  lockdownMode,
  udp2tcp,
  shadowsocks,
  quic,
  lwo,
  lanSharing,
  dnsContentBlockers,
  customDns,
  allowExternalDns,
  serverIpOverride,
  customMtu,
}

export type DisconnectedState = {
  state: 'disconnected';
  location?: Partial<ILocation>;
  lockedDown: boolean;
};
export type ConnectingState = {
  state: 'connecting';
  details?: ITunnelStateRelayInfo;
  featureIndicators?: Array<FeatureIndicator>;
};
export type ConnectedState = {
  state: 'connected';
  details: ITunnelStateRelayInfo;
  featureIndicators?: Array<FeatureIndicator>;
};
export type DisconnectingState = {
  state: 'disconnecting';
  details: AfterDisconnect;
  location?: Partial<ILocation>;
};
export type ErrorState = { state: 'error'; details: ErrorStateDetails };

export type TunnelState =
  | DisconnectedState
  | ConnectingState
  | ConnectedState
  | DisconnectingState
  | ErrorState;

export interface RelayLocationCountry extends Partial<RelayLocationCustomList> {
  country: string;
}

export interface RelayLocationCity extends RelayLocationCountry {
  city: string;
}

export interface RelayLocationRelay extends RelayLocationCity {
  hostname: string;
}

export interface RelayLocationCustomList {
  customList: string;
}

export type RelayLocationGeographical =
  | RelayLocationRelay
  | RelayLocationCountry
  | RelayLocationCity;

export type RelayLocation = RelayLocationGeographical | RelayLocationCustomList;

export interface IWireguardConstraints {
  ipVersion: Constraint<IpVersion>;
  useMultihop: boolean;
  entryLocation: Constraint<RelayLocation>;
}

export type IpVersion = 'ipv4' | 'ipv6';

export interface IRelaySettingsNormal {
  location: Constraint<RelayLocation>;
  providers: string[];
  ownership: Ownership;
  wireguardConstraints: IWireguardConstraints;
}

export type ConnectionConfig = {
  wireguard: {
    tunnel: {
      privateKey: string;
      addresses: string[];
    };
    peer: {
      publicKey: string;
      addresses: string[];
      endpoint: string;
    };
    ipv4Gateway: string;
    ipv6Gateway?: string;
  };
};

// types describing the structure of RelaySettings
export interface IRelaySettingsCustom {
  host: string;
  config: ConnectionConfig;
}
export type RelaySettings =
  | {
      normal: IRelaySettingsNormal;
    }
  | {
      customTunnelEndpoint: IRelaySettingsCustom;
    };

export interface IRelayListWithEndpointData {
  relayList: IRelayList;
  wireguardEndpointData: IWireguardEndpointData;
}

export interface IRelayList {
  countries: IRelayListCountry[];
}

export interface IWireguardEndpointData {
  portRanges: [number, number][];
  udp2tcpPorts: number[];
}

export interface IRelayListCountry {
  name: string;
  code: string;
  cities: IRelayListCity[];
}

export interface IRelayListCity {
  name: string;
  code: string;
  latitude: number;
  longitude: number;
  relays: IRelayListHostname[];
}

export interface IRelayListHostname {
  hostname: string;
  provider: string;
  ipv4AddrIn: string;
  includeInCountry: boolean;
  active: boolean;
  weight: number;
  owned: boolean;
  daita: boolean;
  // The absence of this value signals that the relay does not deploy QUIC.
  quic?: Quic;
  lwo: boolean;
}

export type Quic = {
  domain: string;
  token: string;
  addrIn: string[];
};

export interface ITunnelOptions {
  mtu?: number;
  quantumResistant: boolean;
  daita?: IDaitaSettings;
  enableIpv6: boolean;
  dns: IDnsOptions;
}

export interface IDnsOptions {
  state: 'custom' | 'default';
  // Advanced opt-in that lifts the firewall's DNS leak protection: when true, queries to
  // resolvers other than the configured ones (e.g. `dig @1.1.1.1`) are no longer blocked. The
  // queries still travel through the tunnel. Intended for advanced users testing remote DNS.
  allowExternalDns: boolean;
  customOptions: {
    addresses: string[];
  };
  defaultOptions: {
    blockAds: boolean;
    blockTrackers: boolean;
    blockMalware: boolean;
    blockAdultContent: boolean;
    blockGambling: boolean;
    blockSocialMedia: boolean;
  };
}

export type AppVersionInfoSuggestedUpgrade = {
  changelog: IChangelog;
  verifiedInstallerPath?: string;
  version: string;
};

export interface IAppVersionInfo {
  supported: boolean;
  suggestedUpgrade?: AppVersionInfoSuggestedUpgrade;
  suggestedIsBeta?: boolean;
}

export interface IWarrenIdentity {
  pubkey: WarrenPubKey;
}

export type LoggedInDeviceState = { type: 'logged in'; warrenIdentity: IWarrenIdentity };
export type LoggedOutDeviceState = { type: 'logged out' | 'revoked' };

export type DeviceState = LoggedInDeviceState | LoggedOutDeviceState;

export type DeviceEvent =
  | { type: 'logged in' | 'updated' | 'rotated_key'; deviceState: LoggedInDeviceState }
  | { type: 'logged out' | 'revoked'; deviceState: LoggedOutDeviceState };

export type CustomLists = Array<ICustomList>;

export type Recents = (SinglehopRecentLocation | MultihopRecentLocation)[];

export type SinglehopRecentLocation = {
  type: 'singlehop';
  location: RelayLocation;
};

export type MultihopRecentLocation = {
  type: 'multihop';
  entry: RelayLocation;
  exit: RelayLocation;
};

export interface ICustomList {
  id: string;
  name: string;
  locations: Array<RelayLocationGeographical>;
}

export type NewCustomList = Pick<ICustomList, 'name' | 'locations'>;

export type CustomListError = { type: 'name already exists' };

export type AccessMethodExistsError = { type: 'name already exists' };

export interface ISettings {
  allowLan: boolean;
  autoConnect: boolean;
  lockdownMode: boolean;
  showBetaReleases: boolean;
  relaySettings: RelaySettings;
  tunnelOptions: ITunnelOptions;
  splitTunnel: SplitTunnelSettings;
  obfuscationSettings: ObfuscationSettings;
  customLists: CustomLists;
  recents?: Recents;
  apiAccessMethods: ApiAccessMethodSettings;
  relayOverrides: Array<RelayOverride>;
  // Persistent warren-api URL. `undefined` if unset (= fallback to
  // upstream Mullvad). Daemon restart required.
  warrenApiUrl?: string;
  // Warren two-relayed QUIC multi-hop settings (M4.E.D stack).
  // Default = OFF per doctrine `warren_multihop_doctrine_v1`.
  // Daemon restart required to apply.
  warrenMultiHop: WarrenMultiHopSettings;
  // Warren NAT-PMP port-forwarding settings. Default OFF. Pushed
  // live via `setNatPmpSettings` (no daemon restart required: the
  // next tunnel reconnect picks up the new config).
  warrenNatPmp: NatPmpSettings;
  // Note on DAITA v2 (M5.B.1): Warren reuses Mullvad upstream's
  // existing `wireguard.daita.enabled` toggle rather than introducing
  // a redundant `warrenDaita` field. The daemon-side adapter
  // (talpid-warren-tunnel) reads that boolean and wires it into
  // `ClientTunnel::with_daita(...)` for the Quinn-based Warren tunnel
  // + activates the exit-side `DaitaPool`. The wire path differs
  // (Quinn datagrams + warren-protocol v3 vs WireGuard +
  // maybenot-ffi) but the user surface stays a single switch.
  //
  // Note on multi-exit failover (M5.B.2): the daemon performs failover
  // unconditionally (no settings field). The renderer surfaces it via
  // the live `WarrenStatus.failoverCount` banner; there is no toggle.
}

// Transport protocol enum mirrors the gRPC `NatPmpSettings.Proto`
// shape. Default UDP. Mapping both TCP and UDP simultaneously is a
// future extension (would spawn two daemon-side refresh loops).
export enum NatPmpProto {
  udp = 'udp',
  tcp = 'tcp',
}

// One NAT-PMP port-forward rule. Multi-port: a client may hold several
// at once, up to the exit-enforced per-client quota
// (`warren_config::NATPMP_QUOTA_PER_CLIENT_IP`, currently 5). The rule
// identity used by the exit allocator is `(internalPort, protocol)`, so
// every rule must carry a distinct internal port. The UI's "same port
// on your device" model sets `internalPort === suggestedExternalPort`
// (the single port number the user picks opens publicly and is what
// their app binds locally).
export interface NatPmpRule {
  protocol: NatPmpProto;
  // Suggested external (public) port (0 = server picks from its pool).
  suggestedExternalPort: number;
  // Internal port the user's application binds.
  internalPort: number;
}

// Warren NAT-PMP port-forwarding settings. Persisted in
// `Settings.warrenNatPmp` and surfaced via the port-forwarding
// settings view (Warren differentiator since Mullvad / IVPN dropped
// port-forwarding in 2023).
export interface NatPmpSettings {
  enabled: boolean;
  // Requested lifetime in seconds. Exit clamps to [60, 3600] s, so
  // values outside that range are silently capped server-side. UI
  // exposes 1h / 6h / 24h presets that all collapse to 3600 s.
  lifetimeSecs: number;
  // Multi-port source of truth: one entry per port-forward rule. New
  // writes populate this and leave the legacy fields below at 0.
  rules: NatPmpRule[];
  // --- Legacy single-port fields (deprecated) ---
  // Kept for backward compatibility with a pre-multi-port daemon /
  // settings.json; superseded by `rules`. See `effectiveNatPmpRules`.
  protocol: NatPmpProto;
  suggestedExternalPort: number;
  internalPort: number;
}

// The effective list of rules: prefer `rules`, otherwise synthesize one
// from the legacy single-port fields (upgrade path). Returns [] when
// nothing is configured.
export function effectiveNatPmpRules(settings: NatPmpSettings): NatPmpRule[] {
  if (settings.rules.length > 0) {
    return settings.rules;
  }
  if (settings.internalPort !== 0 || settings.suggestedExternalPort !== 0) {
    return [
      {
        protocol: settings.protocol,
        suggestedExternalPort: settings.suggestedExternalPort,
        internalPort: settings.internalPort,
      },
    ];
  }
  return [];
}

// Stable, translatable category for a NAT-PMP mapping failure. Mirrors
// the daemon's `NatPmpStatus.ErrorReason` proto enum so the UI can show
// a localised message instead of the raw `errorMessage` string.
export type NatPmpErrorReason =
  | 'unknown'
  // The exit refused the explicitly requested external port because it
  // is already in use / reserved for another client (strict policy).
  | 'suggested-port-in-use'
  // Pool exhausted, per-client quota, or rate limit.
  | 'out-of-resources'
  // Port forwarding disabled exit-side, or source not allowed.
  | 'not-authorized';

// Lifecycle state of a single NAT-PMP mapping (one per rule).
export type NatPmpMappingState =
  | { state: 'requesting' }
  | {
      state: 'mapped';
      externalPort: number;
      lifetimeGrantedSecs: number;
      // Per-source rate-limit slots still available, as reported by the
      // exit (a SHARED per-client budget - the same value on every
      // mapping). `undefined` when the exit sent no budget trailer. The
      // UI warns at <= 1 and blocks the port controls at 0.
      attemptsRemaining?: number;
      // Seconds until the rate-limit budget grows by one. Drives the
      // "wait before next change" countdown when attemptsRemaining === 0.
      windowResetSecs: number;
    }
  // The exit rate-limited the last port change (too many in a row). The
  // daemon retries automatically after `retryAfterSecs`; the UI blocks
  // the port controls and shows a deban countdown until then.
  | { state: 'rate-limited'; retryAfterSecs: number }
  | { state: 'failed'; errorMessage: string; errorReason: NatPmpErrorReason }
  // NAT-PMP is off for this mapping (daemon reported DISABLED). Distinct
  // from 'requesting' so the UI does not spin a "requesting…" label
  // forever on a mapping that will never come up.
  | { state: 'disabled' };

// One live mapping, tagged with the rule it belongs to (the UI matches
// it to a rule by `internalPort` + `protocol`).
export interface NatPmpMapping {
  internalPort: number;
  protocol: NatPmpProto;
  status: NatPmpMappingState;
}

// Live NAT-PMP status. Pushed by the daemon via the `natPmpStatusUpdates`
// IPC channel and read on demand via `getNatPmpSettings`. Multi-port: one
// entry per active rule (empty == nothing mapped).
export interface NatPmpStatus {
  mappings: NatPmpMapping[];
}

// Warren multi-hop settings persisted in Settings.warren_multi_hop and
// surfaced via the Warren multi-hop view. `entryCountry` and
// `exitCountry` are ISO 3166 alpha-2 codes; empty string = auto-pick.
export interface WarrenMultiHopSettings {
  enabled: boolean;
  entryCountry: string;
  exitCountry: string;
  // HPKE epoch rotation in milliseconds. Default 4h (14_400_000 ms)
  // per `warren_multihop_doctrine_v1`.
  hpkeEpochRotationMs: number;
}

// Session A.4 TOFU pubkey-pinning mismatch event surfaced to the UI.
// When the daemon-side verify hook detects that the Ed25519 pubkey
// served for a known `exit_id` differs from the locally pinned value,
// it sets this field so the renderer can mount the
// `WarrenPubKeyWarning` modal. `null` (the steady state) means no
// mismatch is pending review.
export interface WarrenPubkeyMismatch {
  // 32-character lower-case hex (16 raw bytes) of the stable
  // `exit_id` for which the pubkey changed.
  exitIdHex: string;
  // 64-character lower-case hex (32 raw bytes) of the previously
  // pinned Ed25519 verifying key.
  pinnedPubkeyHex: string;
  // 64-character lower-case hex (32 raw bytes) of the currently
  // observed key from the signed relay-list.
  observedPubkeyHex: string;
  // Forensic snapshot of the pin's location at first-seen time.
  // Empty string when the pin pre-dates the H.6 enrichment.
  countryCode: string;
  city: string;
}

// Live Warren tunnel status snapshot. Pushed by the daemon via the
// `warrenStatusUpdates` IPC channel and read on demand via
// `getWarrenStatus`.
export interface WarrenStatus {
  reconnectCount: number;
  // Time since the last successful reconnect in milliseconds. `null`
  // if no reconnect has been observed yet (fresh session, single-hop).
  lastReconnectAgeMs: number | null;
  obfuscationActive: boolean;
  // M5.B.2 multi-exit failover: number of times the daemon picked an
  // alternative exit after the previous one became unreachable. The
  // renderer surfaces an increment as a toast "Switched to <country>".
  failoverCount: number;
  // Time since the last failover in milliseconds. `null` if no
  // failover has been observed yet.
  lastFailoverAgeMs: number | null;
  // Session A.4 TOFU pubkey-pinning: `null` (steady state) when no
  // mismatch is pending review, populated when the daemon-side
  // verify hook refused a connect because the served Ed25519
  // pubkey differs from the locally pinned value. The renderer
  // mounts `WarrenPubKeyWarning` while this field is non-null and
  // dismisses it after the user picks Trust / Reject / Report.
  pubkeyMismatchPending: WarrenPubkeyMismatch | null;
}

// Outcome of the gRPC `TrustNewExitKey` RPC. The daemon either
// updates the pinned key in the in-memory table (`ok`) or surfaces
// the precise reason the operation failed so the UI can show a
// matching error message.
export type TrustNewExitKeyOutcome =
  | { result: 'ok' }
  | { result: 'exit-not-found' }
  | { result: 'pubkey-mismatch' }
  | { result: 'io-error'; errorMessage: string };

export type SplitTunnelSettings = {
  enableExclusions: boolean;
  appsList: string[];
};

export type LwoSettings = {
  port: Constraint<number>;
};

export type Udp2TcpObfuscationSettings = {
  port: Constraint<number>;
};

export type ShadowsocksSettings = {
  port: Constraint<number>;
};

export enum ObfuscationType {
  auto,
  off,
  udp2tcp,
  shadowsocks,
  quic,
  lwo,
}

export type ObfuscationSettings = {
  selectedObfuscation: ObfuscationType;
  udp2tcpSettings: Udp2TcpObfuscationSettings;
  shadowsocksSettings: ShadowsocksSettings;
  lwoSettings: LwoSettings;
};

export interface ISocketAddress {
  host: string;
  port: number;
}

export type VoucherResponse =
  | { type: 'success'; newExpiry: string; secondsAdded: number }
  | { type: 'invalid' | 'already_used' | 'error' };

// Outcome of the consumer contract withdrawal (EU CRD art. 11a).
// `withdrawn` is `false` (benign) when no subscription was on file.
export type WithdrawResponse =
  | { type: 'success'; withdrawn: boolean; expiresAt?: number }
  | { type: 'error' };

export interface SocksAuth {
  username: string;
  password: string;
}

export type Socks5LocalCustomProxy = {
  type: 'socks5-local';
  remoteIp: string;
  remotePort: number;
  remoteTransportProtocol: RelayProtocol;
  localPort: number;
};

export type Socks5RemoteCustomProxy = {
  type: 'socks5-remote';
  ip: string;
  port: number;
  authentication?: SocksAuth;
};

export type ShadowsocksCustomProxy = {
  type: 'shadowsocks';
  ip: string;
  port: number;
  password: string;
  cipher: string;
};

export type CustomProxy = Socks5LocalCustomProxy | Socks5RemoteCustomProxy | ShadowsocksCustomProxy;
export type NamedCustomProxy = CustomProxy & { name: string };

export type DirectMethod = { type: 'direct' };
export type BridgesMethod = { type: 'bridges' };
export type EncryptedDnsProxy = { type: 'encrypted-dns-proxy' };
export type DomainFronting = { type: 'domain-fronting' };

export type AccessMethod =
  | DirectMethod
  | BridgesMethod
  | EncryptedDnsProxy
  | CustomProxy
  | DomainFronting;

export type NamedAccessMethod<T extends AccessMethod> = T & { name: string };

export type NewAccessMethodSetting<T extends AccessMethod = AccessMethod> = NamedAccessMethod<T> & {
  enabled: boolean;
};

export type AccessMethodSetting<T extends AccessMethod = AccessMethod> =
  NewAccessMethodSetting<T> & {
    id: string;
  };

export type ApiAccessMethodSettings = {
  direct: AccessMethodSetting<DirectMethod>;
  mullvadBridges: AccessMethodSetting<BridgesMethod>;
  encryptedDnsProxy: AccessMethodSetting<EncryptedDnsProxy>;
  domainFronting: AccessMethodSetting<DomainFronting>;
  custom: Array<AccessMethodSetting<CustomProxy>>;
};

export interface RelayOverride {
  hostname: string;
  ipv4AddrIn?: string;
  ipv6AddrIn?: string;
}

export interface IDaitaSettings {
  enabled: boolean;
  directOnly: boolean;
}

export function parseSocketAddress(socketAddrStr: string): ISocketAddress {
  const re = new RegExp(/(.+):(\d+)$/);
  const matches = socketAddrStr.match(re);

  if (!matches || matches.length < 3) {
    throw new Error(`Failed to parse socket address from address string '${socketAddrStr}'`);
  }
  const socketAddress: ISocketAddress = {
    host: matches[1],
    port: Number(matches[2]),
  };
  return socketAddress;
}

// Extract the host portion of a `host:port` socket-address string,
// dropping the trailing `:port`. IPv6-safe: a bracketed address such as
// `[2a01:...]:7001` keeps everything up to and including the closing `]`
// (the colons inside the brackets are part of the address, not the port
// separator). A bare IPv4 like `204.168.207.130:443` splits on the last
// colon. Used to decide whether two endpoints sit on the same node.
export function socketAddressHost(socketAddrStr: string): string {
  const trimmed = socketAddrStr.trim();
  if (trimmed.startsWith('[')) {
    const closing = trimmed.indexOf(']');
    if (closing !== -1) {
      return trimmed.slice(0, closing + 1);
    }
    return trimmed;
  }
  const lastColon = trimmed.lastIndexOf(':');
  if (lastColon === -1) {
    return trimmed;
  }
  return trimmed.slice(0, lastColon);
}

// A Warren circuit is multi-hop (2 hops) only when the entry/relay node
// and the exit node are DIFFERENT physical nodes, i.e. their host/IP
// differs. A 1-hop circuit reuses the same node as both relay and exit
// with different ports (entry `<ip>:7001`, exit `<ip>:443`), so the port
// MUST be ignored - comparing the full `ip:port` would falsely flag a
// 1-hop circuit as multi-hop. Hosts are compared case-insensitively.
export function isMultihopTunnelEndpoint(endpoint: ITunnelEndpoint): boolean {
  if (!endpoint.entryEndpoint) {
    return false;
  }
  const entryHost = socketAddressHost(endpoint.entryEndpoint.address).toLowerCase();
  const exitHost = socketAddressHost(endpoint.address).toLowerCase();
  return entryHost !== exitHost;
}

export function isMultihopTunnelState(tunnelState: TunnelState): boolean {
  if (
    (tunnelState.state !== 'connected' && tunnelState.state !== 'connecting') ||
    tunnelState.details === undefined
  ) {
    return false;
  }
  return isMultihopTunnelEndpoint(tunnelState.details.endpoint);
}

export function compareRelayLocationCount(lhs: RelayLocation, rhs: RelayLocation): boolean {
  if (
    ('count' in lhs || 'count' in rhs) &&
    !('count' in lhs && 'count' in rhs && lhs.count === rhs.count)
  ) {
    return false;
  }

  return compareRelayLocation(lhs, rhs);
}

export function compareRelayLocation(lhs: RelayLocation, rhs: RelayLocation): boolean {
  if (
    ('customList' in lhs || 'customList' in rhs) &&
    !('customList' in lhs && 'customList' in rhs && lhs.customList === rhs.customList)
  ) {
    return false;
  }

  return compareRelayLocationGeographical(lhs, rhs);
}

export function compareRelayLocationGeographical(lhs: RelayLocation, rhs: RelayLocation): boolean {
  if (
    ('country' in lhs || 'country' in rhs) &&
    !('country' in lhs && 'country' in rhs && lhs.country === rhs.country)
  ) {
    return false;
  }

  if (
    ('city' in lhs || 'city' in rhs) &&
    !('city' in lhs && 'city' in rhs && lhs.city === rhs.city)
  ) {
    return false;
  }

  if (
    ('hostname' in lhs || 'hostname' in rhs) &&
    !('hostname' in lhs && 'hostname' in rhs && lhs.hostname === rhs.hostname)
  ) {
    return false;
  }

  return true;
}

export function compareRelayLocationLoose(lhs?: RelayLocation, rhs?: RelayLocation) {
  if (lhs && rhs) {
    return compareRelayLocation(lhs, rhs);
  } else {
    return lhs === rhs;
  }
}
