import { sprintf } from 'sprintf-js';

import { closeToExpiry } from '../account-expiry';
import { messages } from '../gettext';
import { formatMinorUnits, RenewalNotice } from '../renewal';
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

// The 30-day pre-renewal notice (doc 77): the date, the monthly amount and
// the way out, and a raise names the old and the new price (directive
// 2018/1972 art. 105(4)). The same notice is archived as a PDF.
export class RenewalUpcomingNotificationProvider implements SystemNotificationProvider {
  public constructor(
    private notice: RenewalNotice,
    private locale: string,
  ) {}

  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    const { notice, locale } = this;
    const date = new Date(notice.renewsAtMs).toLocaleDateString(locale);
    const price =
      notice.monthlyCents === undefined
        ? undefined
        : formatMinorUnits(notice.monthlyCents, notice.currency, locale);
    let message: string;
    if (price !== undefined && notice.previousMonthlyCents !== undefined) {
      message = sprintf(
        // TRANSLATORS: Notice of a price increase of the automatic renewal.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(previous)s - the former monthly price
        // TRANSLATORS: %(price)s - the new monthly price
        // TRANSLATORS: %(date)s - the renewal date the new price applies from
        messages.pgettext(
          'notifications',
          'The monthly price of your automatic renewal changes from %(previous)s to %(price)s from around %(date)s. You can turn auto-renewal off anytime in Account settings, at no cost.',
        ),
        {
          previous: formatMinorUnits(notice.previousMonthlyCents, notice.currency, locale),
          price,
          date,
        },
      );
    } else if (price !== undefined) {
      message = sprintf(
        // TRANSLATORS: The 30-day advance notice before an automatic renewal charge.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(date)s - the renewal date
        // TRANSLATORS: %(price)s - the monthly price
        messages.pgettext(
          'notifications',
          'Your Warren time renews automatically around %(date)s, at %(price)s per month. You can turn auto-renewal off anytime in Account settings, at no cost.',
        ),
        { date, price },
      );
    } else {
      message = sprintf(
        // TRANSLATORS: The 30-day advance notice before an automatic renewal charge.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(date)s - the renewal date
        messages.pgettext(
          'notifications',
          'Your Warren time renews automatically around %(date)s, at the monthly rate. You can turn auto-renewal off anytime in Account settings.',
        ),
        { date },
      );
    }
    return {
      message,
      severity: SystemNotificationSeverityType.medium,
      category: SystemNotificationCategory.renewal,
    };
  }
}

// A price raised after the period's notice: that renewal is not charged.
export class RenewalPriceIncreaseSkippedNotificationProvider implements SystemNotificationProvider {
  public constructor(
    private notice: RenewalNotice,
    private locale: string,
  ) {}

  public mayDisplay() {
    return true;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: sprintf(
        // TRANSLATORS: The price rose after the renewal notice, so the renewal is not charged.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(date)s - the date the Warren time ends
        messages.pgettext(
          'notifications',
          'The monthly price changed after your renewal notice, so this renewal was not charged. Your Warren time ends around %(date)s unless you buy more.',
        ),
        { date: new Date(this.notice.renewsAtMs).toLocaleDateString(this.locale) },
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
