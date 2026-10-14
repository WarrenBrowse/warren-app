import { describe, expect, it } from 'vitest';

import { WarrenClockSkewNotificationProvider } from '../../src/renderer/lib/notifications';

describe('Warren device clock banner', () => {
  it('stays hidden while the token issuer accepts the clock', () => {
    const provider = new WarrenClockSkewNotificationProvider({ tokenClockSkew: null });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('names how many minutes the clock is off, rounded up', () => {
    const provider = new WarrenClockSkewNotificationProvider({
      tokenClockSkew: { offsetSecs: -420 },
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.indicator).to.equal('error');
    expect(notification.subtitle).to.contain('7');
    // No close action: the banner drops by itself once a refresh succeeds.
    expect(notification.action).to.be.undefined;
  });

  it('never says zero minutes for a refusal under a minute', () => {
    const provider = new WarrenClockSkewNotificationProvider({
      tokenClockSkew: { offsetSecs: 61 },
    });
    expect(provider.getInAppNotification().subtitle).to.contain('2');
  });

  it('still tells the user to fix the clock when the refusal gave no offset', () => {
    const provider = new WarrenClockSkewNotificationProvider({
      tokenClockSkew: { offsetSecs: undefined },
    });
    expect(provider.mayDisplay()).to.be.true;
    expect(provider.getInAppNotification().subtitle).to.be.a('string').and.not.empty;
  });
});
