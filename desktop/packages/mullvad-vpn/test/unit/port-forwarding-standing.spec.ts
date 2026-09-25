import { describe, expect, it } from 'vitest';

import { standingSection } from '../../src/renderer/features/port-forwarding/standing';
import { WarrenAccountStanding, WarrenAccountStrike } from '../../src/shared/daemon-rpc-types';

/** 2026-09-24T00:00:00Z. */
const DAY = 1_790_208_000;
const NOW_MS = (DAY + 3_600) * 1000;

function strike(port: number, caseReference: string): WarrenAccountStrike {
  return { dayUnixSecs: DAY, category: 'spam', exitCountry: null, port, caseReference };
}

function standing(
  strikes: WarrenAccountStrike[],
  ban: WarrenAccountStanding['ban'] = null,
): WarrenAccountStanding {
  return { strikes, threshold: 3, windowDays: 90, ban };
}

describe('the port-forwarding view standing section', () => {
  it('is absent in good standing and while the standing is unknown', () => {
    expect(standingSection(null, 'en', NOW_MS)).to.equal(undefined);
    expect(standingSection(standing([]), 'en', NOW_MS)).to.equal(undefined);
  });

  it('lists every live strike, oldest first, ranked, with its case reference', () => {
    const section = standingSection(
      standing([strike(50000, 'a'.repeat(32)), strike(50001, 'b'.repeat(32))]),
      'en',
      NOW_MS,
    );

    expect(section?.warnings).to.deep.equal([
      {
        text: 'Warning 1 of 3: port 50000 was closed on September 24, 2026 after an abuse report (spam).',
        reference: `Case reference: ${'a'.repeat(32)}`,
      },
      {
        text: 'Warning 2 of 3: port 50001 was closed on September 24, 2026 after an abuse report (spam).',
        reference: `Case reference: ${'b'.repeat(32)}`,
      },
    ]);
    expect(section?.ban).to.equal(undefined);
  });

  it('names the day a ban lapses', () => {
    const section = standingSection(
      standing([], {
        reason: 'port-forwarding-abuse',
        bannedAtUnixSecs: DAY,
        lapsesAtUnixSecs: DAY + 365 * 86_400,
      }),
      'en',
      NOW_MS,
    );

    expect(section?.ban).to.equal(
      'Access suspended for port-forwarding abuse until September 24, 2027.',
    );
  });

  it('says nothing of a ban that has lapsed', () => {
    const section = standingSection(
      standing([strike(50000, 'a'.repeat(32))], {
        reason: 'other',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: DAY,
      }),
      'en',
      NOW_MS,
    );

    expect(section?.ban).to.equal(undefined);
  });
});
