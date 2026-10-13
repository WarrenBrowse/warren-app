import { describe, expect, it } from 'vitest';

import {
  RenewalPriceIncreaseSkippedNotificationProvider,
  RenewalUpcomingNotificationProvider,
} from '../../src/shared/notifications/renewal';

const RENEWS_AT = Date.UTC(2026, 11, 1);

describe('RenewalUpcomingNotificationProvider', () => {
  it('names the date and the monthly amount', () => {
    const message = new RenewalUpcomingNotificationProvider(
      {
        renewsAtMs: RENEWS_AT,
        shownAtMs: RENEWS_AT - 40 * 86_400_000,
        currency: 'EUR',
        monthlyCents: 700,
      },
      'en',
    ).getSystemNotification().message;

    expect(message).toContain(new Date(RENEWS_AT).toLocaleDateString('en'));
    expect(message).toContain('€7.00');
  });

  it('names the old and the new price when it announces a raise', () => {
    const message = new RenewalUpcomingNotificationProvider(
      {
        renewsAtMs: RENEWS_AT,
        shownAtMs: RENEWS_AT - 40 * 86_400_000,
        currency: 'EUR',
        monthlyCents: 800,
        previousMonthlyCents: 700,
      },
      'en',
    ).getSystemNotification().message;

    expect(message).toContain('€7.00');
    expect(message).toContain('€8.00');
  });
});

describe('RenewalPriceIncreaseSkippedNotificationProvider', () => {
  it('says the renewal was not charged and when the time ends', () => {
    const message = new RenewalPriceIncreaseSkippedNotificationProvider(
      {
        renewsAtMs: RENEWS_AT,
        shownAtMs: RENEWS_AT - 86_400_000,
        currency: 'EUR',
        monthlyCents: 800,
      },
      'en',
    ).getSystemNotification().message;

    expect(message).toContain('not charged');
    expect(message).toContain(new Date(RENEWS_AT).toLocaleDateString('en'));
  });
});
