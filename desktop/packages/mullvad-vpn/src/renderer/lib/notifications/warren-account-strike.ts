import {
  latestStrike,
  strikeCaseReference,
  strikeDismissalKey,
  strikeWarning,
} from '../../../shared/account-standing';
import { urls } from '../../../shared/constants';
import { WarrenAccountStanding } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenAccountStrikeNotificationContext {
  // The wallet's port-forward standing as the daemon last knew it, `null`
  // while unknown.
  accountStanding: WarrenAccountStanding | null;
  // Digests of the warnings this user has put away, from the GUI settings.
  dismissedKeys: string[];
  dismiss: (key: string) => void;
  locale: string;
}

// A forwarded port was closed after an abuse report and counted against the
// account. The banner shows the newest live warning with its case reference,
// and links to the page that explains how to contest it. It can be put away,
// because it would otherwise hold the single slot for the whole window; the
// warning stays listed in the port-forwarding view, and the next strike
// raises the banner again.
export class WarrenAccountStrikeNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenAccountStrikeNotificationContext) {}

  public mayDisplay = () => this.displayable() !== undefined;

  public getInAppNotification(): InAppNotification {
    const { strike, ordinal } = this.displayable()!;
    const threshold = this.context.accountStanding?.threshold ?? 0;
    return {
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'PORT FORWARDING WARNING'),
      subtitle: `${strikeWarning(strike, ordinal, threshold, this.context.locale)} ${strikeCaseReference(strike)}`,
      action: {
        type: 'navigate-external',
        link: {
          to: urls.reports,
          // TRANSLATORS: Provided to accessibility tools such as screenreaders
          // TRANSLATORS: to describe the link to the page on contesting a warning.
          'aria-label': messages.pgettext('accessibility', 'How to contest a warning'),
        },
        dismiss: () => this.context.dismiss(strikeDismissalKey(strike)),
      },
    };
  }

  private displayable() {
    const latest = latestStrike(this.context.accountStanding);
    if (latest === undefined) {
      return undefined;
    }
    return this.context.dismissedKeys.includes(strikeDismissalKey(latest.strike))
      ? undefined
      : latest;
  }
}
