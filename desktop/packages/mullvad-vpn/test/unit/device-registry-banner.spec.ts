import { describe, expect, it } from 'vitest';

import { AuthFailedError, ErrorStateCause, TunnelState } from '../../src/shared/daemon-rpc-types';
import {
  ErrorNotificationProvider,
  InAppNotificationSubtitle,
} from '../../src/shared/notifications';
import { RoutePath } from '../../src/shared/routes';

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

function subtitleOf(provider: ErrorNotificationProvider): InAppNotificationSubtitle[] {
  const subtitle = provider.getInAppNotification()!.subtitle;
  return Array.isArray(subtitle) ? subtitle : [{ content: subtitle as string }];
}

function manageDevicesLink(provider: ErrorNotificationProvider) {
  return subtitleOf(provider).find(
    (part) =>
      part.action?.type === 'navigate-internal' && part.action.link.to === RoutePath.devices,
  );
}

// warren-core doc 115, section 7.1: the two blocked states a device of the
// account can cause open the account's device list from the banner.
describe('the blocked-state banner of a device problem', () => {
  it('says why a session has no token, and offers the device list', () => {
    const provider = blockedFor(AuthFailedError.noSessionToken);

    expect(provider.getInAppNotification()!.title).to.equal('BLOCKING INTERNET');
    expect(subtitleOf(provider)[0].content).to.equal(
      'This device has no session credentials right now. Another device of your account may be holding them.',
    );
    expect(manageDevicesLink(provider)?.content).to.equal('Manage devices');
  });

  it('keeps the device-limit message and adds the device list to it', () => {
    const provider = blockedFor(AuthFailedError.tooManyConnections);

    expect(subtitleOf(provider)[0].content).to.equal(
      'Too many simultaneous connections on this account. Disconnect another device or try connecting again shortly.',
    );
    expect(manageDevicesLink(provider)?.content).to.equal('Manage devices');
  });

  it('names the missing session token in the system notification as well', () => {
    expect(blockedFor(AuthFailedError.noSessionToken).getSystemNotification()!.message).to.equal(
      'This device has no session credentials right now. Another device of your account may be holding them.',
    );
  });

  it.each([
    AuthFailedError.expiredAccount,
    AuthFailedError.invalidAccount,
    AuthFailedError.unknown,
  ])('offers no device list for a block no other device causes (%s)', (reason) => {
    expect(manageDevicesLink(blockedFor(reason))).toBeUndefined();
  });
});
