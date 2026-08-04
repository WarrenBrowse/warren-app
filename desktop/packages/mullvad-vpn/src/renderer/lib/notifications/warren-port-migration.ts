import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenPortMigrationNotificationContext {
  // Live flag from the daemon: true while the last maintenance
  // migration CANCELLED for a pinned-port conflict (docs 59 C5) is
  // inside its display window. The daemon pushes a fresh status when
  // the window ends, so the banner self-dismisses without a renderer
  // timer.
  portMigrationCancellationActive: boolean;
}

// Port-conflict migration-cancelled banner (docs 59 C5). Shown when the
// daemon chose NOT to migrate off a server under maintenance because a
// port the user pinned could not be reserved on any alternative server.
// The client stays put with its ports intact and rides the short server
// update cut instead. Informational: no user action needed, no dismiss
// button; the banner drops by itself when the window elapses.
export class WarrenPortMigrationNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenPortMigrationNotificationContext) {}

  public mayDisplay = () => this.context.portMigrationCancellationActive;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'MIGRATION POSTPONED, PORT KEPT'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'Your forwarded port is busy on every alternative server, so Warren kept you on the current one. Expect a brief interruption while it updates.',
      ),
    };
  }
}
