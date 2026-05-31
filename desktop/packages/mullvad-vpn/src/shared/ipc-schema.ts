import { GetTextTranslations } from 'gettext-parser';

import { MapData } from '../renderer/lib/3dmap';
import { AppUpgradeError, AppUpgradeEvent } from './app-upgrade';
import { ILinuxSplitTunnelingApplication, ISplitTunnelingApplication } from './application-types';
import {
  AccessMethodExistsError,
  AccessMethodSetting,
  AccountDataError,
  CustomListError,
  CustomProxy,
  DeviceEvent,
  DeviceState,
  DisconnectSource,
  IAccountData,
  IAppVersionInfo,
  ICustomList,
  IDnsOptions,
  IRelayListWithEndpointData,
  ISettings,
  LogoutSource,
  NatPmpSettings,
  NatPmpStatus,
  NewAccessMethodSetting,
  NewCustomList,
  ObfuscationSettings,
  RelaySettings,
  TrustNewExitKeyOutcome,
  TunnelState,
  VoucherResponse,
  WarrenFailoverSettings,
  WarrenMultiHopSettings,
  WarrenPubKey,
  WarrenPubkeyMismatch,
  WarrenStatus,
} from './daemon-rpc-types';
import { IGuiSettingsState } from './gui-settings-state';
import { invoke, invokeSync, notifyRenderer, send } from './ipc-helpers';
import {
  DaemonStatus,
  IChangelog,
  ICurrentAppVersionInfo,
  IHistoryObject,
  IWindowShapeParameters,
} from './ipc-types';
import { LogLevel } from './logging-types';
import { RoutePath } from './routes';

interface ILogEntry {
  level: LogLevel;
  message: string;
}

export interface ITranslations {
  locale: string;
  messages?: GetTextTranslations;
  relayLocations?: GetTextTranslations;
}

export type LaunchApplicationResult = { success: true } | { error: string };

export enum MacOsScrollbarVisibility {
  always,
  whenScrolling,
  automatic,
}

export interface IAppStateSnapshot {
  isConnected: boolean;
  autoStart: boolean;
  accountData?: IAccountData;
  accountHistory?: WarrenPubKey;
  tunnelState: TunnelState;
  settings: ISettings;
  isPerformingPostUpgrade: boolean;
  daemonAllowed?: boolean;
  deviceState?: DeviceState;
  relayList?: IRelayListWithEndpointData;
  currentVersion: ICurrentAppVersionInfo;
  upgradeVersion: IAppVersionInfo;
  guiSettings: IGuiSettingsState;
  translations: ITranslations;
  splitTunnelingApplications?: ISplitTunnelingApplication[];
  macOsScrollbarVisibility?: MacOsScrollbarVisibility;
  changelog: IChangelog;
  navigationHistory?: IHistoryObject;
  currentApiAccessMethod?: AccessMethodSetting;
  isMacOs13OrNewer: boolean;
}

export type IpcSchema = typeof ipcSchema;

// The different types of requests are:
// * send<ArgumentType>(), which is used for one-way communication from the renderer process to the
//    main process. The main channel will have a property named 'handle<PropertyName>' and the
//    renderer will have a property named the same as the one specified.
// * invoke<ArgumentType, ReturnType>(), which is used for two-way communication from the renderer
//    process to the main process. The naming is the same as `send<A>()`.
// * invokeSync<ArgumentType, ReturnType>(), same as `invoke<A, R>()` but synchronous.
// * notifyRenderer<ArgumentType>(), which is used for one-way communication from the main process
//    to the renderer process. The renderer ipc channel will have a property named
//    `listen<PropertyName>` and the main channel will have a property named `notify<PropertyName>`.
//
// Example:
// const ipc = {
//   groupOfCalls: {
//     first: send<boolean>(),
//     second: request<boolean, number>(),
//     third: requestSync<boolean, number>(),
//     fourth: notifyRenderer<boolean>(),
//   },
// };
//
// createIpcMain(ipc)
//   => {
//     groupOfCalls: {
//       handleFirst: (fn: (arg: boolean) => void) => void,
//       handleSecond: (fn: (arg: boolean) => Promise<number>) => void,
//       handleThird: (fn: (arg: boolean) => number) => void,
//       notifyFourth: (arg: boolean) => void,
//     },
//
// createIpcRenderer(ipc)
//   => {
//     groupOfCalls: {
//       first: (arg: boolean) => void,
//       second: (arg: boolean) => Promise<number>,
//       third: (arg: boolean) => number,
//       listenFourth: (fn: (arg: boolean) => void) => void,
//     },
//   }
export const ipcSchema = {
  state: {
    get: invokeSync<void, IAppStateSnapshot>(),
  },
  map: {
    getData: invoke<void, MapData>(),
  },
  window: {
    shape: notifyRenderer<IWindowShapeParameters>(),
    focus: notifyRenderer<boolean>(),
    macOsScrollbarVisibility: notifyRenderer<MacOsScrollbarVisibility>(),
    scaleFactorChange: notifyRenderer<void>(),
  },
  navigation: {
    reset: notifyRenderer<void>(),
    setHistory: send<IHistoryObject>(),
  },
  daemon: {
    isPerformingPostUpgrade: notifyRenderer<boolean>(),
    daemonAllowed: notifyRenderer<boolean>(),
    connected: notifyRenderer<void>(),
    disconnected: notifyRenderer<void>(),
    prepareRestart: send<boolean>(),
    tryStart: send<void>(),
    tryStartEvent: notifyRenderer<DaemonStatus>(),
  },
  relays: {
    '': notifyRenderer<IRelayListWithEndpointData>(),
  },
  customLists: {
    createCustomList: invoke<NewCustomList, void | CustomListError>(),
    deleteCustomList: invoke<string, void>(),
    updateCustomList: invoke<ICustomList, void | CustomListError>(),
  },
  currentVersion: {
    '': notifyRenderer<ICurrentAppVersionInfo>(),
    displayedChangelog: send<void>(),
  },
  upgradeVersion: {
    '': notifyRenderer<IAppVersionInfo>(),
    dismissedUpgrade: send<string>(),
  },
  app: {
    quit: send<DisconnectSource>(),
    openUrl: invoke<string, void>(),
    openRoute: notifyRenderer<RoutePath>(),
    showOpenDialog: invoke<Electron.OpenDialogOptions, Electron.OpenDialogReturnValue>(),
    showLaunchDaemonSettings: invoke<void, void>(),
    showFullDiskAccessSettings: invoke<void, void>(),
    getPathBaseName: invoke<string, string>(),
    upgrade: send<void>(),
    upgradeAbort: send<void>(),
    upgradeEvent: notifyRenderer<AppUpgradeEvent>(),
    upgradeError: notifyRenderer<AppUpgradeError>(),
    upgradeInstallerStart: send<void>(),
    getUpgradeCacheDir: invoke<void, string>(),
  },
  tunnel: {
    '': notifyRenderer<TunnelState>(),
    connect: invoke<void, void>(),
    disconnect: invoke<DisconnectSource, void>(),
    reconnect: invoke<void, void>(),
  },
  // Warren live status (M4.E.D auto-reconnect counter + age, M4.0
  // obfuscation indicator). The main process subscribes to the daemon
  // WarrenStatusUpdates push stream and forwards every snapshot via
  // this channel; the renderer dispatches it into the redux store.
  warrenStatus: {
    '': notifyRenderer<WarrenStatus>(),
  },
  // NAT-PMP port-forwarding live status. Same pattern as
  // `warrenStatus`: main subscribes to the daemon NatPmpStatusUpdates
  // stream, forwards each snapshot via this channel, renderer
  // dispatches into the redux store, and the port-forwarding settings
  // view rerenders with the current port + countdown.
  natPmpStatus: {
    '': notifyRenderer<NatPmpStatus>(),
  },
  settings: {
    '': notifyRenderer<ISettings>(),
    importFile: invoke<string, void>(),
    importText: invoke<string, void>(),
    apiAccessMethodSettingChange: notifyRenderer<AccessMethodSetting>(),
    setAllowLan: invoke<boolean, void>(),
    // Persistent toggle for Warren local account mode.
    setWarrenLocalAccount: invoke<boolean, void>(),
    // Persistent warren-api URL (empty string = unset). Daemon restart
    // required to apply.
    setWarrenApiUrl: invoke<string, void>(),
    // Warren multi-hop settings (M4.E.D). Daemon restart required.
    setWarrenMultiHop: invoke<WarrenMultiHopSettings, void>(),
    // Warren multi-exit failover toggle (M5.B.2). Picked up on the
    // next tunnel reconnect (no restart needed). M5.B.1 DAITA toggle
    // reuses Mullvad upstream's `setEnableDaita` rather than
    // introducing a dedicated Warren-side switch.
    setWarrenFailover: invoke<WarrenFailoverSettings, void>(),
    // Warren NAT-PMP port-forwarding settings. Daemon picks up the
    // new value on the NEXT tunnel reconnect (no restart needed).
    setNatPmpSettings: invoke<NatPmpSettings, void>(),
    setShowBetaReleases: invoke<boolean, void>(),
    setEnableIpv6: invoke<boolean, void>(),
    setLockdownMode: invoke<boolean, void>(),
    setWireguardMtu: invoke<number | undefined, void>(),
    setWireguardQuantumResistant: invoke<boolean, void>(),
    setRelaySettings: invoke<RelaySettings, void>(),
    setDnsOptions: invoke<IDnsOptions, void>(),
    setObfuscationSettings: invoke<ObfuscationSettings, void>(),
    addApiAccessMethod: invoke<NewAccessMethodSetting, string | AccessMethodExistsError>(),
    updateApiAccessMethod: invoke<AccessMethodSetting, void | AccessMethodExistsError>(),
    removeApiAccessMethod: invoke<string, void>(),
    setApiAccessMethod: invoke<string, void>(),
    testApiAccessMethodById: invoke<string, boolean>(),
    testCustomApiAccessMethod: invoke<CustomProxy, boolean>(),
    clearAllRelayOverrides: invoke<void, void>(),
    setEnableDaita: invoke<boolean, void>(),
    setDaitaDirectOnly: invoke<boolean, void>(),
    setEnableRecents: invoke<boolean, void>(),
    // Session H A.4: trust the new pubkey served for the
    // `exitIdHex` so future connects to that exit accept it as the
    // baseline. The daemon clears the pending mismatch from
    // WarrenStatus on success.
    trustNewExitKey: invoke<{ exitIdHex: string; newPubkeyHex: string }, TrustNewExitKeyOutcome>(),
    // Clear the entire TOFU pin table. Used by Settings -> "Reset
    // pinned exit keys" when the user wants a fresh baseline (e.g.
    // after switching identity / device).
    resetPinnedExitKeys: invoke<void, number>(),
    // Dismiss the pending pubkey mismatch without trusting the new
    // key. Daemon clears `pubkeyMismatchPending` on WarrenStatus so
    // the modal unmounts; the existing pin is preserved.
    dismissPubkeyMismatch: invoke<void, void>(),
    // Post a forensic report about the mismatch to warren-api
    // (best-effort, no PII). Daemon clears `pubkeyMismatchPending`
    // regardless of the report outcome.
    reportPubkeyMismatch: invoke<WarrenPubkeyMismatch, void>(),
  },
  guiSettings: {
    '': notifyRenderer<IGuiSettingsState>(),
    setEnableSystemNotifications: send<boolean>(),
    setAutoConnect: send<boolean>(),
    setStartMinimized: send<boolean>(),
    setMonochromaticIcon: send<boolean>(),
    setPreferredLocale: invoke<string, ITranslations>(),
    setUnpinnedWindow: send<boolean>(),
    setAnimateMap: send<boolean>(),
    // M5.B.3 onboarding wizard: persist the completion timestamp.
    // Passing `undefined` clears it so the wizard re-runs on the next
    // boot (used by the Settings "Replay onboarding" entry).
    setOnboardingCompletedUnix: send<number | undefined>(),
  },
  account: {
    '': notifyRenderer<IAccountData | undefined>(),
    device: notifyRenderer<DeviceEvent>(),
    create: invoke<void, string>(),
    login: invoke<WarrenPubKey, AccountDataError | undefined>(),
    logout: invoke<LogoutSource, void>(),
    getWwwAuthToken: invoke<void, string>(),
    // Returns the BIP39 mnemonic (12 words) so the user can back it
    // up. Empty string if the identity has never been bootstrapped.
    // The renderer caller must display it with a safety warning and
    // explicit user confirmation.
    getWarrenMnemonic: invoke<void, string>(),
    // Replaces the identity with the provided BIP39 mnemonic. BIP39
    // validation is performed daemon-side. Throws if invalid (=
    // caller must catch + show error). Daemon restart is required to
    // activate.
    setWarrenMnemonic: invoke<string, void>(),
    submitVoucher: invoke<string, VoucherResponse>(),
    updateData: invoke<void, void>(),
  },
  accountHistory: {
    '': notifyRenderer<WarrenPubKey | undefined>(),
    clear: invoke<void, void>(),
  },
  autoStart: {
    '': notifyRenderer<boolean>(),
    set: invoke<boolean, void>(),
  },
  problemReport: {
    collectLogs: invoke<string | undefined, string>(),
    sendReport: invoke<{ email: string; message: string; savedReportId: string }, void>(),
    viewLog: invoke<string, string>(),
  },
  logging: {
    log: send<ILogEntry>(),
  },
  linuxSplitTunneling: {
    getApplications: invoke<void, ILinuxSplitTunnelingApplication[]>(),
    launchApplication: invoke<ILinuxSplitTunnelingApplication | string, LaunchApplicationResult>(),
  },
  macOsSplitTunneling: {
    needFullDiskPermissions: invoke<void, boolean>(),
  },
  splitTunneling: {
    '': notifyRenderer<ISplitTunnelingApplication[]>(),
    setState: invoke<boolean, void>(),
    getApplications: invoke<
      boolean,
      { fromCache: boolean; applications: ISplitTunnelingApplication[] }
    >(),
    addApplication: invoke<ISplitTunnelingApplication | string, void>(),
    removeApplication: invoke<ISplitTunnelingApplication, void>(),
    forgetManuallyAddedApplication: invoke<ISplitTunnelingApplication, void>(),
    getSupported: invoke<void, boolean>(),
    isSupported: notifyRenderer<boolean>(),
  },
};
