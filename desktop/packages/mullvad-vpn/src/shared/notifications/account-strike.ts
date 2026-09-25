import { strikeWarning } from '../account-standing';
import { WarrenAccountStrikeNotice } from '../daemon-rpc-types';
import { messages } from '../gettext';
import { RoutePath } from '../routes';
import {
  SystemNotification,
  SystemNotificationCategory,
  SystemNotificationProvider,
  SystemNotificationSeverityType,
} from './notification';

/**
 * Tells the user that a forwarded port was closed after an abuse report and
 * counted against the account. The daemon sends each strike once, so each one
 * raises one notification.
 *
 * Medium severity: it is shown even with the informational notifications
 * turned off, and it stays on screen, because three of these revoke the
 * account and the reader has to be able to act on the first.
 */
export class AccountStrikeNotificationProvider implements SystemNotificationProvider {
  public constructor(
    private notice: WarrenAccountStrikeNotice,
    private locale: string,
  ) {}

  public mayDisplay = () => true;

  public getSystemNotification(): SystemNotification {
    return {
      message: strikeWarning(
        this.notice.strike,
        this.notice.ordinal,
        this.notice.threshold,
        this.locale,
      ),
      severity: SystemNotificationSeverityType.medium,
      category: SystemNotificationCategory.accountStrike,
      action: {
        type: 'navigate-internal',
        link: {
          to: RoutePath.portForwardingSettings,
          text: messages.pgettext('notifications', 'Open port forwarding'),
        },
      },
    };
  }
}
