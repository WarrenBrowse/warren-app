import { hasExpired, isNeverActivatedExpiry, NEVER_ACTIVATED_EXPIRY } from '../account-expiry';
import { urls } from '../constants';
import { isBetaBuild } from '../constants/product-env';
import { TunnelState } from '../daemon-rpc-types';
import { messages } from '../gettext';
import {
  SystemNotification,
  SystemNotificationCategory,
  SystemNotificationProvider,
  SystemNotificationSeverityType,
} from './notification';

export { isNeverActivatedExpiry, NEVER_ACTIVATED_EXPIRY };

interface AccountExpiredNotificaitonContext {
  accountExpiry: string;
  tunnelState: TunnelState;
  // Injectable for tests; the build constant elsewhere.
  betaBuild?: boolean;
}

export class AccountExpiredNotificationProvider implements SystemNotificationProvider {
  public constructor(private context: AccountExpiredNotificaitonContext) {}

  public mayDisplay() {
    // Only show when disconnected since the error state handles this if the connection is closed
    // due to account expiry.
    if (this.context.tunnelState.state !== 'disconnected') {
      return false;
    }
    // A beta wallet that never registered is not out of time, it is not
    // activated yet: the wizard (or the "refresh beta access" button) does
    // that, and a toast saying the account has run out sent a first-run
    // user to uninstall (topic 195). A lapsed beta access still warns.
    if (
      (this.context.betaBuild ?? isBetaBuild) &&
      isNeverActivatedExpiry(this.context.accountExpiry)
    ) {
      return false;
    }
    return hasExpired(this.context.accountExpiry);
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: messages.pgettext('notifications', 'Account is out of time'),
      category: SystemNotificationCategory.expiry,
      severity: SystemNotificationSeverityType.high,
      presentOnce: { value: true, name: this.constructor.name },
      // Beta builds carry no purchase surface: the out-of-time view offers
      // the free "refresh beta access" recovery instead.
      action: isBetaBuild
        ? undefined
        : {
            type: 'navigate-external',
            link: {
              text: messages.pgettext('notifications', 'Buy more'),
              to: urls.purchase,
            },
          },
    };
  }
}
