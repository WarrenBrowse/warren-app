import { describe, expect, it } from 'vitest';

import { WarrenMaintenanceNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren maintenance migration banner', () => {
  it('stays hidden in the steady state', () => {
    const provider = new WarrenMaintenanceNotificationProvider({
      maintenanceMigrationActive: false,
    });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('displays while the daemon reports an active maintenance window', () => {
    const provider = new WarrenMaintenanceNotificationProvider({
      maintenanceMigrationActive: true,
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.indicator).to.equal('success');
    // No close action: the banner is time-driven and self-dismisses
    // when the daemon pushes the expired status.
    expect(notification.action).to.be.undefined;
  });
});
