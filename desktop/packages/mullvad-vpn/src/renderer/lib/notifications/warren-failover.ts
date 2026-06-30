import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenFailoverNotificationContext {
  // Live failover counter from the daemon. Each increment represents
  // a successful exit swap triggered by `assemble_failover_for_attempt`.
  failoverCount: number;
  // Counter value the user has already acknowledged (= dismissed).
  // The banner displays whenever `failoverCount > acknowledged`.
  acknowledgedCount: number;
  // Dispatched when the user clicks the close button on the banner.
  // Should bump `acknowledgedCount` up to the current `failoverCount`
  // so the banner stays dismissed until another failover lands.
  close: () => void;
}

// Multi-exit failover banner. Shown on the connect view whenever
// the daemon reports a new failover (an alternative exit was picked
// after the previous one became unreachable). Auto-dismisses next
// time the user closes it; reappears on the next failover. Doctrine
// per `warren_competitor_comparatives`: surface the differentiator vs
// Mullvad/IVPN, which require the user to disconnect manually.
export class WarrenFailoverNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenFailoverNotificationContext) {}

  public mayDisplay = () => this.context.failoverCount > this.context.acknowledgedCount;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'EXIT SWITCHED'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'Your previous exit became unreachable. Warren routed you through an alternative server automatically.',
      ),
      action: { type: 'close', close: this.context.close },
    };
  }
}
