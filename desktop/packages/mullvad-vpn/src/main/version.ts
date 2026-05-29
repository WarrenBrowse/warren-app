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

// Warren uses full semver (e.g. `1.0.0`) and the daemon reports the same string,
// so the GUI version must match it verbatim. Upstream Mullvad stripped a trailing
// `.0` here because its date-based versions (`2024.5.0`) are reported as `2024.5`
// by the daemon. Applying that strip to semver breaks the consistency check:
// `String.replace('.0', '')` only replaces the FIRST match, turning `1.0.0` into
// `1.0`, which never equals the daemon's `1.0.0` and triggers a permanent
// "APP IS OUT OF SYNC" notification.
export const GUI_VERSION = app.getVersion();
/// Mirrors the beta check regex in the daemon. Matches only well formed beta versions
const IS_BETA = /^(\d{4})\.(\d+)-beta(\d+)$/;

export default class Version {
  private currentVersionData: ICurrentAppVersionInfo = {
    daemon: undefined,
    gui: GUI_VERSION,
    isConsistent: true,
    isBeta: IS_BETA.test(GUI_VERSION),
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
      IS_BETA.test(latestVersionInfo.suggestedUpgrade.version);

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
        log.debug(`Version check skipped (router closed in Warren mode)`);
      } else {
        log.error(`Failed to request the version info: ${error.message}`);
      }
    }
  }
}
