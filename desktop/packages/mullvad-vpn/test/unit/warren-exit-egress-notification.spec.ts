import { describe, expect, it } from 'vitest';

import { WarrenExitEgressNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren exit-egress banner', () => {
  it('stays hidden while the exit forwards traffic', () => {
    const provider = new WarrenExitEgressNotificationProvider({
      exitEgressDead: false,
      tunnelConnected: true,
    });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('displays as an error while the exit is not forwarding and the tunnel claims Connected', () => {
    const provider = new WarrenExitEgressNotificationProvider({
      exitEgressDead: true,
      tunnelConnected: true,
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.indicator).to.equal('error');
    // No close action: the banner drops by itself (probe clears the
    // verdict on the first success, and the daemon clears it whenever
    // the tunnel leaves Connected).
    expect(notification.action).to.be.undefined;
  });

  it('stays hidden outside the connected state (other states already tell the truth)', () => {
    const provider = new WarrenExitEgressNotificationProvider({
      exitEgressDead: true,
      tunnelConnected: false,
    });
    expect(provider.mayDisplay()).to.be.false;
  });
});
