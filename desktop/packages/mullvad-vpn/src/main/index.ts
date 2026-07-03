import { exec, execFile } from 'child_process';
import {
  app,
  clipboard,
  nativeTheme,
  powerMonitor,
  session,
  shell,
  systemPreferences,
} from 'electron';
import fs from 'fs';
import * as path from 'path';
import util from 'util';

import { hasExpired } from '../shared/account-expiry';
import {
  ISplitTunnelingApplication,
  ISplitTunnelingAppListRetriever,
} from '../shared/application-types';
import { urls } from '../shared/constants';
import {
  AccessMethodSetting,
  DaemonAppUpgradeEvent,
  DaemonEvent,
  DeviceEvent,
  DisconnectSource,
  IRelayListWithEndpointData,
  ISettings,
  NatPmpStatus,
  TunnelState,
  WarrenStatus,
} from '../shared/daemon-rpc-types';
import { messages, relayLocations } from '../shared/gettext';
import { SYSTEM_PREFERRED_LOCALE_KEY } from '../shared/gui-settings-state';
import { ITranslations, MacOsScrollbarVisibility } from '../shared/ipc-schema';
import { IChangelog, IHistoryObject } from '../shared/ipc-types';
import log, { ConsoleOutput, Logger } from '../shared/logging';
import { LogLevel } from '../shared/logging-types';
import {
  SystemNotification,
  SystemNotificationCategory,
} from '../shared/notifications/notification';
import { RoutePath } from '../shared/routes';
import Account, { AccountDelegate, LocaleProvider } from './account';
import AppUpgrade from './app-upgrade';
import { getOpenAtLogin } from './autostart';
import { readChangelog } from './changelog';
import {
  CommandLineOptions,
  printCommandLineOptions,
  printElectronOptions,
} from './command-line-options';
import { DaemonRpc, SubscriptionListener } from './daemon-rpc';
import Expectation from './expectation';
import {
  approveForumLogin,
  cancelForumLogin,
  findForumLoginArg,
  FORUM_DEEP_LINK_SCHEME,
  parseForumLoginUrl,
} from './forum-login';
import { ConnectionObserver } from './grpc-client';
import { IpcMainEventChannel } from './ipc-event-channel';
import { findIconPath } from './linux-desktop-entry';
import { loadTranslations } from './load-translations';
import {
  backupLogFile,
  cleanUpLogDirectory,
  createLoggingDirectory,
  FileOutput,
  getMainLogPath,
  getRendererLogPath,
  IpcInput,
  OLD_LOG_FILES,
} from './logging';
import NotificationController, {
  NotificationControllerDelegate,
  NotificationSender,
} from './notification-controller';
import { isMacOs13OrNewer } from './platform-version';
import * as problemReport from './problem-report';
import { resolveBin } from './proc';
import ReconnectionBackoff from './reconnection-backoff';
import Settings, { SettingsDelegate } from './settings';
import TunnelStateHandler, {
  TunnelStateHandlerDelegate,
  TunnelStateProvider,
} from './tunnel-state';
import UserInterface, { UserInterfaceDelegate } from './user-interface';
import Version, { GUI_VERSION } from './version';

const execAsync = util.promisify(exec);

const ALLOWED_PERMISSIONS = ['clipboard-sanitized-write'];

const SANDBOX_DISABLED = app.commandLine.hasSwitch('no-sandbox');
const UPDATE_NOTIFICATION_DISABLED = process.env.MULLVAD_DISABLE_UPDATE_NOTIFICATION === '1';

const GEO_DIR = path.resolve(import.meta.dirname, 'assets/geo');

class ApplicationMain
  implements
    NotificationSender,
    TunnelStateProvider,
    LocaleProvider,
    NotificationControllerDelegate,
    UserInterfaceDelegate,
    TunnelStateHandlerDelegate,
    SettingsDelegate,
    AccountDelegate
{
  private daemonRpc: DaemonRpc;

  private notificationController = new NotificationController(this);
  private version: Version;
  private settings: Settings;
  private account: Account;
  private appUpgrade: AppUpgrade;
  private userInterface?: UserInterface;
  private tunnelState = new TunnelStateHandler(this);

  private daemonEventListener?: SubscriptionListener<DaemonEvent>;
  private daemonAppUpgradeEventListener?: SubscriptionListener<DaemonAppUpgradeEvent>;
  // Subscription to the daemon WarrenStatusUpdates push stream.
  // Created on every successful daemon reconnect and torn down on
  // disconnect, mirroring the daemonEventListener lifecycle.
  private warrenStatusListener?: SubscriptionListener<WarrenStatus>;
  // Subscription to the daemon NatPmpStatusUpdates push stream.
  // Same lifecycle as warrenStatusListener.
  private natPmpStatusListener?: SubscriptionListener<NatPmpStatus>;
  private reconnectBackoff = new ReconnectionBackoff();
  private beforeFirstDaemonConnection = true;
  private isPerformingPostUpgrade = false;
  private daemonAllowed?: boolean;
  private quitInitiated = false;

  private linuxSplitTunneling?: typeof import('./linux-split-tunneling');
  private splitTunneling?: ISplitTunnelingAppListRetriever;
  private splitTunnelingSupported = false;

  private tunnelStateExpectation?: Expectation;

  // The UI locale which is set once from onReady handler
  private locale = 'en';

  private rendererLog?: Logger;
  private translations: ITranslations = { locale: this.locale };

  private splitTunnelingApplications?: ISplitTunnelingApplication[];

  private macOsScrollbarVisibility?: MacOsScrollbarVisibility;

  private changelog?: IChangelog;

  private navigationHistory?: IHistoryObject;

  private relayList?: IRelayListWithEndpointData;

  private currentApiAccessMethod?: AccessMethodSetting;

  public constructor() {
    this.daemonRpc = new DaemonRpc(
      new ConnectionObserver(this.onDaemonConnected, this.onDaemonDisconnected),
    );

    this.version = new Version(this, this.daemonRpc, UPDATE_NOTIFICATION_DISABLED);
    this.settings = new Settings(this, this.daemonRpc, this.version.currentVersion);
    this.account = new Account(this, this.daemonRpc);
    this.appUpgrade = new AppUpgrade(this.daemonRpc);
  }

  public run() {
    // Remove window animations to combat window flickering when opening window. Can be removed when
    // this issue has been resolved: https://github.com/electron/electron/issues/12130
    if (process.platform === 'win32') {
      app.commandLine.appendSwitch('wm-window-animations-disabled');
    }

    if (process.platform === 'darwin') {
      app.commandLine.appendSwitch('disable-geolocation');
    }

    if (process.platform === 'linux') {
      // NOTE: Keep in sync with mocked-utils.ts
      app.commandLine.appendSwitch('gtk-version', '3');
    }

    // Display correct colors regardless of monitor color profile.
    app.commandLine.appendSwitch('force-color-profile', 'srgb');

    this.overrideAppPaths();

    // This ensures that only a single instance is running at the same time.
    if (!app.requestSingleInstanceLock()) {
      app.quit();
      return;
    }

    this.addSecondInstanceEventHandler();
    this.registerForumLoginDeepLink();

    this.initLogging();

    log.verbose(`Chromium sandbox is ${SANDBOX_DISABLED ? 'disabled' : 'enabled'}`);
    if (!SANDBOX_DISABLED) {
      app.enableSandbox();
    }

    log.info(`Running version ${this.version.currentVersion.gui}`);

    if (process.platform === 'win32') {
      app.setAppUserModelId('com.warrenbrowse.vpn');
    }

    // While running in development the watch script triggers a reload of the renderer by sending
    // the signal `SIGUSR2`.
    if (process.env.NODE_ENV === 'development') {
      process.on('SIGUSR2', () => {
        this.userInterface?.reloadWindow();
      });
    }

    this.settings.gui.load();
    this.changelog = readChangelog();

    app.on('render-process-gone', (_event, _webContents, details) => {
      log.error(
        `Render process exited with exit code ${details.exitCode} due to ${details.reason}`,
      );
      app.quit();
    });
    app.on('child-process-gone', (_event, details) => {
      log.error(
        `Child process of type ${details.type} exited with exit code ${details.exitCode} due to ${details.reason}`,
      );
    });

    app.on('ready', this.onReady);

    app.on('before-quit', this.onBeforeQuit);
    app.on('will-quit', () => {
      log.info('will-quit received');
      this.onQuit();
    });
    app.on('quit', () => {
      log.info('quit received');
      this.onQuit();
    });
  }

  public async performPostUpgradeCheck(): Promise<void> {
    const oldValue = this.isPerformingPostUpgrade;
    this.isPerformingPostUpgrade = await this.daemonRpc.isPerformingPostUpgrade();
    if (this.isPerformingPostUpgrade !== oldValue) {
      IpcMainEventChannel.daemon.notifyIsPerformingPostUpgrade?.(this.isPerformingPostUpgrade);
    }
  }

  public connectTunnel = async (): Promise<void> => {
    if (this.tunnelState.allowConnect(this.daemonRpc.isConnected, this.account.isLoggedIn())) {
      this.tunnelState.expectNextTunnelState('connecting');
      await this.daemonRpc.connectTunnel();
    }
  };

  public reconnectTunnel = async (): Promise<void> => {
    if (this.tunnelState.allowReconnect(this.daemonRpc.isConnected, this.account.isLoggedIn())) {
      this.tunnelState.expectNextTunnelState('connecting');
      await this.daemonRpc.reconnectTunnel();
    }
  };

  public disconnectTunnel = async (source: DisconnectSource): Promise<void> => {
    if (this.tunnelState.allowDisconnect(this.daemonRpc.isConnected)) {
      this.tunnelState.expectNextTunnelState('disconnecting');
      await this.daemonRpc.disconnectTunnel(source);
    }
  };

  public isLoggedIn = () => this.account.isLoggedIn();

  public disconnectAndQuit = async (source: DisconnectSource) => {
    if (this.daemonRpc.isConnected) {
      try {
        await this.daemonRpc.disconnectTunnel(source);
        log.info('Disconnected the tunnel');
      } catch (e) {
        const error = e as Error;
        log.error(`Failed to disconnect the tunnel: ${error.message}`);
      }
    } else {
      log.info('Cannot close the tunnel because there is no active connection to daemon.');
    }

    app.quit();
  };

  private addSecondInstanceEventHandler() {
    app.on('second-instance', (_event, argv, _workingDirectory) => {
      this.userInterface?.showWindow();
      // Windows/Linux deliver a deep link to the already-running instance as
      // an argv entry on the second launch.
      const deepLink = findForumLoginArg(argv);
      if (deepLink) {
        void this.handleForumLoginDeepLink(deepLink);
      }
    });
  }

  // Community-forum wallet login (doc 55). Registers the `warren://` scheme and
  // wires the three delivery paths: macOS `open-url`, Windows/Linux argv on the
  // second instance (see above) and on first launch (checked in onReady).
  private registerForumLoginDeepLink() {
    app.setAsDefaultProtocolClient(FORUM_DEEP_LINK_SCHEME);
    app.on('open-url', (event, url) => {
      event.preventDefault();
      this.userInterface?.showWindow();
      void this.handleForumLoginDeepLink(url);
    });
  }

  // A `warren://forum-login` deep link never logs in on its own: it surfaces a
  // consent prompt in the renderer (approve/cancel), and only an explicit
  // approval signs and submits (handled via IPC below).
  private handleForumLoginDeepLink(url: string) {
    const request = parseForumLoginUrl(url);
    if (!request) {
      log.warn('Ignoring malformed or non-allowlisted forum-login deep link');
      return;
    }
    this.userInterface?.showWindow();
    IpcMainEventChannel.forumLogin.notifyRequest?.(request);
  }

  private overrideAppPaths() {
    // This ensures that on Windows the %LOCALAPPDATA% directory is used instead of the %ADDDATA%
    // directory that has roaming contents
    if (process.platform === 'win32') {
      const appDataDir = process.env.LOCALAPPDATA;
      if (appDataDir) {
        const userDataDir = path.join(appDataDir, app.name);
        const logDir = path.join(userDataDir, 'logs');
        // In Electron 16, the `appData` directory must be created explicitly or an error is
        // thrown when creating the singleton lock file.
        fs.mkdirSync(logDir, { recursive: true });
        app.setPath('appData', appDataDir);
        app.setPath('userData', userDataDir);
        app.setPath('logs', logDir);
      } else {
        throw new Error('Missing %LOCALAPPDATA% environment variable');
      }
    } else if (process.platform === 'linux') {
      const userDataDir = app.getPath('userData');
      const logDir = path.join(userDataDir, 'logs');
      fs.mkdirSync(logDir, { recursive: true });
      app.setPath('logs', logDir);
    }
  }

  private initLogging() {
    const mainLogPath = getMainLogPath();
    const rendererLogPath = getRendererLogPath();

    if (process.env.NODE_ENV === 'production') {
      this.rendererLog = new Logger();
      this.rendererLog.addInput(new IpcInput());

      try {
        createLoggingDirectory();
        cleanUpLogDirectory(OLD_LOG_FILES);

        backupLogFile(mainLogPath);
        backupLogFile(rendererLogPath);

        log.addOutput(new FileOutput(LogLevel.verbose, mainLogPath));
        this.rendererLog.addOutput(new FileOutput(LogLevel.verbose, rendererLogPath));
      } catch (e) {
        const error = e as Error;
        console.error('Failed to initialize logging:', error);
      }
    }

    log.addOutput(new ConsoleOutput(LogLevel.debug));
  }

  private onActivate = () => this.userInterface?.showWindow();

  // This is a last try to disconnect and quit gracefully if the app quits without having received
  // the before-quit event.
  private onQuit = () => {
    if (!this.quitInitiated) {
      this.prepareToQuit();
    }
  };

  private onBeforeQuit = (event: Electron.Event) => {
    // NOTE: keep this handler synchronous. Restarting the daemon here (the old
    // `await prepareRestart(true)` FDA branch) tore down the management socket
    // mid-quit, which made the gRPC client reconnect to the relaunching daemon
    // and left the GUI alive-but-disposed: an orphaned, unresponsive tray icon
    // that never quit. The daemon restart belongs to the explicit FDA flow
    // (renderer-triggered), not to every quit. Do NOT reintroduce async work or
    // a daemon restart in before-quit.
    log.info('before-quit received');
    if (this.quitInitiated) {
      event.preventDefault();
    } else {
      this.prepareToQuit();
    }
  };

  private prepareToQuit() {
    this.quitInitiated = true;
    log.info('Quit initiated');

    this.userInterface?.dispose();
    this.notificationController.dispose();

    // Unsubscribe the event handler
    try {
      if (this.daemonEventListener) {
        this.daemonRpc.unsubscribeDaemonEventListener(this.daemonEventListener);
        log.info('Unsubscribed from the daemon events');
      }
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to unsubscribe from daemon events: ${error.message}`);
    }

    if (this.daemonRpc.isConnected) {
      this.daemonRpc.disconnect();
    }

    this.settings.gui.changelogDisplayedForVersion = this.version.currentVersion.gui;
    for (const logger of [log, this.rendererLog]) {
      try {
        logger?.disposeDisposableOutputs();
      } catch (e) {
        const error = e as Error;
        log.error('Failed to dispose logger:', error);
      }
    }

    log.info('Disposable logging outputs disposed');
    log.info('Quit preparations finished');

    // Watchdog: if the normal Electron quit stalls (e.g. a daemon restart or a
    // lingering gRPC reconnect keeps the event loop alive), force the process to
    // exit so we never leave an orphaned, unresponsive tray icon behind. The
    // timer is unref'd so it never keeps an otherwise-clean quit alive.
    setTimeout(() => app.exit(0), 2000).unref();
  }

  private detectLocale(): string {
    const preferredLocale = this.settings.gui.preferredLocale;
    if (preferredLocale === SYSTEM_PREFERRED_LOCALE_KEY) {
      return app.getLocale();
    } else {
      return preferredLocale;
    }
  }

  private onReady = async () => {
    app.on('activate', this.onActivate);
    powerMonitor.on('suspend', this.onSuspend);
    powerMonitor.on('resume', this.onResume);

    // Windows/Linux cold start: a deep link that launched the app arrives as a
    // process argument. macOS delivers it through `open-url` instead.
    const initialDeepLink = findForumLoginArg(process.argv);
    if (initialDeepLink) {
      void this.handleForumLoginDeepLink(initialDeepLink);
    }

    // Disable built-in DNS resolver.
    app.configureHostResolver({
      enableBuiltInResolver: false,
      secureDnsMode: 'off',
      secureDnsServers: [],
    });

    // There's no option that prevents Electron from fetching spellcheck dictionaries from
    // Chromium's CDN and passing a non-resolving URL is the only known way to prevent it from
    // fetching.  https://github.com/electron/electron/issues/22995
    session.defaultSession.setSpellCheckerDictionaryDownloadURL('https://00.00/');

    // Blocks scripts in the renderer process from asking for any permission.
    this.blockPermissionRequests();
    // Blocks any http(s) and file requests that aren't supposed to happen.
    this.blockRequests();
    // Blocks navigation and window.open since it's not needed.
    this.blockNavigationAndWindowOpen();

    this.updateCurrentLocale();

    // Load split tunneling before connecting to the daemon to make sure that it is initialized
    // before first daemon connected event is handled.
    await this.loadSplitTunneling();

    this.connectToDaemon();

    if (process.platform === 'darwin') {
      await this.updateMacOsScrollbarVisibility();
      systemPreferences.subscribeNotification('AppleShowScrollBarsSettingChanged', async () => {
        await this.updateMacOsScrollbarVisibility();
      });

      await this.checkMacOsLaunchDaemon();
    }

    this.userInterface = new UserInterface(
      this,
      this.daemonRpc,
      SANDBOX_DISABLED,
      CommandLineOptions.disableResetNavigation.match,
    );

    this.tunnelStateExpectation = new Expectation(async () => {
      this.userInterface?.createTrayIconController(
        this.tunnelState.tunnelState,
        this.settings.gui.monochromaticIcon,
      );
      await this.userInterface?.updateTrayTheme();

      this.userInterface?.updateTray(this.account.isLoggedIn(), this.tunnelState.tunnelState);

      if (process.platform === 'win32') {
        nativeTheme.on('updated', async () => {
          if (this.settings.gui.monochromaticIcon) {
            await this.userInterface?.updateTrayTheme();
          }
        });
      }
    });

    this.registerIpcListeners();

    if (this.shouldShowWindowOnStart() || process.env.NODE_ENV === 'development') {
      this.userInterface.showWindow();
    }

    // For some reason playwright hangs on Linux if we call `window.setIcon`. Since the icon isn't
    // needed for the tests this block has been disabled when running e2e tests.
    if (process.platform === 'linux' && process.env.CI !== 'e2e') {
      try {
        const icon = await findIconPath('warren-vpn', ['png']);
        if (icon) {
          this.userInterface.setWindowIcon(icon);
        }
      } catch (e) {
        const error = e as Error;
        log.error('Failed to set window icon:', error.message);
      }
    }

    await this.userInterface.initializeWindow(
      this.account.isLoggedIn(),
      this.tunnelState.tunnelState,
    );
  };

  private loadSplitTunneling = async () => {
    // Only import split tunneling library on correct OS.
    if (process.platform === 'linux') {
      this.linuxSplitTunneling = await import('./linux-split-tunneling');
    } else if (process.platform === 'win32') {
      const { WindowsSplitTunnelingAppListRetriever } = await import('./windows-split-tunneling');
      this.splitTunneling = new WindowsSplitTunnelingAppListRetriever();
    } else if (process.platform === 'darwin') {
      const { MacOsSplitTunnelingAppListRetriever } = await import('./macos-split-tunneling');
      this.splitTunneling = new MacOsSplitTunnelingAppListRetriever();
    }
  };

  // The app-list retriever only exists on macOS/Windows. Linux uses the
  // separate `linuxSplitTunneling` module and never calls the shared
  // `splitTunneling.*` channels, so reaching this on Linux is a bug:
  // fail loudly instead of dereferencing undefined.
  private get splitTunnelingApi(): ISplitTunnelingAppListRetriever {
    if (!this.splitTunneling) {
      throw new Error('Split tunneling app-list API is unavailable on this platform');
    }
    return this.splitTunneling;
  }

  private onSuspend = () => {
    log.info('Suspend event received, disconnecting from daemon');
    if (this.daemonEventListener) {
      this.daemonRpc.unsubscribeDaemonEventListener(this.daemonEventListener);
    }

    const wasConnected = this.daemonRpc.isConnected;
    IpcMainEventChannel.navigation.notifyReset?.();
    this.daemonRpc.disconnect();
    this.onDaemonDisconnected(wasConnected, undefined, true);
  };

  private onResume = () => {
    log.info('Resume event received, connecting to daemon');
    this.daemonRpc.reopen(
      new ConnectionObserver(this.onDaemonConnected, this.onDaemonDisconnected),
    );
    this.connectToDaemon();
  };

  private onDaemonConnected = async () => {
    const firstDaemonConnection = this.beforeFirstDaemonConnection;
    this.beforeFirstDaemonConnection = false;

    log.info('Connected to the daemon');

    this.notificationController.closeNotificationsInCategory(
      SystemNotificationCategory.tunnelState,
    );

    // subscribe to events
    try {
      this.daemonEventListener = this.subscribeEvents();
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to subscribe: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // subscribe to app upgrade events
    try {
      this.daemonAppUpgradeEventListener = this.appUpgrade.subscribeEvents();
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to subscribe to app upgrade events: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // Subscribe to Warren live status. The stream is best-effort; a
    // failing subscribe (older daemon without the rpc, network blip,
    // etc.) must not block the rest of the bootstrap, so we only log
    // and continue.
    try {
      this.warrenStatusListener = this.subscribeWarrenStatusEvents();
    } catch (e) {
      const error = e as Error;
      log.warn(`Failed to subscribe to Warren status events: ${error.message}`);
    }

    // Subscribe to NAT-PMP refresh-loop status. Same best-effort
    // policy as warrenStatus: failure must not block the bootstrap.
    try {
      this.natPmpStatusListener = this.subscribeNatPmpStatusEvents();
    } catch (e) {
      const error = e as Error;
      log.warn(`Failed to subscribe to NAT-PMP status events: ${error.message}`);
    }

    if (firstDaemonConnection) {
      // check if daemon is performing post upgrade tasks the first time it's connected to
      try {
        await this.performPostUpgradeCheck();
      } catch (e) {
        const error = e as Error;
        log.error(`Failed to check if daemon is performing post upgrade tasks: ${error.message}`);

        return this.handleBootstrapError(error);
      }
    }

    // fetch account history
    try {
      this.account.setAccountHistory(await this.daemonRpc.getAccountHistory());
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch the account history: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch split tunneling support status
    try {
      if (process.platform === 'linux' || process.platform === 'win32') {
        this.splitTunnelingSupported = await this.daemonRpc.splitTunnelIsSupported();
      } else if (process.platform === 'darwin') {
        // macOS split tunneling relies on Endpoint Security, which only
        // works on a SIGNED build (Developer ID + endpoint-security
        // entitlement + Full Disk Access). On unsigned builds the daemon
        // refuses to enable it (Error::MacosSplitTunnelUnsupported, gated
        // by the `macos-split-tunnel` cargo feature), so report it
        // unsupported here to hide the feature rather than let the user
        // toggle it for nothing. Flip to `true` (or wire the daemon
        // feature through an RPC) once the app ships signed.
        this.splitTunnelingSupported = false;
      } else {
        this.splitTunnelingSupported = true;
      }
      IpcMainEventChannel.splitTunneling.notifyIsSupported?.(this.splitTunnelingSupported);
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch if split tunneling is supported: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch the tunnel state
    try {
      this.handleNewTunnelState(await this.daemonRpc.getState());
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch the tunnel state: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch device
    try {
      const deviceState = await this.daemonRpc.getDevice();
      this.account.handleDeviceEvent({ type: deviceState.type, deviceState } as DeviceEvent);
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch device: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch settings
    try {
      this.setSettings(await this.daemonRpc.getSettings());
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch settings: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch current api access method
    try {
      this.currentApiAccessMethod = await this.daemonRpc.getCurrentApiAccessMethod();
      IpcMainEventChannel.settings.notifyApiAccessMethodSettingChange?.(
        this.currentApiAccessMethod,
      );
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch settings: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    if (this.tunnelStateExpectation) {
      this.tunnelStateExpectation.fulfill();
    }

    // fetch relays
    try {
      this.setRelayList(await this.daemonRpc.getRelayLocations());
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch relay locations: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch the daemon's version
    try {
      this.version.setDaemonVersion(await this.daemonRpc.getCurrentVersion());
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch the daemon's version: ${error.message}`);

      return this.handleBootstrapError(error);
    }

    // fetch the latest version info in background
    if (!UPDATE_NOTIFICATION_DISABLED) {
      void this.version.fetchLatestVersion();
    }

    // reset the reconnect backoff when connection established.
    this.reconnectBackoff.reset();

    // notify renderer, this.daemonRpc.isConnected could have changed if the daemon disconnected
    // again before this if-statement is reached.
    if (this.daemonRpc.isConnected) {
      IpcMainEventChannel.daemon.notifyConnected?.();
    }

    if (firstDaemonConnection) {
      void this.autoConnect();
    }

    // show window when account is not set
    if (!this.account.isLoggedIn()) {
      this.userInterface?.showWindow();
    }
  };

  private onDaemonDisconnected = (wasConnected: boolean, error?: Error, planned?: boolean) => {
    if (this.daemonEventListener) {
      this.daemonRpc.unsubscribeDaemonEventListener(this.daemonEventListener);
    }
    if (this.daemonAppUpgradeEventListener) {
      this.daemonRpc.unsubscribeAppUpgradeEventListener(this.daemonAppUpgradeEventListener);
    }
    if (this.warrenStatusListener) {
      this.daemonRpc.unsubscribeWarrenStatusListener(this.warrenStatusListener);
    }
    if (this.natPmpStatusListener) {
      this.daemonRpc.unsubscribeNatPmpStatusListener(this.natPmpStatusListener);
    }
    // Reset all listeners because they are going to be invalidated
    // on disconnect.
    this.daemonEventListener = undefined;
    this.daemonAppUpgradeEventListener = undefined;
    this.warrenStatusListener = undefined;
    this.natPmpStatusListener = undefined;

    this.notificationController.closeNotificationsInCategory(
      SystemNotificationCategory.tunnelState,
    );

    if (this.tunnelState.tunnelState.state !== 'disconnected' && !planned) {
      this.notificationController.notifyDaemonDisconnected(
        this.userInterface?.isWindowVisible() ?? false,
        this.settings.gui.enableSystemNotifications,
      );
    }

    this.tunnelState.resetFallback();

    if (wasConnected) {
      // update the tray icon to indicate that the computer is not secure anymore
      this.userInterface?.updateTray(false, {
        state: 'disconnected',
        lockedDown: this.settings.lockdownMode,
      });

      // notify renderer process
      IpcMainEventChannel.daemon.notifyDisconnected?.();
    }

    // recover connection on error
    if (error) {
      if (wasConnected) {
        log.error(`Lost connection to daemon: ${error.message}`);
      } else {
        log.error(`Failed to connect to daemon: ${error.message}`);
      }
    } else {
      log.info('Disconnected from the daemon');
    }
    if (process.platform === 'darwin') {
      void this.checkMacOsLaunchDaemon();
    }
  };

  private connectToDaemon() {
    void this.daemonRpc
      .connect()
      .catch((error) => log.error(`Unable to connect to daemon: ${error.message}`));
  }

  private handleBootstrapError(_error?: Error) {
    // Unsubscribe from daemon, app upgrade, and Warren status events
    // when encountering errors during initial data retrieval.
    if (this.daemonEventListener) {
      this.daemonRpc.unsubscribeDaemonEventListener(this.daemonEventListener);
    }

    if (this.daemonAppUpgradeEventListener) {
      this.daemonRpc.unsubscribeAppUpgradeEventListener(this.daemonAppUpgradeEventListener);
    }

    if (this.warrenStatusListener) {
      this.daemonRpc.unsubscribeWarrenStatusListener(this.warrenStatusListener);
    }

    if (this.natPmpStatusListener) {
      this.daemonRpc.unsubscribeNatPmpStatusListener(this.natPmpStatusListener);
    }
  }

  private subscribeEvents(): SubscriptionListener<DaemonEvent> {
    const daemonEventListener = new SubscriptionListener(
      (daemonEvent: DaemonEvent) => {
        if ('tunnelState' in daemonEvent) {
          this.handleNewTunnelState(daemonEvent.tunnelState);
        } else if ('settings' in daemonEvent) {
          this.setSettings(daemonEvent.settings);
        } else if ('relayList' in daemonEvent) {
          IpcMainEventChannel.relays.notify?.(daemonEvent.relayList);
        } else if ('appVersionInfo' in daemonEvent) {
          this.version.setLatestVersion(daemonEvent.appVersionInfo);
        } else if ('device' in daemonEvent) {
          this.account.handleDeviceEvent(daemonEvent.device);
        } else if ('accessMethodSetting' in daemonEvent) {
          IpcMainEventChannel.settings.notifyApiAccessMethodSettingChange?.(
            daemonEvent.accessMethodSetting,
          );
        }
      },
      (error: Error) => {
        log.error(`Cannot deserialize the daemon event: ${error.message}`);
      },
    );

    this.daemonRpc.subscribeDaemonEventListener(daemonEventListener);

    return daemonEventListener;
  }

  // Forwards every WarrenStatus snapshot received from the daemon to
  // the renderer over the `warrenStatus` IPC channel. Errors are
  // logged but do not bring down the rest of the bootstrap because
  // the status stream is purely informational.
  private subscribeWarrenStatusEvents(): SubscriptionListener<WarrenStatus> {
    const listener = new SubscriptionListener(
      (snapshot: WarrenStatus) => {
        IpcMainEventChannel.warrenStatus.notify?.(snapshot);
      },
      (error: Error) => {
        log.warn(`Cannot deserialize the Warren status event: ${error.message}`);
      },
    );
    this.daemonRpc.subscribeWarrenStatusListener(listener);
    return listener;
  }

  // Forwards every NatPmpStatus snapshot received from the daemon to
  // the renderer over the `natPmpStatus` IPC channel. Same best-effort
  // semantics as subscribeWarrenStatusEvents.
  private subscribeNatPmpStatusEvents(): SubscriptionListener<NatPmpStatus> {
    const listener = new SubscriptionListener(
      (snapshot: NatPmpStatus) => {
        IpcMainEventChannel.natPmpStatus.notify?.(snapshot);
      },
      (error: Error) => {
        log.warn(`Cannot deserialize the NAT-PMP status event: ${error.message}`);
      },
    );
    this.daemonRpc.subscribeNatPmpStatusListener(listener);
    return listener;
  }

  private setSettings(newSettings: ISettings) {
    const oldSettings = this.settings;
    this.settings.handleNewSettings(newSettings);

    this.userInterface?.updateTray(this.account.isLoggedIn(), this.tunnelState.tunnelState);

    if (oldSettings.showBetaReleases !== newSettings.showBetaReleases) {
      this.version.setLatestVersion(this.version.upgradeVersion);
    }

    IpcMainEventChannel.settings.notify?.(newSettings);

    void this.updateSplitTunnelingApplications(newSettings.splitTunnel.appsList);
  }

  private handleNewTunnelState(newState: TunnelState) {
    this.tunnelState.handleNewTunnelState(newState);
  }

  private setRelayList(relayList: IRelayListWithEndpointData) {
    this.relayList = relayList;
    IpcMainEventChannel.relays.notify?.(relayList);
  }

  private async updateSplitTunnelingApplications(appList: string[]): Promise<void> {
    if (this.splitTunneling) {
      const { applications } = await this.splitTunneling.getMetadataForApplications(appList);
      this.splitTunnelingApplications = applications;

      IpcMainEventChannel.splitTunneling.notify?.(applications);
    }
  }

  private registerIpcListeners() {
    IpcMainEventChannel.state.handleGet(() => ({
      isConnected: this.daemonRpc.isConnected,
      autoStart: getOpenAtLogin(),
      accountData: this.account.accountData,
      accountHistory: this.account.accountHistory,
      tunnelState: this.tunnelState.tunnelState,
      settings: this.settings.all,
      isPerformingPostUpgrade: this.isPerformingPostUpgrade,
      daemonAllowed: this.daemonAllowed,
      deviceState: this.account.deviceState,
      relayList: this.relayList,
      currentVersion: this.version.currentVersion,
      upgradeVersion: this.version.upgradeVersion,
      guiSettings: this.settings.gui.state,
      translations: this.translations,
      splitTunnelingApplications: this.splitTunnelingApplications,
      splitTunnelingSupported: this.splitTunnelingSupported,
      macOsScrollbarVisibility: this.macOsScrollbarVisibility,
      changelog: this.changelog ?? [],
      navigationHistory: this.navigationHistory,
      currentApiAccessMethod: this.currentApiAccessMethod,
      isMacOs13OrNewer: isMacOs13OrNewer(),
    }));

    IpcMainEventChannel.map.handleGetData(async () => {
      const readGeoFile = async (fileName: string) => {
        const data = await fs.promises.readFile(path.join(GEO_DIR, fileName));

        return new Uint8Array(data).buffer;
      };

      return {
        landContourIndices: await readGeoFile('land_contour_indices.gl'),
        landPositions: await readGeoFile('land_positions.gl'),
        landTriangleIndices: await readGeoFile('land_triangle_indices.gl'),
        oceanIndices: await readGeoFile('ocean_indices.gl'),
        oceanPositions: await readGeoFile('ocean_positions.gl'),
      };
    });

    IpcMainEventChannel.tunnel.handleConnect(this.connectTunnel);
    IpcMainEventChannel.tunnel.handleReconnect(this.reconnectTunnel);
    IpcMainEventChannel.tunnel.handleDisconnect((source) => this.disconnectTunnel(source));

    IpcMainEventChannel.guiSettings.handleSetPreferredLocale((locale: string) => {
      this.settings.gui.preferredLocale = locale;
      this.updateCurrentLocale();
      return Promise.resolve(this.translations);
    });

    IpcMainEventChannel.linuxSplitTunneling.handleGetApplications(() => {
      return this.linuxSplitTunneling!.getApplications(this.locale);
    });
    IpcMainEventChannel.splitTunneling.handleGetApplications((updateCaches: boolean) => {
      return this.splitTunnelingApi.getApplications(updateCaches);
    });
    IpcMainEventChannel.linuxSplitTunneling.handleLaunchApplication((application) => {
      return this.linuxSplitTunneling!.launchApplication(application);
    });

    IpcMainEventChannel.splitTunneling.handleSetState((enabled) => {
      return this.daemonRpc.setSplitTunnelingState(enabled);
    });
    IpcMainEventChannel.splitTunneling.handleAddApplication(async (application) => {
      // If the applications is a string (path) it's an application picked with the file picker
      // that we want to add to the list of additional applications.
      if (typeof application === 'string') {
        let executablePath;
        try {
          executablePath = await this.splitTunnelingApi.resolveExecutablePath(application);
        } catch {
          return;
        }
        this.settings.gui.addBrowsedForSplitTunnelingApplications(executablePath);
        await this.splitTunnelingApi.addApplicationPathToCache(application);
        await this.daemonRpc.addSplitTunnelingApplication(executablePath);
      } else {
        await this.daemonRpc.addSplitTunnelingApplication(application.absolutepath);
      }
    });
    IpcMainEventChannel.splitTunneling.handleRemoveApplication((application) => {
      return this.daemonRpc.removeSplitTunnelingApplication(
        typeof application === 'string' ? application : application.absolutepath,
      );
    });
    IpcMainEventChannel.splitTunneling.handleForgetManuallyAddedApplication((application) => {
      this.settings.gui.deleteBrowsedForSplitTunnelingApplications(application.absolutepath);
      this.splitTunnelingApi.removeApplicationFromCache(application);
      return Promise.resolve();
    });
    IpcMainEventChannel.splitTunneling.handleGetSupported(() => {
      return this.daemonRpc.splitTunnelIsSupported();
    });
    IpcMainEventChannel.macOsSplitTunneling.handleNeedFullDiskPermissions(() => {
      return this.daemonRpc.needFullDiskPermissions();
    });

    IpcMainEventChannel.app.handleQuit((source: DisconnectSource) =>
      this.disconnectAndQuit(source),
    );
    IpcMainEventChannel.app.handleOpenUrl(async (url) => {
      if (Object.values(urls).find((allowedUrl) => url.startsWith(allowedUrl))) {
        await shell.openExternal(url);
      }
    });
    IpcMainEventChannel.app.handleGetPathBaseName((filePath) =>
      Promise.resolve(path.basename(filePath)),
    );
    IpcMainEventChannel.app.handleClearMnemonicFromClipboard((mnemonic) => {
      // Only wipe the clipboard if it still holds our mnemonic, so we
      // never clobber something the user copied in between.
      if (clipboard.readText() === mnemonic) {
        clipboard.clear();
        return Promise.resolve(true);
      }
      return Promise.resolve(false);
    });

    IpcMainEventChannel.navigation.handleSetHistory((history) => {
      this.navigationHistory = history;
    });

    // Forum login: only an explicit user approval signs with the wallet key.
    IpcMainEventChannel.forumLogin.handleApprove((request) =>
      approveForumLogin(request, this.daemonRpc),
    );
    IpcMainEventChannel.forumLogin.handleCancel((request) => cancelForumLogin(request));

    IpcMainEventChannel.customLists.handleCreateCustomList((name) => {
      return this.daemonRpc.createCustomList(name);
    });
    IpcMainEventChannel.customLists.handleDeleteCustomList((id) => {
      return this.daemonRpc.deleteCustomList(id);
    });
    IpcMainEventChannel.customLists.handleUpdateCustomList((customList) => {
      return this.daemonRpc.updateCustomList(customList);
    });

    IpcMainEventChannel.daemon.handlePrepareRestart((shutdown) => {
      return this.daemonRpc.prepareRestart(shutdown);
    });

    problemReport.registerIpcListeners();
    this.userInterface!.registerIpcListeners();
    this.settings.registerIpcListeners();
    this.account.registerIpcListeners();
    this.appUpgrade.registerIpcListeners();

    if (this.splitTunneling) {
      this.settings.gui.browsedForSplitTunnelingApplications.forEach((application) => {
        void this.splitTunneling!.addApplicationPathToCache(application);
      });
    }
  }

  private async autoConnect() {
    if (process.env.NODE_ENV === 'development') {
      log.info('Skip autoconnect in development');
    } else if (
      this.account.isLoggedIn() &&
      (!this.account.accountData || !hasExpired(this.account.accountData.expiry))
    ) {
      if (this.settings.gui.autoConnect) {
        try {
          log.info('Autoconnect the tunnel');

          await this.daemonRpc.connectTunnel();
        } catch (e) {
          const error = e as Error;
          log.error(`Failed to autoconnect the tunnel: ${error.message}`);
        }
      } else {
        log.info('Skip autoconnect because GUI setting is disabled');
      }
    } else {
      log.info('Skip autoconnect because public key is not set');
    }
  }

  private updateCurrentLocale() {
    this.locale = this.detectLocale();

    log.info(`Detected locale: ${this.locale}`);

    const messagesTranslations = loadTranslations(this.locale, messages);
    const relayLocationsTranslations = loadTranslations(this.locale, relayLocations);

    this.translations = {
      locale: this.locale,
      messages: messagesTranslations,
      relayLocations: relayLocationsTranslations,
    };

    this.userInterface?.updateTray(this.account.isLoggedIn(), this.tunnelState.tunnelState);
  }

  private blockPermissionRequests() {
    session.defaultSession.setPermissionRequestHandler((_webContents, permission, callback) => {
      callback(ALLOWED_PERMISSIONS.includes(permission));
    });
    session.defaultSession.setPermissionCheckHandler((_webContents, permission) =>
      ALLOWED_PERMISSIONS.includes(permission),
    );
  }

  // Since the app frontend never performs any network requests, all requests originating from the
  // renderer process are blocked to protect against the potential threat of malicious third party
  // dependencies. There are a few exceptions which are described further down.
  private blockRequests() {
    session.defaultSession.webRequest.onBeforeRequest((details, callback) => {
      if (this.allowFileAccess(details.url) || this.allowDevelopmentRequest(details.url)) {
        callback({});
      } else {
        log.error(`${details.method} request blocked: ${details.url}`);
        callback({ cancel: true });

        // Throw error in development to notify since this should never happen.
        if (process.env.NODE_ENV === 'development') {
          throw new Error('Web request blocked');
        }
      }
    });
  }

  private allowFileAccess(url: string): boolean {
    const buildDir = path.normalize(path.join(path.resolve(import.meta.dirname), '..', '..'));

    if (url.startsWith('file:')) {
      // Extract the path from the URL
      let filePath = decodeURI(new URL(url).pathname);
      if (process.platform === 'win32') {
        // Windows paths shouldn't start with a '/'
        filePath = filePath.replace(/^\//, '');
      }
      filePath = path.resolve(filePath);

      return !path.relative(buildDir, filePath).includes('..');
    } else {
      return false;
    }
  }

  private allowDevelopmentRequest(url: string): boolean {
    if (process.env.NODE_ENV === 'development') {
      const isViteDevServerRequest = (url: string): boolean => {
        if (process.env.VITE_DEV_SERVER_URL) {
          const viteDevServerUrl = new URL(process.env.VITE_DEV_SERVER_URL);
          const viteDevServerUrlWs = new URL(viteDevServerUrl);
          viteDevServerUrlWs.protocol = 'ws';

          return url.startsWith(viteDevServerUrl.href) || url.startsWith(viteDevServerUrlWs.href);
        }

        return false;
      };

      const isDevtoolsRequest = (url: string): boolean => {
        // Downloading of React and Redux developer tools.
        const devtoolsUrls = [
          'devtools://',
          'chrome-extension://',
          'https://clients2.google.com',
          'https://clients2.googleusercontent.com',
        ];

        return devtoolsUrls.some((devtoolsUrl) => url.startsWith(devtoolsUrl));
      };

      return isViteDevServerRequest(url) || isDevtoolsRequest(url);
    }

    return false;
  }

  // Blocks navigation and window.open since it's not needed.
  private blockNavigationAndWindowOpen() {
    app.on('web-contents-created', (_event, contents) => {
      contents.on('will-navigate', (event) => event.preventDefault());
      contents.setWindowOpenHandler(() => ({ action: 'deny' }));
    });
  }

  private shouldShowWindowOnStart(): boolean {
    return this.settings.gui.unpinnedWindow && !this.settings.gui.startMinimized;
  }

  private checkMacOsLaunchDaemon(): Promise<void> {
    const daemonBin = resolveBin('warren-daemon');
    const args = ['--launch-daemon-status'];
    return new Promise((resolve, _reject) => {
      execFile(daemonBin, args, { windowsHide: true }, (error, stdout, stderr) => {
        if (error) {
          if (error.code === 2) {
            IpcMainEventChannel.daemon.notifyDaemonAllowed?.(false);
            this.daemonAllowed = false;
          } else {
            log.error(
              `Error while checking launch daemon authorization status.
                Status: ${error.code}
                Stdout: ${stdout.toString()}
                Stderr: ${stderr.toString()}`,
            );
          }
        } else {
          IpcMainEventChannel.daemon.notifyDaemonAllowed?.(true);
          this.daemonAllowed = true;
        }
        resolve();
      });
    });
  }

  private async updateMacOsScrollbarVisibility(): Promise<void> {
    const command =
      'defaults read kCFPreferencesAnyApplication AppleShowScrollBars || echo Automatic';
    const { stdout } = await execAsync(command);
    switch (stdout.trim()) {
      case 'WhenScrolling':
        this.macOsScrollbarVisibility = MacOsScrollbarVisibility.whenScrolling;
        break;
      case 'Always':
        this.macOsScrollbarVisibility = MacOsScrollbarVisibility.always;
        break;
      case 'Automatic':
      default:
        this.macOsScrollbarVisibility = MacOsScrollbarVisibility.automatic;
        break;
    }

    IpcMainEventChannel.window.notifyMacOsScrollbarVisibility?.(this.macOsScrollbarVisibility);
  }

  /* eslint-disable @typescript-eslint/member-ordering */
  // NotificationControllerDelagate
  public openApp = () => this.userInterface?.showWindow();
  public openLink = async (url: string) => {
    return shell.openExternal(url);
  };
  public openRoute = (route: RoutePath) => {
    void IpcMainEventChannel.app.notifyOpenRoute?.(route);
  };
  public showNotificationIcon = (value: boolean, reason?: string) =>
    this.userInterface?.showNotificationIcon(value, reason);

  // NotificationSender
  public notify = (notification: SystemNotification) => {
    this.notificationController.notify(
      notification,
      this.userInterface?.isWindowVisible() ?? false,
      this.settings.gui.enableSystemNotifications,
    );
  };
  public closeNotificationsInCategory = (category: SystemNotificationCategory) =>
    this.notificationController.closeNotificationsInCategory(category);

  // UserInterfaceDelegate
  public dismissActiveNotifications = () =>
    this.notificationController.dismissActiveNotifications();
  public isUnpinnedWindow = () => this.settings.gui.unpinnedWindow;
  public updateAccountData = () => this.account.updateAccountData();
  public getAccountData = () => this.account.accountData;
  public getVersionInfo = () => this.version.fetchLatestVersion();

  // TunnelStateHandlerDelegate
  public handleTunnelStateUpdate = (tunnelState: TunnelState) => {
    this.userInterface?.updateTray(this.account.isLoggedIn(), tunnelState);

    this.notificationController.notifyTunnelState(
      tunnelState,
      this.settings.splitTunnel.enableExclusions && this.settings.splitTunnel.appsList.length > 0,
      this.userInterface?.isWindowVisible() ?? false,
      this.settings.gui.enableSystemNotifications,
      this.splitTunnelingSupported,
    );

    IpcMainEventChannel.tunnel.notify?.(tunnelState);

    if (this.account.accountData) {
      this.account.detectStaleAccountExpiry(tunnelState);
    }
  };

  // SettingsDelegate
  public handleMonochromaticIconChange = (value: boolean) =>
    this.userInterface?.setMonochromaticIcon(value) ?? Promise.resolve();
  public handleUnpinnedWindowChange = () =>
    void this.userInterface?.recreateWindow(
      this.account.isLoggedIn(),
      this.tunnelState.tunnelState,
    );

  // AccountDelegate
  public getLocale = () => this.locale;
  public getTunnelState = () => this.tunnelState.tunnelState;
  public onDeviceEvent = () => {
    this.userInterface?.updateTray(this.account.isLoggedIn(), this.tunnelState.tunnelState);

    if (this.isPerformingPostUpgrade) {
      void this.performPostUpgradeCheck();
    }
  };
  /* eslint-enable @typescript-eslint/member-ordering */
}

if (CommandLineOptions.help.match) {
  console.log('Warren VPN');
  console.log('Graphical interface for managing the Warren VPN daemon');

  console.log('');
  console.log('OPTIONS:');
  printCommandLineOptions();

  console.log('');
  console.log('USEFUL ELECTRON/CHROMIUM OPTIONS:');
  printElectronOptions();

  process.exit(0);
} else if (CommandLineOptions.version.match) {
  console.log(GUI_VERSION);
  console.log('Electron version:', process.versions.electron);

  process.exit(0);
} else {
  const applicationMain = new ApplicationMain();
  applicationMain.run();
}
