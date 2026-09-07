import { describe, expect, it } from 'vitest';

import { TunnelState } from '../../src/shared/daemon-rpc-types';
import {
  AccountExpiredNotificationProvider,
  isNeverActivatedExpiry,
  NEVER_ACTIVATED_EXPIRY,
} from '../../src/shared/notifications/account-expired';

const disconnected: TunnelState = { state: 'disconnected', lockedDown: false };
const lapsed = '2020-01-01T00:00:00.000Z';

// A fresh beta wallet has no subscription row until the empty-voucher
// register runs, and the account-data cache renders that as the epoch
// expiry. The "Account is out of time" toast fired on that state on a
// Windows first run (topic 195): the window had hidden on blur, the toast
// was the only thing the user saw, and it read as "the account is broken".
describe('AccountExpiredNotificationProvider on a never-activated beta wallet', () => {
  it('names the never-activated sentinel the cache produces', () => {
    expect(isNeverActivatedExpiry(NEVER_ACTIVATED_EXPIRY)).toBe(true);
    expect(isNeverActivatedExpiry(lapsed)).toBe(false);
  });

  it('stays silent on a beta build while the wallet was never activated', () => {
    const provider = new AccountExpiredNotificationProvider({
      accountExpiry: NEVER_ACTIVATED_EXPIRY,
      tunnelState: disconnected,
      betaBuild: true,
    });
    expect(provider.mayDisplay()).toBe(false);
  });

  it('still fires on a beta build for an access that lapsed', () => {
    const provider = new AccountExpiredNotificationProvider({
      accountExpiry: lapsed,
      tunnelState: disconnected,
      betaBuild: true,
    });
    expect(provider.mayDisplay()).toBe(true);
  });

  it('keeps firing on a prod build, where the epoch expiry means a plan is owed', () => {
    const provider = new AccountExpiredNotificationProvider({
      accountExpiry: NEVER_ACTIVATED_EXPIRY,
      tunnelState: disconnected,
      betaBuild: false,
    });
    expect(provider.mayDisplay()).toBe(true);
  });
});
