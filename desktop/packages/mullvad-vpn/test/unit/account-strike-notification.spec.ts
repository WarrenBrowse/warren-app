import { describe, expect, it } from 'vitest';

import {
  banInForce,
  formatStandingDay,
  latestStrike,
  strikeDismissalKey,
  strikeWarning,
} from '../../src/shared/account-standing';
import {
  WarrenAccountStanding,
  WarrenAccountStrike,
  WarrenAccountStrikeNotice,
} from '../../src/shared/daemon-rpc-types';
import {
  AccountStrikeNotificationProvider,
  SystemNotificationCategory,
  SystemNotificationSeverityType,
} from '../../src/shared/notifications';
import { RoutePath } from '../../src/shared/routes';

/** 2026-09-24T00:00:00Z, the precision a strike is recorded at. */
const DAY = 1_790_208_000;

function strike(port: number, caseReference: string): WarrenAccountStrike {
  return { dayUnixSecs: DAY, category: 'copyright', exitCountry: 'FI', port, caseReference };
}

const notice: WarrenAccountStrikeNotice = {
  strike: strike(51413, '8c0294aa1b2c3d4e5f60718293a4b5c6'),
  ordinal: 1,
  threshold: 3,
};

describe('the warning a strike raises', () => {
  it('ranks it against the threshold and names the port, the day and the category', () => {
    expect(strikeWarning(notice.strike, 1, 3, 'en')).to.equal(
      'Warning 1 of 3: port 51413 was closed on September 24, 2026 after an abuse report (copyright).',
    );
  });

  it('gives the rank alone while the threshold is unknown', () => {
    expect(strikeWarning(notice.strike, 2, 0, 'en')).to.match(/^Warning 2: port 51413 /);
  });

  // A strike is recorded as midnight UTC: read in local time, it would move to
  // the day before for everyone west of Greenwich.
  it('dates the strike in UTC', () => {
    const zone = process.env.TZ;
    process.env.TZ = 'America/Los_Angeles';
    try {
      expect(formatStandingDay(DAY, 'en')).to.equal('September 24, 2026');
    } finally {
      if (zone === undefined) {
        delete process.env.TZ;
      } else {
        process.env.TZ = zone;
      }
    }
  });
});

describe('the system notification of a new strike', () => {
  const notification = new AccountStrikeNotificationProvider(notice, 'en').getSystemNotification();

  it('carries the warning', () => {
    expect(notification.message).to.equal(strikeWarning(notice.strike, 1, 3, 'en'));
  });

  // Its own category: the closure raises a port change at about the same
  // time, and that toast must not replace this one.
  it('is filed apart from the port changes', () => {
    expect(notification.category).to.equal(SystemNotificationCategory.accountStrike);
    expect(notification.category).to.not.equal(SystemNotificationCategory.portForwarding);
  });

  // Shown with the informational notifications off, and not closed after a few
  // seconds like an info toast: three of these revoke the account.
  it('is shown above the informational level', () => {
    expect(notification.severity).to.equal(SystemNotificationSeverityType.medium);
  });

  it('opens the port-forwarding view, where the strikes are listed', () => {
    expect(notification.action?.type).to.equal('navigate-internal');
    expect(notification.action?.link.to).to.equal(RoutePath.portForwardingSettings);
  });
});

describe('the standing helpers', () => {
  const standing: WarrenAccountStanding = {
    strikes: [strike(50000, 'a'.repeat(32)), strike(50001, 'b'.repeat(32))],
    threshold: 3,
    windowDays: 90,
    ban: null,
  };

  it('picks the newest strike with its rank', () => {
    expect(latestStrike(standing)).to.deep.equal({ strike: standing.strikes[1], ordinal: 2 });
    expect(latestStrike({ ...standing, strikes: [] })).to.equal(undefined);
    expect(latestStrike(null)).to.equal(undefined);
  });

  it('holds a ban until its lapse instant', () => {
    const banned = {
      ...standing,
      ban: {
        reason: 'port-forwarding-abuse' as const,
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: 10,
      },
    };
    expect(banInForce(banned, 9_999)).to.deep.equal(banned.ban);
    expect(banInForce(banned, 10_000)).to.equal(undefined);
  });

  // The GUI settings file remembers the dismissed warnings: it names no case.
  it('remembers a dismissed warning without its case reference', () => {
    const key = strikeDismissalKey(notice.strike);

    expect(key).to.not.contain(notice.strike.caseReference);
    expect(key).to.equal(strikeDismissalKey({ ...notice.strike, port: 1 }));
    expect(key).to.not.equal(strikeDismissalKey(strike(51413, 'c'.repeat(32))));
  });
});
