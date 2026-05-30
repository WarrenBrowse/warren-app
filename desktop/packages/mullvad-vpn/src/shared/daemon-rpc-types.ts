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
 * 64-char lowercase hex Ed25519 public key identifying a Warren wallet.
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
        | ErrorStateCause.needFullDiskPermissions;
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
  | { deviceRemoval: Array<IDevice> }
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
  device?: IDevice;
}

export type LoggedInDeviceState = { type: 'logged in'; warrenIdentity: IWarrenIdentity };
export type LoggedOutDeviceState = { type: 'logged out' | 'revoked' };

export type DeviceState = LoggedInDeviceState | LoggedOutDeviceState;

export type DeviceEvent =
  | { type: 'logged in' | 'updated' | 'rotated_key'; deviceState: LoggedInDeviceState }
  | { type: 'logged out' | 'revoked'; deviceState: LoggedOutDeviceState };

export interface IDevice {
  id: string;
  name: string;
  created: Date;
}

export interface IDeviceRemoval {
  pubkey: WarrenPubKey;
  deviceId: string;
}

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
  // Persistent toggles exposed via gRPC. Daemon restart is required
  // to apply a change.
  warrenMode: boolean;
  warrenLocalAccount: boolean;
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
  // Warren multi-exit auto-failover (M5.B.2): when the current exit
  // becomes unreachable, the client automatically reconnects to an
  // alternative exit (same country preferred, global fallback).
  // Default ON (differentiator vs Mullvad / IVPN, which require
  // manual reconnect on exit down).
  //
  // Note on DAITA v2 (M5.B.1): Warren reuses Mullvad upstream's
  // existing `wireguard.daita.enabled` toggle rather than introducing
  // a redundant `warrenDaita` field. The daemon-side adapter
  // (talpid-warren-tunnel) reads that boolean and wires it into
  // `ClientTunnel::with_daita(...)` for the Quinn-based Warren tunnel
  // + activates the exit-side `DaitaPool`. The wire path differs
  // (Quinn datagrams + warren-protocol v3 vs WireGuard +
  // maybenot-ffi) but the user surface stays a single switch.
  warrenFailover: WarrenFailoverSettings;
}

// Transport protocol enum mirrors the gRPC `NatPmpSettings.Proto`
// shape. Default UDP. Mapping both TCP and UDP simultaneously is a
// future extension (would spawn two daemon-side refresh loops).
export enum NatPmpProto {
  udp = 'udp',
  tcp = 'tcp',
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
  protocol: NatPmpProto;
  // Suggested external port (0 = server picks).
  suggestedExternalPort: number;
  // Internal port the user's application binds (0 = unset).
  internalPort: number;
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

// Live NAT-PMP refresh-loop status. Pushed by the daemon via the
// `natPmpStatusUpdates` IPC channel and read on demand via
// `getNatPmpSettings`.
export type NatPmpStatus =
  | { state: 'disabled' }
  | { state: 'requesting' }
  | {
      state: 'mapped';
      externalPort: number;
      lifetimeGrantedSecs: number;
      // Per-source rate-limit slots still available, as reported by the
      // exit. `undefined` when the exit sent no budget trailer. The UI
      // warns at <= 1 and blocks the port control at 0.
      attemptsRemaining?: number;
      // Seconds until the rate-limit budget grows by one. Drives the
      // "wait before next change" countdown when attemptsRemaining === 0.
      windowResetSecs: number;
    }
  // The exit rate-limited the last port change (too many in a row). The
  // daemon retries automatically after `retryAfterSecs`; the UI blocks
  // the port control and shows a deban countdown until then.
  | { state: 'rate-limited'; retryAfterSecs: number }
  | { state: 'failed'; errorMessage: string; errorReason: NatPmpErrorReason };

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

// Warren multi-exit failover settings (M5.B.2). When `enabled`, the
// daemon detects an unreachable exit via tunnel handshake timeouts
// (default 3 consecutive failures) and reconnects to an alternative
// exit using `select_failover_alternative` (same-country preference,
// global fallback). Default ON: a key differentiator vs
// Mullvad/IVPN, which require the user to manually disconnect and
// pick a new server when their exit becomes unreachable.
export interface WarrenFailoverSettings {
  enabled: boolean;
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
