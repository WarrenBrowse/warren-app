import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenHostOfflineNotificationContext {
  // Live flag from the daemon's offline monitor, pushed on the edge
  // itself: true while the host has no usable route to the internet.
  // Independent of the tunnel state on purpose: the state machine
  // holds Connected through its migration grace window, and the
  // multi-hop supervisor redials transparently, so without this flag
  // the user would see a green "Connected" with no working network.
  hostOffline: boolean;
}

// Immediate "no internet" banner. Shown in every tunnel state while
// the daemon reports the host offline; drops by itself on the online
// edge (the daemon pushes a fresh status). Informational: Warren
// reconnects automatically when the network returns, so there is no
// action button.
export class WarrenHostOfflineNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenHostOfflineNotificationContext) {}

  public mayDisplay = () => this.context.hostOffline;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'error',
      title: messages.pgettext('in-app-notifications', 'NO INTERNET CONNECTION'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'Your device is offline. Warren will reconnect automatically as soon as the network is back.',
      ),
    };
  }
}
