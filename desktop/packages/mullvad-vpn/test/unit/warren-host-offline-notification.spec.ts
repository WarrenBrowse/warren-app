import { describe, expect, it } from 'vitest';

import { WarrenHostOfflineNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren host offline banner', () => {
  it('stays hidden while the host is online', () => {
    const provider = new WarrenHostOfflineNotificationProvider({
      hostOffline: false,
    });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('displays as an error while the daemon reports the host offline', () => {
    const provider = new WarrenHostOfflineNotificationProvider({
      hostOffline: true,
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.indicator).to.equal('error');
    // No close action: the banner drops by itself on the online edge
    // (the daemon pushes a fresh status).
    expect(notification.action).to.be.undefined;
  });
});
