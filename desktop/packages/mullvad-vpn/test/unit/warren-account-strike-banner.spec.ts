import { describe, expect, it } from 'vitest';

import { WarrenAccountStrikeNotificationProvider } from '../../src/renderer/lib/notifications';
import { strikeDismissalKey } from '../../src/shared/account-standing';
import { urls } from '../../src/shared/constants';
import { WarrenAccountStanding, WarrenAccountStrike } from '../../src/shared/daemon-rpc-types';

/** 2026-09-24T00:00:00Z. */
const DAY = 1_790_208_000;

function strike(port: number, caseReference: string): WarrenAccountStrike {
  return { dayUnixSecs: DAY, category: 'copyright', exitCountry: 'FI', port, caseReference };
}

const older = strike(50000, 'a'.repeat(32));
const newer = strike(51413, 'b'.repeat(32));

function standing(strikes: WarrenAccountStrike[]): WarrenAccountStanding {
  return { strikes, threshold: 3, windowDays: 90, ban: null };
}

function provider(accountStanding: WarrenAccountStanding | null, dismissedKeys: string[] = []) {
  const dismissed: string[] = [];
  return {
    dismissed,
    provider: new WarrenAccountStrikeNotificationProvider({
      accountStanding,
      dismissedKeys,
      dismiss: (key) => dismissed.push(key),
      locale: 'en',
    }),
  };
}

describe('the port-forwarding warning banner', () => {
  it('stays hidden in good standing and while the standing is unknown', () => {
    expect(provider(standing([])).provider.mayDisplay()).to.be.false;
    expect(provider(null).provider.mayDisplay()).to.be.false;
  });

  it('warns about the newest strike, ranked, with its case reference', () => {
    const notification = provider(standing([older, newer])).provider.getInAppNotification();

    expect(notification.indicator).to.equal('warning');
    expect(notification.subtitle).to.equal(
      'Warning 2 of 3: port 51413 was closed on September 24, 2026 after an abuse report (copyright). ' +
        `Case reference: ${'b'.repeat(32)}`,
    );
  });

  it('links to the page that explains how to contest it', () => {
    const action = provider(standing([newer])).provider.getInAppNotification().action;

    expect(action?.type).to.equal('navigate-external');
    expect(action?.type === 'navigate-external' && action.link.to).to.equal(urls.reports);
  });

  it('can be put away, remembering the warning by its digest', () => {
    const { provider: banner, dismissed } = provider(standing([newer]));
    const action = banner.getInAppNotification().action;

    if (action?.type === 'navigate-external') {
      action.dismiss?.();
    }

    expect(dismissed).to.deep.equal([strikeDismissalKey(newer)]);
  });

  it('stays away once put away, and comes back for the next strike', () => {
    const put = [strikeDismissalKey(newer)];

    expect(provider(standing([older, newer]), put).provider.mayDisplay()).to.be.false;
    expect(
      provider(standing([older, newer, strike(52000, 'c'.repeat(32))]), put).provider.mayDisplay(),
    ).to.be.true;
  });
});
