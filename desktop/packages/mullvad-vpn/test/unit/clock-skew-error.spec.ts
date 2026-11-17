import { describe, expect, it } from 'vitest';

import { AuthFailedError, ErrorStateCause, TunnelState } from '../../src/shared/daemon-rpc-types';
import { ErrorNotificationProvider } from '../../src/shared/notifications';

function blockedForTheClock(): ErrorNotificationProvider {
  const tunnelState = {
    state: 'error',
    details: { cause: ErrorStateCause.authFailed, authFailedError: AuthFailedError.clockSkew },
  } as TunnelState;
  return new ErrorNotificationProvider({
    tunnelState,
    hasExcludedApps: false,
    splitTunnelingSupported: true,
    ban: null,
    locale: 'en',
  });
}

// The daemon blocks with `clockSkew` once two exits refused this device's
// time: the fix is in the system settings, so the banner names the clock.
describe('a block held for a refused device clock', () => {
  it('names the device clock', () => {
    const notification = blockedForTheClock().getInAppNotification()!;

    expect(notification.title).to.equal('DEVICE CLOCK IS OFF');
    expect(notification.subtitle).to.equal('This device’s clock is off: set it right to connect.');
  });
});
