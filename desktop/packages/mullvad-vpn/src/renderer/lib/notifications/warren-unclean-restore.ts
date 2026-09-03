import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenUncleanRestoreNotificationContext {
  // Live flag from the daemon: true when it restored a Secured target
  // state that a previous run left behind after failing to exit cleanly
  // (host crash, power loss, SIGKILL). Cleared by the daemon as soon as
  // the user sets the target state.
  restoredAfterUncleanShutdown: boolean;
}

// Unclean-shutdown restore banner. Shown when the tunnel is up because
// the daemon re-armed it after a crash, and not because anything the
// user configured asked for it: this path is gated by neither
// auto-connect nor launch-at-login, so without this banner the only
// honest answer to "why am I connected?" lives in the daemon log.
//
// Deliberately has no close button. The daemon drops the flag the moment
// the user connects or disconnects, so the banner is dismissed by taking
// control of the tunnel, which is the decision the message is asking for.
export class WarrenUncleanRestoreNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenUncleanRestoreNotificationContext) {}

  public mayDisplay = () => this.context.restoredAfterUncleanShutdown;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'RECONNECTED AFTER A CRASH'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'Your device did not shut down cleanly, so Warren restored your previous connection. Neither auto-connect nor launch on start-up is on. Disconnect if you did not want this.',
      ),
    };
  }
}
