import { getDefaultApiAccessMethods } from '../../../main/default-settings';
import { ISplitTunnelingApplication } from '../../../shared/application-types';
import {
  AccessMethodSetting,
  ApiAccessMethodSettings,
  CustomLists,
  IDaitaSettings,
  IDnsOptions,
  IpVersion,
  IWireguardEndpointData,
  LiftedConstraint,
  NatPmpProto,
  NatPmpSettings,
  NatPmpStatus,
  ObfuscationSettings,
  ObfuscationType,
  Ownership,
  Quic,
  type Recents,
  RelayLocation,
  RelayOverride,
  RelayProtocol,
  WarrenCustomExitSettings,
  WarrenMultiHopSettings,
  WarrenStatus,
} from '../../../shared/daemon-rpc-types';
import { IGuiSettingsState } from '../../../shared/gui-settings-state';
import { ReduxAction } from '../store';

export type NormalRelaySettingsRedux = {
  location: LiftedConstraint<RelayLocation>;
  providers: string[];
  ownership: Ownership;
  wireguard: {
    ipVersion: LiftedConstraint<IpVersion>;
    useMultihop: boolean;
    entryLocation: LiftedConstraint<RelayLocation>;
  };
};

export type RelaySettingsRedux =
  | {
      normal: NormalRelaySettingsRedux;
    }
  | {
      customTunnelEndpoint: {
        host: string;
        port: number;
        protocol: RelayProtocol;
      };
    };

export interface IRelayLocationRelayRedux {
  hostname: string;
  provider: string;
  ipv4AddrIn: string;
  includeInCountry: boolean;
  active: boolean;
  owned: boolean;
  weight: number;
  daita: boolean;
  quic?: Quic;
  lwo: boolean;
}

export interface IRelayLocationCityRedux {
  name: string;
  code: string;
  latitude: number;
  longitude: number;
  relays: IRelayLocationRelayRedux[];
}

export interface IRelayLocationCountryRedux {
  name: string;
  code: string;
  cities: IRelayLocationCityRedux[];
}

export interface ISettingsReduxState {
  autoStart: boolean;
  guiSettings: IGuiSettingsState;
  relaySettings: RelaySettingsRedux;
  relayLocations: IRelayLocationCountryRedux[];
  wireguardEndpointData: IWireguardEndpointData;
  allowLan: boolean;
  enableIpv6: boolean;
  lockdownMode: boolean;
  showBetaReleases: boolean;
  // Persistent warren-api URL.
  warrenApiUrl?: string;
  warrenMaxRateBps?: number;
  // Warren two-relayed QUIC multi-hop settings.
  warrenMultiHop: WarrenMultiHopSettings;
  // Advanced Warren custom-exit override.
  warrenCustomExit: WarrenCustomExitSettings;
  // Live Warren tunnel status (reconnect_count, last_reconnect_age,
  // obfuscation_active). Undefined until the first push from the
  // daemon WarrenStatusUpdates stream.
  warrenStatus?: WarrenStatus;
  // Persistent NAT-PMP port-forwarding settings.
  warrenNatPmp: NatPmpSettings;
  // Live NAT-PMP refresh-loop status. Undefined until the first push
  // from the daemon NatPmpStatusUpdates stream; treat undefined as
  // `{ state: 'disabled' }` UI-side.
  natPmpStatus?: NatPmpStatus;
  // Wall-clock instant (ms) the current `natPmpStatus` arrived. The
  // rate-limit countdown anchors to this (see IUpdateNatPmpStatusAction)
  // so a stale snapshot self-expires instead of re-flashing on mount.
  natPmpStatusReceivedAt?: number;
  wireguard: {
    mtu?: number;
    quantumResistant: boolean;
    daita?: IDaitaSettings;
  };
  dns: IDnsOptions;
  splitTunneling: boolean;
  splitTunnelingApplications: ISplitTunnelingApplication[];
  splitTunnelingSupported: boolean;
  obfuscationSettings: ObfuscationSettings;
  customLists: CustomLists;
  recents?: Recents;
  apiAccessMethods: ApiAccessMethodSettings;
  currentApiAccessMethod?: AccessMethodSetting;
  relayOverrides: Array<RelayOverride>;
}

const initialState: ISettingsReduxState = {
  autoStart: false,
  guiSettings: {
    preferredLocale: 'system',
    enableSystemNotifications: true,
    autoConnect: true,
    monochromaticIcon: false,
    startMinimized: false,
    unpinnedWindow: window.env.platform !== 'win32' && window.env.platform !== 'darwin',
    browsedForSplitTunnelingApplications: [],
    changelogDisplayedForVersion: '',
    updateDismissedForVersion: '',
    animateMap: true,
  },
  relaySettings: {
    normal: {
      location: 'any',
      providers: [],
      ownership: Ownership.any,
      wireguard: { ipVersion: 'any', useMultihop: false, entryLocation: 'any' },
    },
  },
  relayLocations: [],
  wireguardEndpointData: { portRanges: [], udp2tcpPorts: [] },
  allowLan: false,
  enableIpv6: true,
  lockdownMode: false,
  showBetaReleases: false,
  warrenApiUrl: undefined,
  warrenMaxRateBps: undefined,
  warrenMultiHop: {
    enabled: false,
    entryCountry: '',
    exitCountry: '',
    hpkeEpochRotationMs: 4 * 60 * 60 * 1000,
  },
  warrenCustomExit: {
    enabled: false,
    endpoint: '',
    pubkeyHex: '',
    coverDomain: undefined,
    label: '',
    x25519MultihopPubkeyHex: '',
    exitIdHex: '',
  },
  warrenStatus: undefined,
  warrenNatPmp: {
    enabled: false,
    lifetimeSecs: 3600,
    rules: [],
    protocol: NatPmpProto.udp,
    suggestedExternalPort: 0,
    internalPort: 0,
  },
  natPmpStatus: undefined,
  natPmpStatusReceivedAt: undefined,
  wireguard: {
    quantumResistant: true,
  },
  dns: {
    state: 'default',
    allowExternalDns: false,
    defaultOptions: {
      blockAds: false,
      blockTrackers: false,
      blockMalware: false,
      blockAdultContent: false,
      blockGambling: false,
      blockSocialMedia: false,
    },
    customOptions: {
      addresses: [],
    },
  },
  splitTunneling: false,
  splitTunnelingApplications: [],
  splitTunnelingSupported: false,
  obfuscationSettings: {
    selectedObfuscation: ObfuscationType.auto,
    udp2tcpSettings: {
      port: 'any',
    },
    shadowsocksSettings: {
      port: 'any',
    },
    lwoSettings: {
      port: 'any',
    },
  },
  customLists: [],
  recents: undefined,
  apiAccessMethods: getDefaultApiAccessMethods(),
  currentApiAccessMethod: undefined,
  relayOverrides: [],
};

export default function (
  state: ISettingsReduxState = initialState,
  action: ReduxAction,
): ISettingsReduxState {
  switch (action.type) {
    case 'UPDATE_GUI_SETTINGS':
      return {
        ...state,
        guiSettings: action.guiSettings,
      };

    case 'UPDATE_RELAY':
      return {
        ...state,
        relaySettings: action.relay,
      };

    case 'UPDATE_RELAY_LOCATIONS':
      return {
        ...state,
        relayLocations: action.relayLocations,
      };

    case 'UPDATE_WIREGUARD_ENDPOINT_DATA':
      return {
        ...state,
        wireguardEndpointData: action.wireguardEndpointData,
      };

    case 'UPDATE_ALLOW_LAN':
      return {
        ...state,
        allowLan: action.allowLan,
      };

    case 'UPDATE_WARREN_API_URL':
      return {
        ...state,
        warrenApiUrl: action.warrenApiUrl,
      };

    case 'UPDATE_WARREN_MAX_RATE_BPS':
      return {
        ...state,
        warrenMaxRateBps: action.warrenMaxRateBps,
      };

    case 'UPDATE_WARREN_MULTI_HOP':
      return {
        ...state,
        warrenMultiHop: action.warrenMultiHop,
      };

    case 'UPDATE_WARREN_CUSTOM_EXIT':
      return {
        ...state,
        warrenCustomExit: action.warrenCustomExit,
      };

    case 'UPDATE_WARREN_STATUS':
      return {
        ...state,
        warrenStatus: action.warrenStatus,
      };

    case 'UPDATE_NAT_PMP_SETTINGS':
      return {
        ...state,
        warrenNatPmp: action.natPmpSettings,
      };

    case 'UPDATE_NAT_PMP_STATUS':
      return {
        ...state,
        natPmpStatus: action.natPmpStatus,
        natPmpStatusReceivedAt: action.receivedAt,
      };

    case 'UPDATE_ENABLE_IPV6':
      return {
        ...state,
        enableIpv6: action.enableIpv6,
      };

    case 'UPDATE_LOCKDOWN_MODE':
      return {
        ...state,
        lockdownMode: action.lockdownMode,
      };

    case 'UPDATE_SHOW_BETA_NOTIFICATIONS':
      return {
        ...state,
        showBetaReleases: action.showBetaReleases,
      };

    case 'UPDATE_WIREGUARD_MTU':
      return {
        ...state,
        wireguard: {
          ...state.wireguard,
          mtu: action.mtu,
        },
      };

    case 'UPDATE_WIREGUARD_QUANTUM_RESISTANT':
      return {
        ...state,
        wireguard: {
          ...state.wireguard,
          quantumResistant: action.quantumResistant,
        },
      };
    case 'UPDATE_WIREGUARD_DAITA':
      return {
        ...state,
        wireguard: {
          ...state.wireguard,
          daita: action.daita,
        },
      };

    case 'UPDATE_AUTO_START':
      return {
        ...state,
        autoStart: action.autoStart,
      };

    case 'UPDATE_DNS_OPTIONS':
      return {
        ...state,
        dns: action.dns,
      };

    case 'UPDATE_SPLIT_TUNNELING_STATE':
      return {
        ...state,
        splitTunneling: action.enabled,
      };

    case 'SET_SPLIT_TUNNELING_APPLICATIONS':
      return {
        ...state,
        splitTunnelingApplications: action.applications,
      };

    case 'SET_SPLIT_TUNNELING_SUPPORTED':
      return {
        ...state,
        splitTunnelingSupported: action.supported,
      };

    case 'SET_OBFUSCATION_SETTINGS':
      return {
        ...state,
        obfuscationSettings: action.obfuscationSettings,
      };

    case 'SET_CUSTOM_LISTS':
      return {
        ...state,
        customLists: action.customLists,
      };

    case 'SET_RECENTS':
      return {
        ...state,
        recents: action.recents,
      };

    case 'SET_API_ACCESS_METHODS':
      return {
        ...state,
        apiAccessMethods: action.accessMethods,
      };

    case 'SET_CURRENT_API_ACCESS_METHOD':
      return {
        ...state,
        currentApiAccessMethod: action.accessMethod,
      };

    case 'SET_RELAY_OVERRIDES':
      return {
        ...state,
        relayOverrides: action.relayOverrides,
      };

    default:
      return state;
  }
}
