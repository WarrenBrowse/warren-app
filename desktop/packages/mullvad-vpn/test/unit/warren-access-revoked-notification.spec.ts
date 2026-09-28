import { describe, expect, it } from 'vitest';

import { WarrenAccessRevokedNotificationProvider } from '../../src/renderer/lib/notifications';
import {
  TunnelState,
  WarrenAccountBan,
  WarrenAccountStanding,
} from '../../src/shared/daemon-rpc-types';

/** 2026-09-24T00:00:00Z. */
const DAY = 1_790_208_000;
const NOW_MS = (DAY + 3_600) * 1000;

const disconnected: TunnelState = { state: 'disconnected', lockedDown: false };

function standing(ban: WarrenAccountBan | null): WarrenAccountStanding {
  return { strikes: [], threshold: 3, windowDays: 90, ban };
}

function provider(tunnelState: TunnelState, ban: WarrenAccountBan | null) {
  return new WarrenAccessRevokedNotificationProvider({
    tunnelState,
    accountStanding: standing(ban),
    locale: 'en',
    nowMs: () => NOW_MS,
  });
}

describe('the access revoked banner', () => {
  it('says why a disconnected client does not connect while a ban holds', () => {
    const banner = provider(disconnected, {
      reason: 'other',
      bannedAtUnixSecs: DAY,
      lapsesAtUnixSecs: DAY + 30 * 86_400,
    });

    expect(banner.mayDisplay()).to.be.true;
    const notification = banner.getInAppNotification();
    expect(notification.title).to.equal('ACCESS REVOKED');
    expect(notification.subtitle).to.equal(
      'Access revoked until October 24, 2026. Connecting is not possible while access is revoked.',
    );
    expect(notification.action).to.equal(undefined);
  });

  it('links a port-forwarding ban to the page on contesting it', () => {
    const banner = provider(disconnected, {
      reason: 'port-forwarding-abuse',
      bannedAtUnixSecs: null,
      lapsesAtUnixSecs: null,
    });

    expect(banner.getInAppNotification().action?.type).to.equal('navigate-external');
  });

  it('stays away once the ban has lapsed', () => {
    const banner = provider(disconnected, {
      reason: 'other',
      bannedAtUnixSecs: null,
      lapsesAtUnixSecs: DAY,
    });

    expect(banner.mayDisplay()).to.be.false;
  });

  it('leaves a tunnel the ban blocked to the error banner', () => {
    const banner = provider(
      {
        state: 'error',
        details: { cause: 'auth-failed' } as never,
      } as TunnelState,
      { reason: 'other', bannedAtUnixSecs: null, lapsesAtUnixSecs: null },
    );

    expect(banner.mayDisplay()).to.be.false;
  });

  it('stays away in good standing', () => {
    expect(provider(disconnected, null).mayDisplay()).to.be.false;
  });
});
