import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenMaintenanceNotificationContext {
  // Live flag from the daemon: true while the last drain-triggered
  // exit switch (server maintenance, ADR 36) is inside its display
  // window. The daemon pushes a fresh status when the window ends,
  // so the banner self-dismisses without any renderer timer.
  maintenanceMigrationActive: boolean;
}

// Maintenance migration banner. Shown on the connect view while the
// daemon reports it proactively moved the tunnel off an exit that
// entered its maintenance window (fleet rollout drain). Informational
// only: the switch is gap-free and needs no user action, so there is
// no dismiss button; the banner drops by itself when the maintenance
// window elapses.
export class WarrenMaintenanceNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenMaintenanceNotificationContext) {}

  public mayDisplay = () => this.context.maintenanceMigrationActive;

  public getInAppNotification(): InAppNotification {
    return {
      indicator: 'success',
      title: messages.pgettext('in-app-notifications', 'SERVER MAINTENANCE'),
      subtitle: messages.pgettext(
        'in-app-notifications',
        'Your server is being updated. Warren moved you to another server automatically, no action needed.',
      ),
    };
  }
}
