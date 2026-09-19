import { sprintf } from 'sprintf-js';

import { messages } from '../gettext';
import { PortChange } from '../port-forwarding-changes';
import { RoutePath } from '../routes';
import {
  SystemNotification,
  SystemNotificationCategory,
  SystemNotificationProvider,
  SystemNotificationSeverityType,
} from './notification';

/**
 * Tells the user that a forwarded public port opened, moved or closed.
 *
 * The port number is the whole point of the message: an application that
 * listens on a forwarded port keeps working only as long as it is told which
 * port it is, and the exit can move a grant at any renewal.
 */
export class PortForwardingNotificationProvider implements SystemNotificationProvider {
  public constructor(private change: PortChange) {}

  public mayDisplay = () => true;

  public getSystemNotification(): SystemNotification {
    return {
      message: this.message(),
      severity: SystemNotificationSeverityType.info,
      category: SystemNotificationCategory.portForwarding,
      action: {
        type: 'navigate-internal',
        link: {
          to: RoutePath.portForwardingSettings,
          // TRANSLATORS: Button on the port-forwarding notification, opening
          // TRANSLATORS: the port-forwarding settings.
          text: messages.pgettext('notifications', 'Open port forwarding'),
        },
      },
    };
  }

  private message(): string {
    if (this.change.state === 'mapped') {
      return sprintf(
        // TRANSLATORS: Notification shown when the exit grants a public port.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(port)d - the public port that is now forwarded
        messages.pgettext('notifications', 'Port forwarding: public port %(port)d is open'),
        { port: this.change.port },
      );
    }
    return sprintf(
      // TRANSLATORS: Notification shown when a forwarded public port is not
      // TRANSLATORS: forwarded any more. Available placeholder:
      // TRANSLATORS: %(port)d - the public port that was forwarded until now
      messages.pgettext('notifications', 'Port forwarding: public port %(port)d is no longer open'),
      { port: this.change.previousPort },
    );
  }
}
