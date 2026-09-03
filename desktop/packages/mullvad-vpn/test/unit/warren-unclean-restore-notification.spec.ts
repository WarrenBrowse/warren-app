import { describe, expect, it } from 'vitest';

import { WarrenUncleanRestoreNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren unclean-shutdown restore banner', () => {
  it('stays hidden when the tunnel is up because the user asked', () => {
    const provider = new WarrenUncleanRestoreNotificationProvider({
      restoredAfterUncleanShutdown: false,
    });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('displays while the daemon reports it restored the tunnel on its own', () => {
    const provider = new WarrenUncleanRestoreNotificationProvider({
      restoredAfterUncleanShutdown: true,
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    // A tunnel nobody asked for is not good news to be told quietly.
    expect(notification.indicator).to.equal('warning');
    // No close action on purpose: the daemon clears the flag as soon as
    // the user sets the target state, so the banner is dismissed by
    // taking control of the tunnel rather than by hiding the message.
    expect(notification.action).to.be.undefined;
  });
});
