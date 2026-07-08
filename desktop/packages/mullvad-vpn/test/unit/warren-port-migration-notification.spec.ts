import { describe, expect, it } from 'vitest';

import { WarrenPortMigrationNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren port-conflict migration-cancelled banner (docs 59 C5)', () => {
  it('stays hidden in the steady state', () => {
    const provider = new WarrenPortMigrationNotificationProvider({
      portMigrationCancellationActive: false,
    });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('displays while the daemon reports a cancelled migration window', () => {
    const provider = new WarrenPortMigrationNotificationProvider({
      portMigrationCancellationActive: true,
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.indicator).to.equal('warning');
    // No close action: the banner is time-driven and self-dismisses
    // when the daemon pushes the expired status.
    expect(notification.action).to.be.undefined;
  });
});
