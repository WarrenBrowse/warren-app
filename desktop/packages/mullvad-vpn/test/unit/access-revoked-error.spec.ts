import { describe, expect, it } from 'vitest';

import { AuthFailedError, ErrorStateCause, TunnelState } from '../../src/shared/daemon-rpc-types';
import { ErrorNotificationProvider } from '../../src/shared/notifications';

function blockedFor(authFailedError: AuthFailedError): ErrorNotificationProvider {
  const tunnelState = {
    state: 'error',
    details: { cause: ErrorStateCause.authFailed, authFailedError },
  } as TunnelState;
  return new ErrorNotificationProvider({
    tunnelState,
    hasExcludedApps: false,
    splitTunnelingSupported: true,
    ban: null,
    locale: 'en',
  });
}

// The daemon blocks with `invalidAccount` only for a device the server
// revoked, whether the revocation came while running or before the start.
describe('a block held for a revocation', () => {
  it.each([
    AuthFailedError.invalidAccount,
    AuthFailedError.banned,
    AuthFailedError.bannedPortForwarding,
  ])('is titled ACCESS REVOKED (%s)', (reason) => {
    expect(blockedFor(reason).getInAppNotification()!.title).to.equal('ACCESS REVOKED');
  });

  it('says the device is revoked and how to leave the block', () => {
    const subtitle = blockedFor(AuthFailedError.invalidAccount).getInAppNotification()!.subtitle;

    expect(subtitle).to.equal(
      'Blocking internet: this device has been revoked. Log in again to connect, or disconnect to unblock the internet.',
    );
  });

  it('keeps the blocking title for an account out of time', () => {
    expect(blockedFor(AuthFailedError.expiredAccount).getInAppNotification()!.title).to.equal(
      'BLOCKING INTERNET',
    );
  });
});
