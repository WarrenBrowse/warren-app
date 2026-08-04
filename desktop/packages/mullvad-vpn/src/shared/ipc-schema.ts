import { GetTextTranslations } from 'gettext-parser';

import { MapData } from '../renderer/lib/3dmap';
import { AppUpgradeError, AppUpgradeEvent } from './app-upgrade';
import { ILinuxSplitTunnelingApplication, ISplitTunnelingApplication } from './application-types';
import {
  AccessMethodExistsError,
  AccessMethodSetting,
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
  WarrenCustomExitSettings,
  WarrenMultiHopSettings,
  WarrenPubKey,
  WarrenPubkeyMismatch,
  WarrenStatus,
} from './daemon-rpc-types';
import { ForumAttachResult, IForumAttachRequest } from './forum-attach';
import { ForumLoginResult, IForumLoginRequest } from './forum-login';
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
import { RenewalUiState } from './renewal';
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
  // Whether the daemon/platform supports split tunneling at all. Part of
  // the initial snapshot so the renderer has a deterministic value at
  // boot (the `notifyIsSupported` push alone races a window that opens
  // after the daemon already bootstrapped).
  splitTunnelingSupported: boolean;
  macOsScrollbarVisibility?: MacOsScrollbarVisibility;
  changelog: IChangelog;
  navigationHistory?: IHistoryObject;
  currentApiAccessMethod?: AccessMethodSetting;
  isMacOs13OrNewer: boolean;
  purchaseInFlight: boolean;
  // This wallet's community-forum handle, absent until the user has signed in
  // to the forum once (the derivation is keyed server side, so the app cannot
  // compute it).
  forumHandle?: string;
  // Latest Warren live status known to the main process. Part of the initial
  // snapshot for the same reason as `splitTunnelingSupported`: the daemon's
  // status stream delivers its current value once, at subscribe time, which
  // happens during the main-process bootstrap and therefore BEFORE the
  // renderer has registered its listener. Fields that keep changing while the
  // app runs (failover count, host offline) self-heal on the next push; a
  // rarely-changing one (a published notice, the network descriptor) would
  // otherwise stay invisible until it next changed, which for a notice can be
  // never.
  warrenStatus?: WarrenStatus;
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
  // Community-forum wallet login (doc 55). `request` is pushed when a
  // `warren://forum-login` deep link arrives; the renderer shows a consent
  // prompt and calls `approve` or `cancel`. Never a silent external login.
  forumLogin: {
    request: notifyRenderer<IForumLoginRequest>(),
    // A deep link that cold-starts the app fires before the renderer exists;
    // the prompt fetches the buffered request on mount instead.
    getPending: invoke<void, IForumLoginRequest | undefined>(),
    approve: invoke<IForumLoginRequest, ForumLoginResult>(),
    cancel: invoke<IForumLoginRequest, void>(),
    // The derived forum handle of this wallet, known only once the user has
    // signed in to the forum at least once (the derivation is keyed server
    // side). Pushed on change so the account view updates without a restart.
    handle: notifyRenderer<string | undefined>(),
    getHandle: invoke<void, string | undefined>(),
  },
  // Community-forum attach-logs (doc 55). `request` is pushed when a
  // `warren://attach-logs` deep link arrives; the renderer shows a consent
  // prompt (with a "view the logs" action backed by problemReport.viewLog)
  // and calls `approve` or `cancel`. Logs are never sent silently.
  forumAttach: {
    request: notifyRenderer<IForumAttachRequest>(),
    getPending: invoke<void, IForumAttachRequest | undefined>(),
    approve: invoke<IForumAttachRequest, ForumAttachResult>(),
    cancel: invoke<IForumAttachRequest, void>(),
  },
  daemon: {
    isPerformingPostUpgrade: notifyRenderer<boolean>(),
    daemonAllowed: notifyRenderer<boolean>(),
    connected: notifyRenderer<void>(),
    disconnected: notifyRenderer<void>(),
    prepareRestart: send<boolean>(),
    tryStart: send<void>(),
    tryStartEvent: notifyRenderer<DaemonStatus>(),
    unblockNetwork: invoke<void, boolean>(),
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
    // Clears the system clipboard, but only if it still holds the
    // provided value (the just-copied mnemonic). Runs in the main
    // process where Electron's `clipboard` module bypasses the
    // renderer permission handler, which only grants
    // `clipboard-sanitized-write` and so rejects a renderer-side
    // `readText()`. Returns whether the clipboard was actually cleared.
    clearMnemonicFromClipboard: invoke<string, boolean>(),
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
  // Warren live status (auto-reconnect counter + age, obfuscation
  // indicator). The main process subscribes to the daemon
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
    // Persistent warren-api URL (empty string = unset). Daemon restart
    // required to apply.
    setWarrenApiUrl: invoke<string, void>(),
    // Client-side bandwidth ceiling in bits per second (undefined =
    // unlimited). Applies to a live tunnel without a reconnect.
    setWarrenMaxRateBps: invoke<number | undefined, void>(),
    // Warren multi-hop settings. Daemon restart required.
    setWarrenMultiHop: invoke<WarrenMultiHopSettings, void>(),
    // Advanced Warren "custom exit" override. The daemon reconnects on
    // change (no restart needed).
    setWarrenCustomExit: invoke<WarrenCustomExitSettings, void>(),
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
    // Trust the new pubkey served for the
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
    // Onboarding wizard: persist the completion timestamp.
    // Passing `undefined` clears it so the wizard re-runs on the next
    // boot (used by the Settings "Replay onboarding" entry).
    setOnboardingCompletedUnix: send<number | undefined>(),
    // Persisted backup gate. Set true when a fresh identity is minted
    // and awaiting recovery-phrase backup, cleared once the backup is
    // confirmed, so a GUI restart mid-backup can re-route to the
    // backup-pending state instead of the main view.
    setBackupPending: send<boolean>(),
  },
  account: {
    '': notifyRenderer<IAccountData | undefined>(),
    device: notifyRenderer<DeviceEvent>(),
    create: invoke<void, string>(),
    logout: invoke<LogoutSource, void>(),
    // Returns the BIP39 mnemonic (12 words) so the user can back it
    // up. Empty string if the identity has never been bootstrapped.
    // The renderer caller must display it with a safety warning and
    // explicit user confirmation.
    getWarrenMnemonic: invoke<void, string>(),
    // Restores an identity from the provided BIP39 mnemonic. BIP39
    // validation is performed daemon-side. Throws if invalid (=
    // caller must catch + show error). The daemon hot-swaps the
    // identity and logs in without requiring a restart.
    setWarrenMnemonic: invoke<string, void>(),
    submitVoucher: invoke<string, VoucherResponse>(),
    updateData: invoke<void, void>(),
    // Starts an app-initiated purchase (doc 35): the main process
    // mints the wpid, opens the checkout in the browser and owns the
    // auto-redeem poll (the renderer is background-throttled while
    // the user pays, so the poll cannot live there).
    buyCredit: invoke<void, void>(),
    // Forces an immediate redeem attempt of every pending purchase
    // (bypasses the focus-check throttle). Wired to the manual
    // "I've completed payment" buttons.
    checkPendingPurchases: invoke<void, void>(),
    // True while the main-process purchase poll is active; drives the
    // "Checking..." labels in the paywall views.
    purchase: notifyRenderer<boolean>(),
    // Auto-renewal display state (warren-core doc 65): undefined when no mandate
    // is held for the logged-in account.
    renewal: notifyRenderer<RenewalUiState | undefined>(),
    getRenewalState: invoke<void, RenewalUiState | undefined>(),
    disableRenewal: invoke<void, void>(),
  },
  accountHistory: {
    '': notifyRenderer<WarrenPubKey | undefined>(),
  },
  autoStart: {
    '': notifyRenderer<boolean>(),
    set: invoke<boolean, void>(),
  },
  problemReport: {
    // Only log viewing remains renderer-facing: the forum attach-logs
    // consent prompt opens the collected report for inspection. Collection
    // happens in the main process (forum-attach deep link) and the old
    // "send report" upload channel is gone, the forum is the support
    // front door.
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
