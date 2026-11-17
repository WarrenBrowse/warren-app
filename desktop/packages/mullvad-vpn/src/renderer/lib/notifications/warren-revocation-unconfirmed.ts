import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenRevocationUnconfirmedNotificationContext {
  // Set by the daemon when the API refused this device as disconnected from
  // another device of the account without the wallet's signature over that
  // revocation. The daemon disconnected and kept the login and the recovery
  // phrase, since nothing proves the account holder asked.
  deviceRevocationUnconfirmed: boolean;
}

// Stays up until the login changes: no credential is issued to the device
// while the API refuses it, so there is nothing to dismiss it for.
export class WarrenRevocationUnconfirmedNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenRevocationUnconfirmedNotificationContext) {}

  public mayDisplay = () => this.context.deviceRevocationUnconfirmed;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'error',
      // TRANSLATORS: Banner title shown when the server reports this device as
      // TRANSLATORS: disconnected but the account did not sign that request.
      title: messages.pgettext('in-app-notifications', 'DISCONNECTION NOT CONFIRMED'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'The server says this device was disconnected from your account, but your account did not confirm it. Nothing was erased from this device.',
      ),
    };
  }
}
