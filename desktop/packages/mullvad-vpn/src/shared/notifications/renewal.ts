import { sprintf } from 'sprintf-js';

import { closeToExpiry } from '../account-expiry';
import { messages } from '../gettext';
import {
  InAppNotification,
  InAppNotificationProvider,
  SystemNotification,
  SystemNotificationCategory,
  SystemNotificationProvider,
  SystemNotificationSeverityType,
} from './notification';

// Auto-renewal lifecycle notifications (warren-core doc 65). The reminder is the
// legal pre-charge notice (decision 12.2: in-app channel only), sent
// with a guaranteed lead before the merchant-initiated charge.

export class RenewalReminderNotificationProvider implements SystemNotificationProvider {
  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: messages.pgettext(
        'notifications',
        'Your Warren time renews in the next days. You can turn auto-renewal off anytime in Account settings.',
      ),
      severity: SystemNotificationSeverityType.medium,
      category: SystemNotificationCategory.renewal,
    };
  }
}

// In-app twin of the reminder: the system notification is suppressed
// while the app window is visible, and the pre-charge notice must reach
// the user either way (decision 12.2: the app IS the notice channel).
export class RenewalUpcomingInAppNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: { accountExpiry: string; renewalActive: boolean }) {}

  public mayDisplay = () => this.context.renewalActive && closeToExpiry(this.context.accountExpiry);

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'AUTO-RENEWAL SOON'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'Your Warren time renews in the next days. You can turn it off anytime in Account settings.',
      ),
    };
  }
}

// The 30-day pre-renewal notice (doc 77): shown at adopt and right
// after each charge, when the app is necessarily open.
export class RenewalUpcomingNotificationProvider implements SystemNotificationProvider {
  public constructor(private renewsAtMs: number) {}

  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    const date = new Date(this.renewsAtMs).toLocaleDateString();
    return {
      message: sprintf(
        // TRANSLATORS: The 30-day advance notice before an automatic renewal charge.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(date)s - the renewal date
        messages.pgettext(
          'notifications',
          'Your Warren time renews automatically around %(date)s, at the monthly rate. You can turn auto-renewal off anytime in Account settings.',
        ),
        { date },
      ),
      severity: SystemNotificationSeverityType.medium,
      category: SystemNotificationCategory.renewal,
    };
  }
}

// Post-charge receipt; carries the cancellation instructions (card
// network guidance for subscription merchants).
export class RenewalReceiptNotificationProvider implements SystemNotificationProvider {
  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: messages.pgettext(
        'notifications',
        'Automatic renewal charged: 1 month will be added to your Warren time. You can turn auto-renewal off anytime in Account settings.',
      ),
      severity: SystemNotificationSeverityType.medium,
      category: SystemNotificationCategory.renewal,
    };
  }
}

export class RenewalActionRequiredNotificationProvider implements SystemNotificationProvider {
  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: messages.pgettext(
        'notifications',
        'Your bank asks for a confirmation to renew. Open Warren and buy credit to confirm.',
      ),
      severity: SystemNotificationSeverityType.high,
      category: SystemNotificationCategory.renewal,
    };
  }
}

export class RenewalFailedNotificationProvider implements SystemNotificationProvider {
  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: messages.pgettext(
        'notifications',
        'Automatic renewal failed. Update your payment method or buy credit manually.',
      ),
      severity: SystemNotificationSeverityType.high,
      category: SystemNotificationCategory.renewal,
    };
  }
}

export class RenewalDisabledNotificationProvider implements SystemNotificationProvider {
  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: messages.pgettext(
        'notifications',
        'Automatic renewal has been turned off for this account.',
      ),
      severity: SystemNotificationSeverityType.medium,
      category: SystemNotificationCategory.renewal,
    };
  }
}
