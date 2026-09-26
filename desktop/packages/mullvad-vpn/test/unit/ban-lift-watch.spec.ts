import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import BanLiftWatch from '../../src/main/ban-lift-watch';
import { WarrenAccountStanding } from '../../src/shared/daemon-rpc-types';

const T0 = 1_750_000_000_000;
const DAY_MS = 24 * 3600_000;

function standing(lapsesAtMs: number | null): WarrenAccountStanding {
  return {
    strikes: [],
    threshold: 3,
    windowDays: 90,
    ban: {
      reason: 'port-forwarding-abuse',
      bannedAtUnixSecs: T0 / 1000,
      lapsesAtUnixSecs: lapsesAtMs === null ? null : lapsesAtMs / 1000,
    },
  } as unknown as WarrenAccountStanding;
}

const clean = {
  strikes: [],
  threshold: 3,
  windowDays: 90,
  ban: null,
} as unknown as WarrenAccountStanding;

describe('the watch on a ban ending', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('reports a ban a new standing drops', () => {
    const changes: boolean[] = [];
    const watch = new BanLiftWatch((banned) => changes.push(banned));

    watch.observe(standing(null));
    watch.observe(clean);

    expect(changes).toEqual([true, false]);
    watch.dispose();
  });

  it('reports a ban reaching its lapse, which no new standing announces', async () => {
    const changes: boolean[] = [];
    const watch = new BanLiftWatch((banned) => changes.push(banned));

    watch.observe(standing(T0 + DAY_MS));
    await vi.advanceTimersByTimeAsync(DAY_MS - 1);
    expect(changes).toEqual([true]);
    await vi.advanceTimersByTimeAsync(2);

    expect(changes).toEqual([true, false]);
    watch.dispose();
  });

  // A timer cannot wait longer than 2^31 - 1 ms (about 24.8 days): a longer
  // delay fires at once. A ban of a year must not read as lifted today.
  it('waits out a lapse a year away', async () => {
    const changes: boolean[] = [];
    const watch = new BanLiftWatch((banned) => changes.push(banned));

    watch.observe(standing(T0 + 365 * DAY_MS));
    await vi.advanceTimersByTimeAsync(364 * DAY_MS);
    expect(changes).toEqual([true]);
    await vi.advanceTimersByTimeAsync(DAY_MS + 1);

    expect(changes).toEqual([true, false]);
    watch.dispose();
  });

  it('says nothing while nothing about the ban changes', () => {
    const changes: boolean[] = [];
    const watch = new BanLiftWatch((banned) => changes.push(banned));

    watch.observe(clean);
    watch.observe(undefined);
    watch.observe(standing(null));
    watch.observe(standing(null));

    expect(changes).toEqual([true]);
    watch.dispose();
  });
});
