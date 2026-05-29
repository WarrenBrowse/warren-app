import { app } from 'electron';

import { IAppVersionInfo } from '../shared/daemon-rpc-types';
import { ICurrentAppVersionInfo } from '../shared/ipc-types';
import log from '../shared/logging';
import {
  InconsistentVersionNotificationProvider,
  SystemNotificationCategory,
  UnsupportedVersionNotificationProvider,
  UpdateAvailableNotificationProvider,
} from '../shared/notifications';
import { DaemonRpc } from './daemon-rpc';
import { IpcMainEventChannel } from './ipc-event-channel';
import { NotificationSender } from './notification-controller';

// The product version is injected at build time by Vite's `define` (see
// vite.config.ts) from the `mullvad-version` binary — the single source of
// truth shared with the daemon (dist-assets/desktop-product-version.txt). Using
// it verbatim guarantees the GUI version equals what the daemon reports in every
// build mode: `1.0.0` on a release tag, `1.0.0-dev-<hash>` in development. This
// avoids both the spurious "APP IS OUT OF SYNC" notification and the stale
// `0.0.0` that `app.getVersion()` returns from package.json during development.
// We must NOT transform the string (e.g. strip a trailing `.0`): the daemon
// reports the same string verbatim, so any rewrite would break the equality
// check in `setDaemonVersion`.
//
// `WARREN_GUI_VERSION` is replaced by a string literal at build time. In
// environments without the define (e.g. unit tests run outside Vite) the
// identifier is undefined, so we fall back to `app.getVersion()`.
declare const WARREN_GUI_VERSION: string | undefined;
export const GUI_VERSION =
  typeof WARREN_GUI_VERSION === 'string' ? WARREN_GUI_VERSION : app.getVersion();
// Mirrors the daemon's beta detection, which is a plain substring check
// (`mullvad_version::VERSION.contains("beta")` in mullvad-daemon/src/version/mod.rs).
// Warren uses semver, so betas look like `1.0.0-beta1`; the legacy upstream regex
// `/^(\d{4})\.(\d+)-beta(\d+)$/` required a 4-digit year and no patch component,
// so it never matched a Warren version and always reported `isBeta = false`.
const isBetaVersion = (version: string): boolean => version.includes('beta');

export default class Version {
  private currentVersionData: ICurrentAppVersionInfo = {
    daemon: undefined,
    gui: GUI_VERSION,
    isConsistent: true,
    isBeta: isBetaVersion(GUI_VERSION),
  };

  private upgradeVersionData: IAppVersionInfo = {
    supported: true,
    suggestedUpgrade: undefined,
  };

  public constructor(
    private delegate: NotificationSender,
    private daemonRpc: DaemonRpc,
    private updateNotificationDisabled: boolean,
  ) {}

  public get currentVersion() {
    return this.currentVersionData;
  }

  public get upgradeVersion() {
    return this.upgradeVersionData;
  }

  public setDaemonVersion(daemonVersion: string) {
    const versionInfo = {
      ...this.currentVersionData,
      daemon: daemonVersion,
      isConsistent: daemonVersion === this.currentVersionData.gui,
    };

    this.currentVersionData = versionInfo;

    if (!versionInfo.isConsistent) {
      log.info('Inconsistent version', {
        guiVersion: versionInfo.gui,
        daemonVersion: versionInfo.daemon,
      });
    }

    // notify user about inconsistent version
    const notificationProvider = new InconsistentVersionNotificationProvider({
      consistent: versionInfo.isConsistent,
    });
    if (notificationProvider.mayDisplay()) {
      this.delegate.notify(notificationProvider.getSystemNotification());
    } else {
      this.delegate.closeNotificationsInCategory(SystemNotificationCategory.inconsistentVersion);
    }

    // notify renderer
    IpcMainEventChannel.currentVersion.notify?.(versionInfo);
  }

  public setLatestVersion(latestVersionInfo: IAppVersionInfo) {
    if (this.updateNotificationDisabled) {
      return;
    }

    const suggestedIsBeta =
      latestVersionInfo.suggestedUpgrade !== undefined &&
      isBetaVersion(latestVersionInfo.suggestedUpgrade.version);

    const upgradeVersion = {
      ...latestVersionInfo,
      suggestedIsBeta,
    };

    this.upgradeVersionData = upgradeVersion;

    // notify user to update the app if it became unsupported
    const notificationProviders = [
      new UnsupportedVersionNotificationProvider({
        supported: latestVersionInfo.supported,
        consistent: this.currentVersionData.isConsistent,
        suggestedUpgrade: latestVersionInfo.suggestedUpgrade,
        suggestedIsBeta,
      }),
      new UpdateAvailableNotificationProvider({
        suggestedUpgrade: latestVersionInfo.suggestedUpgrade,
        suggestedIsBeta,
      }),
    ];
    const notificationProvider = notificationProviders.find((notificationProvider) =>
      notificationProvider.mayDisplay(),
    );
    if (notificationProvider) {
      this.delegate.notify(notificationProvider.getSystemNotification());
    } else {
      this.delegate.closeNotificationsInCategory(SystemNotificationCategory.newVersion);
    }

    IpcMainEventChannel.upgradeVersion.notify?.(upgradeVersion);
  }

  public async fetchLatestVersion() {
    try {
      this.setLatestVersion(await this.daemonRpc.getVersionInfo());
    } catch (e) {
      const error = e as Error;
      // In Warren mode the Mullvad version router is permanently
      // disabled (Warren ships its own GitHub Releases pipeline),
      // so every call to `getVersionInfo` rejects with "Version
      // router is down". Polling this from the GUI is intrinsic
      // to the upstream design, so demote the expected case to
      // debug; any other failure (transient API outage, malformed
      // response) still logs at error.
      if (error.message.includes('Version router is down')) {
        log.debug('Version check skipped (router closed in Warren mode)');
      } else {
        log.error(`Failed to request the version info: ${error.message}`);
      }
    }
  }
}
