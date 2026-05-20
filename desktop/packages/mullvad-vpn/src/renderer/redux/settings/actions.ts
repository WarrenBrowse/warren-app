import { ISplitTunnelingApplication } from '../../../shared/application-types';
import {
  AccessMethodSetting,
  ApiAccessMethodSettings,
  CustomLists,
  IDaitaSettings,
  IDnsOptions,
  IWireguardEndpointData,
  NatPmpSettings,
  NatPmpStatus,
  ObfuscationSettings,
  type Recents,
  RelayOverride,
  WarrenFailoverSettings,
  WarrenMultiHopSettings,
  WarrenStatus,
} from '../../../shared/daemon-rpc-types';
import { IGuiSettingsState } from '../../../shared/gui-settings-state';
import { IRelayLocationCountryRedux, RelaySettingsRedux } from './reducers';

export interface IUpdateGuiSettingsAction {
  type: 'UPDATE_GUI_SETTINGS';
  guiSettings: IGuiSettingsState;
}

export interface IUpdateRelayAction {
  type: 'UPDATE_RELAY';
  relay: RelaySettingsRedux;
}

export interface IUpdateRelayLocationsAction {
  type: 'UPDATE_RELAY_LOCATIONS';
  relayLocations: IRelayLocationCountryRedux[];
}

export interface IUpdateWireguardEndpointData {
  type: 'UPDATE_WIREGUARD_ENDPOINT_DATA';
  wireguardEndpointData: IWireguardEndpointData;
}

export interface IUpdateAllowLanAction {
  type: 'UPDATE_ALLOW_LAN';
  allowLan: boolean;
}

// Update actions for the Warren toggles.
export interface IUpdateWarrenModeAction {
  type: 'UPDATE_WARREN_MODE';
  warrenMode: boolean;
}

export interface IUpdateWarrenLocalAccountAction {
  type: 'UPDATE_WARREN_LOCAL_ACCOUNT';
  warrenLocalAccount: boolean;
}

// Update action for the warren-api URL.
export interface IUpdateWarrenApiUrlAction {
  type: 'UPDATE_WARREN_API_URL';
  warrenApiUrl?: string;
}

// Update action for the Warren multi-hop settings.
export interface IUpdateWarrenMultiHopAction {
  type: 'UPDATE_WARREN_MULTI_HOP';
  warrenMultiHop: WarrenMultiHopSettings;
}

// Update action for the Warren multi-exit failover settings (M5.B.2).
// M5.B.1 DAITA settings reuse Mullvad upstream's `wireguard.daita`
// state slice, so there is no Warren-specific `UPDATE_WARREN_DAITA`.
export interface IUpdateWarrenFailoverAction {
  type: 'UPDATE_WARREN_FAILOVER';
  warrenFailover: WarrenFailoverSettings;
}

// Update action for the live Warren status (reconnect_count + age).
export interface IUpdateWarrenStatusAction {
  type: 'UPDATE_WARREN_STATUS';
  warrenStatus: WarrenStatus;
}

// Update action for the persisted NAT-PMP settings (toggle + lifetime
// + protocol + ports). Dispatched on every Settings.warrenNatPmp
// change so the port-forwarding view stays in sync.
export interface IUpdateNatPmpSettingsAction {
  type: 'UPDATE_NAT_PMP_SETTINGS';
  natPmpSettings: NatPmpSettings;
}

// Update action for the live NAT-PMP status (refresh-loop lifecycle).
export interface IUpdateNatPmpStatusAction {
  type: 'UPDATE_NAT_PMP_STATUS';
  natPmpStatus: NatPmpStatus;
}

export interface IUpdateEnableIpv6Action {
  type: 'UPDATE_ENABLE_IPV6';
  enableIpv6: boolean;
}

export interface IUpdateLockdownModeAction {
  type: 'UPDATE_LOCKDOWN_MODE';
  lockdownMode: boolean;
}

export interface IUpdateShowBetaReleasesAction {
  type: 'UPDATE_SHOW_BETA_NOTIFICATIONS';
  showBetaReleases: boolean;
}

export interface IUpdateWireguardMtuAction {
  type: 'UPDATE_WIREGUARD_MTU';
  mtu?: number;
}

export interface IUpdateWireguardQuantumResistantAction {
  type: 'UPDATE_WIREGUARD_QUANTUM_RESISTANT';
  quantumResistant: boolean;
}

export interface IUpdateWireguardDaitaAction {
  type: 'UPDATE_WIREGUARD_DAITA';
  daita?: IDaitaSettings;
}

export interface IUpdateAutoStartAction {
  type: 'UPDATE_AUTO_START';
  autoStart: boolean;
}

export interface IUpdateDnsOptionsAction {
  type: 'UPDATE_DNS_OPTIONS';
  dns: IDnsOptions;
}

export interface IUpdateSplitTunnelingStateAction {
  type: 'UPDATE_SPLIT_TUNNELING_STATE';
  enabled: boolean;
}

export interface ISetSplitTunnelingApplicationsAction {
  type: 'SET_SPLIT_TUNNELING_APPLICATIONS';
  applications: ISplitTunnelingApplication[];
}

export interface ISetSplitTunnelingSupportedAction {
  type: 'SET_SPLIT_TUNNELING_SUPPORTED';
  supported: boolean;
}

export interface ISetObfuscationSettings {
  type: 'SET_OBFUSCATION_SETTINGS';
  obfuscationSettings: ObfuscationSettings;
}

export interface ISetCustomLists {
  type: 'SET_CUSTOM_LISTS';
  customLists: CustomLists;
}

export interface ISetApiAccessMethods {
  type: 'SET_API_ACCESS_METHODS';
  accessMethods: ApiAccessMethodSettings;
}

export interface ISetCurrentApiAccessMethod {
  type: 'SET_CURRENT_API_ACCESS_METHOD';
  accessMethod: AccessMethodSetting;
}

export interface ISetRelayOverrides {
  type: 'SET_RELAY_OVERRIDES';
  relayOverrides: Array<RelayOverride>;
}

export interface ISetRecents {
  type: 'SET_RECENTS';
  recents?: Recents;
}

export type SettingsAction =
  | IUpdateGuiSettingsAction
  | IUpdateRelayAction
  | IUpdateRelayLocationsAction
  | IUpdateWireguardEndpointData
  | IUpdateAllowLanAction
  | IUpdateWarrenModeAction
  | IUpdateWarrenLocalAccountAction
  | IUpdateWarrenApiUrlAction
  | IUpdateWarrenMultiHopAction
  | IUpdateWarrenFailoverAction
  | IUpdateWarrenStatusAction
  | IUpdateNatPmpSettingsAction
  | IUpdateNatPmpStatusAction
  | IUpdateEnableIpv6Action
  | IUpdateLockdownModeAction
  | IUpdateShowBetaReleasesAction
  | IUpdateWireguardMtuAction
  | IUpdateWireguardQuantumResistantAction
  | IUpdateWireguardDaitaAction
  | IUpdateAutoStartAction
  | IUpdateDnsOptionsAction
  | IUpdateSplitTunnelingStateAction
  | ISetSplitTunnelingApplicationsAction
  | ISetSplitTunnelingSupportedAction
  | ISetObfuscationSettings
  | ISetCustomLists
  | ISetRecents
  | ISetApiAccessMethods
  | ISetCurrentApiAccessMethod
  | ISetRelayOverrides;

function updateGuiSettings(guiSettings: IGuiSettingsState): IUpdateGuiSettingsAction {
  return {
    type: 'UPDATE_GUI_SETTINGS',
    guiSettings,
  };
}

function updateRelay(relay: RelaySettingsRedux): IUpdateRelayAction {
  return {
    type: 'UPDATE_RELAY',
    relay,
  };
}

function updateRelayLocations(
  relayLocations: IRelayLocationCountryRedux[],
): IUpdateRelayLocationsAction {
  return {
    type: 'UPDATE_RELAY_LOCATIONS',
    relayLocations,
  };
}

function updateWireguardEndpointData(
  wireguardEndpointData: IWireguardEndpointData,
): IUpdateWireguardEndpointData {
  return {
    type: 'UPDATE_WIREGUARD_ENDPOINT_DATA',
    wireguardEndpointData,
  };
}

function updateAllowLan(allowLan: boolean): IUpdateAllowLanAction {
  return {
    type: 'UPDATE_ALLOW_LAN',
    allowLan,
  };
}

function updateWarrenMode(warrenMode: boolean): IUpdateWarrenModeAction {
  return {
    type: 'UPDATE_WARREN_MODE',
    warrenMode,
  };
}

function updateWarrenLocalAccount(warrenLocalAccount: boolean): IUpdateWarrenLocalAccountAction {
  return {
    type: 'UPDATE_WARREN_LOCAL_ACCOUNT',
    warrenLocalAccount,
  };
}

function updateWarrenApiUrl(warrenApiUrl?: string): IUpdateWarrenApiUrlAction {
  return {
    type: 'UPDATE_WARREN_API_URL',
    warrenApiUrl,
  };
}

function updateWarrenMultiHop(warrenMultiHop: WarrenMultiHopSettings): IUpdateWarrenMultiHopAction {
  return {
    type: 'UPDATE_WARREN_MULTI_HOP',
    warrenMultiHop,
  };
}

function updateWarrenFailover(
  warrenFailover: WarrenFailoverSettings,
): IUpdateWarrenFailoverAction {
  return {
    type: 'UPDATE_WARREN_FAILOVER',
    warrenFailover,
  };
}

function updateWarrenStatus(warrenStatus: WarrenStatus): IUpdateWarrenStatusAction {
  return {
    type: 'UPDATE_WARREN_STATUS',
    warrenStatus,
  };
}

function updateNatPmpSettings(natPmpSettings: NatPmpSettings): IUpdateNatPmpSettingsAction {
  return {
    type: 'UPDATE_NAT_PMP_SETTINGS',
    natPmpSettings,
  };
}

function updateNatPmpStatus(natPmpStatus: NatPmpStatus): IUpdateNatPmpStatusAction {
  return {
    type: 'UPDATE_NAT_PMP_STATUS',
    natPmpStatus,
  };
}

function updateEnableIpv6(enableIpv6: boolean): IUpdateEnableIpv6Action {
  return {
    type: 'UPDATE_ENABLE_IPV6',
    enableIpv6,
  };
}

function updateLockdownMode(lockdownMode: boolean): IUpdateLockdownModeAction {
  return {
    type: 'UPDATE_LOCKDOWN_MODE',
    lockdownMode,
  };
}

function updateShowBetaReleases(showBetaReleases: boolean): IUpdateShowBetaReleasesAction {
  return {
    type: 'UPDATE_SHOW_BETA_NOTIFICATIONS',
    showBetaReleases,
  };
}

function updateWireguardMtu(mtu?: number): IUpdateWireguardMtuAction {
  return {
    type: 'UPDATE_WIREGUARD_MTU',
    mtu,
  };
}

function updateWireguardQuantumResistant(
  quantumResistant: boolean,
): IUpdateWireguardQuantumResistantAction {
  return {
    type: 'UPDATE_WIREGUARD_QUANTUM_RESISTANT',
    quantumResistant,
  };
}

function updateWireguardDaita(daita?: IDaitaSettings): IUpdateWireguardDaitaAction {
  return {
    type: 'UPDATE_WIREGUARD_DAITA',
    daita,
  };
}

function updateAutoStart(autoStart: boolean): IUpdateAutoStartAction {
  return {
    type: 'UPDATE_AUTO_START',
    autoStart,
  };
}

function updateDnsOptions(dns: IDnsOptions): IUpdateDnsOptionsAction {
  return {
    type: 'UPDATE_DNS_OPTIONS',
    dns,
  };
}

function updateSplitTunnelingState(enabled: boolean): IUpdateSplitTunnelingStateAction {
  return {
    type: 'UPDATE_SPLIT_TUNNELING_STATE',
    enabled,
  };
}

function setSplitTunnelingApplications(
  applications: ISplitTunnelingApplication[],
): ISetSplitTunnelingApplicationsAction {
  return {
    type: 'SET_SPLIT_TUNNELING_APPLICATIONS',
    applications,
  };
}

function setSplitTunnelingSupported(supported: boolean): ISetSplitTunnelingSupportedAction {
  return {
    type: 'SET_SPLIT_TUNNELING_SUPPORTED',
    supported,
  };
}

function updateObfuscationSettings(
  obfuscationSettings: ObfuscationSettings,
): ISetObfuscationSettings {
  return {
    type: 'SET_OBFUSCATION_SETTINGS',
    obfuscationSettings,
  };
}

function updateCustomLists(customLists: CustomLists): ISetCustomLists {
  return {
    type: 'SET_CUSTOM_LISTS',
    customLists,
  };
}

function updateApiAccessMethods(methods: ApiAccessMethodSettings): ISetApiAccessMethods {
  return {
    type: 'SET_API_ACCESS_METHODS',
    accessMethods: methods,
  };
}

function updateCurrentApiAccessMethod(setting: AccessMethodSetting): ISetCurrentApiAccessMethod {
  return {
    type: 'SET_CURRENT_API_ACCESS_METHOD',
    accessMethod: setting,
  };
}

function updateRelayOverrides(relayOverrides: Array<RelayOverride>): ISetRelayOverrides {
  return {
    type: 'SET_RELAY_OVERRIDES',
    relayOverrides,
  };
}

function updateRecents(recents?: Recents): ISetRecents {
  return {
    type: 'SET_RECENTS',
    recents,
  };
}

export default {
  updateGuiSettings,
  updateRelay,
  updateRelayLocations,
  updateWireguardEndpointData,
  updateAllowLan,
  updateWarrenMode,
  updateWarrenLocalAccount,
  updateWarrenApiUrl,
  updateWarrenMultiHop,
  updateWarrenFailover,
  updateWarrenStatus,
  updateNatPmpSettings,
  updateNatPmpStatus,
  updateEnableIpv6,
  updateLockdownMode,
  updateShowBetaReleases,
  updateWireguardMtu,
  updateWireguardQuantumResistant,
  updateWireguardDaita,
  updateAutoStart,
  updateDnsOptions,
  updateSplitTunnelingState,
  setSplitTunnelingApplications,
  setSplitTunnelingSupported,
  updateObfuscationSettings,
  updateCustomLists,
  updateApiAccessMethods,
  updateCurrentApiAccessMethod,
  updateRelayOverrides,
  updateRecents,
};
