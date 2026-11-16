import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';
import { RoutePath } from '../../../shared/routes';
import { getDownloadUrl } from '../../../shared/version';

interface AppUpgradeAvailableNotificationContext {
  suggestedUpgradeVersion?: string;
  suggestedIsBeta?: boolean;
  updateDismissedForVersion?: string;
  platform: NodeJS.Platform;
  close: () => void;
}

export class AppUpgradeAvailableNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: AppUpgradeAvailableNotificationContext) {}

  public mayDisplay(): boolean {
    const { suggestedUpgradeVersion, suggestedIsBeta, updateDismissedForVersion } = this.context;
    if (!suggestedUpgradeVersion) {
      return false;
    }
    if (suggestedIsBeta && suggestedUpgradeVersion === updateDismissedForVersion) {
      return false;
    }
    return true;
  }

  public getInAppNotification(): InAppNotification {
    const { close, platform, suggestedIsBeta, suggestedUpgradeVersion } = this.context;
    const isLinux = platform === 'linux';

    return {
      indicator: 'warning',
      icon: 'update',
      title: messages.pgettext('app-info-view', 'Update available'),
      subtitle: sprintf(messages.pgettext('app-upgrade-view', 'Version %(version)s'), {
        version: suggestedUpgradeVersion,
      }),
      button: {
        // TRANSLATORS: Button of the home's update banner, which installs the new version.
        label: messages.pgettext('in-app-notifications', 'Install'),
        action: isLinux
          ? {
              type: 'navigate-external',
              link: {
                to: getDownloadUrl(suggestedIsBeta ?? false),
                'aria-label':
                  // TRANSLATORS: Accessibility label for link to go to download page.
                  messages.pgettext(
                    'accessibility',
                    'New version available, click here to go to download page, opens externally',
                  ),
              },
            }
          : {
              type: 'navigate-internal',
              link: {
                to: RoutePath.appUpgrade,
                // TRANSLATORS: Accessibility label for link to go to upgrade view.
                'aria-label': messages.pgettext(
                  'accessibility',
                  'New version available, click here to go to update view',
                ),
              },
            },
      },
      // A beta is an offer: it can be put away until the next one.
      action: suggestedIsBeta ? { type: 'close', close } : undefined,
    };
  }
}
