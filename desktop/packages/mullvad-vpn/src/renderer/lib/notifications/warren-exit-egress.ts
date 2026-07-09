import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenExitEgressNotificationContext {
  // Doc 62 item 5: true while the daemon's in-tunnel egress probe
  // reports the exit not forwarding (a drained or half-swapped exit
  // keeps answering QUIC keep-alives, so the tunnel state stays
  // Connected while zero traffic gets through).
  exitEgressDead: boolean;
  // The banner is only meaningful while the tunnel claims Connected:
  // in every other state the presentation already tells the truth and
  // the daemon clears the verdict on the state edge anyway.
  tunnelConnected: boolean;
}

// "Server not forwarding" banner: the distinct cause label for the
// interrupted phase when the host network is fine but the exit's
// datapath is dead. Informational: recovery is automatic (probe clears
// on success; session-liveness / drain migration handle the switch),
// so there is no action button.
export class WarrenExitEgressNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenExitEgressNotificationContext) {}

  public mayDisplay = () => this.context.exitEgressDead && this.context.tunnelConnected;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'error',
      title: messages.pgettext('in-app-notifications', 'SERVER NOT FORWARDING TRAFFIC'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'The server stopped forwarding your traffic. Warren will switch or reconnect automatically.',
      ),
    };
  }
}
