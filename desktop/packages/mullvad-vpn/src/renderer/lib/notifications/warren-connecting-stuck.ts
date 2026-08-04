import { urls } from '../../../shared/constants';
import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenConnectingStuckNotificationContext {
  // Debounced flag from useConnectingStuck: the tunnel has been trying
  // to connect for longer than the stuck window.
  connectingStuck: boolean;
}

// Shown instead of the neutral "Connecting" banner when a connect attempt
// drags on: at that point the user is likely facing a real problem
// (blocked network, captive portal, dead exit), so point them at the
// community forum, the support front door, while the daemon keeps
// retrying underneath.
export class WarrenConnectingStuckNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenConnectingStuckNotificationContext) {}

  public mayDisplay = () => this.context.connectingStuck;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'TROUBLE CONNECTING?'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'This is taking longer than usual. Warren keeps retrying; if it does not connect, report the problem on our community forum.',
      ),
      action: {
        type: 'navigate-external',
        link: { to: urls.forum },
      },
    };
  }
}
