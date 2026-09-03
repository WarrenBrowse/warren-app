import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import {
  WarrenAnnouncementNotificationProvider,
  WarrenEnvStandDownNotificationProvider,
  WarrenHostOfflineNotificationProvider,
  WarrenNoticeNotificationProvider,
} from '../../src/renderer/lib/notifications';
import { InAppNotificationProvider } from '../../src/shared/notifications';

// The notification area holds a single slot: it builds its providers in one
// array and shows the first one whose `mayDisplay()` is true, so that array IS
// the priority ladder. The suite reads the ladder off the source rather than
// copying it, which is the same anchor `warren-announcement-card.spec.tsx`
// uses for the card's own rank.
const NOTIFICATION_AREA_SOURCE = path.resolve(
  __dirname,
  '../../src/renderer/components/NotificationArea.tsx',
);

function rankOf(source: string, name: string): number {
  const rank = source.indexOf(`new ${name}(`);
  expect(rank, `${name} is not in the notification area`).to.be.greaterThan(-1);
  return rank;
}

/** The provider the single slot would show, given what each one may display. */
function winner(candidates: Record<string, InAppNotificationProvider>): string | undefined {
  const source = fs.readFileSync(NOTIFICATION_AREA_SOURCE, 'utf8');
  return Object.entries(candidates)
    .sort(([left], [right]) => rankOf(source, left) - rankOf(source, right))
    .find(([, provider]) => provider.mayDisplay())?.[0];
}

function standDown(restorable = false) {
  return new WarrenEnvStandDownNotificationProvider({
    envYield: { yieldedTo: 'prod', restorable },
    clearEnvYield: () => undefined,
  });
}

function announcement() {
  return new WarrenAnnouncementNotificationProvider({
    announcements: [
      {
        id: 'prod-launch-2026',
        headline: 'Warren production is open',
        body: 'The production network is live, and your beta account gets a free month on it.',
        level: 'info',
        cta: null,
        voucherCode: null,
      },
    ],
    dismissedIds: [],
    dismiss: () => undefined,
  });
}

function notice() {
  return new WarrenNoticeNotificationProvider({
    notices: [{ id: 'n1', message: 'Payments are down, we are on it.', level: 'warning' }],
    dismissedKeys: [],
    dismiss: () => undefined,
  });
}

describe('the single-slot notification ladder', () => {
  it('shows the stand-down rather than the launch announcement', () => {
    // The day production opens is exactly when both fire at once on a beta
    // build: the announcement goes out, and the machine now runs prod, so beta
    // has stood down. Showing the campaign card there leaves the reader with
    // an app that refuses to connect and no word on why, nor any way back.
    const providers = {
      WarrenAnnouncementNotificationProvider: announcement(),
      WarrenEnvStandDownNotificationProvider: standDown(),
    };
    expect(providers.WarrenAnnouncementNotificationProvider.mayDisplay()).to.be.true;
    expect(providers.WarrenEnvStandDownNotificationProvider.mayDisplay()).to.be.true;

    expect(winner(providers)).to.equal('WarrenEnvStandDownNotificationProvider');
  });

  it('shows the stand-down rather than an operator notice', () => {
    // Same reason: a broadcast notice describes the service, the stand-down
    // describes this build's own refusal to work, and only one of the two can
    // be read.
    expect(
      winner({
        WarrenNoticeNotificationProvider: notice(),
        WarrenEnvStandDownNotificationProvider: standDown(),
      }),
    ).to.equal('WarrenEnvStandDownNotificationProvider');
  });

  it('leaves the announcement above the notice while this build is not stood down', () => {
    // A warning or an error notice cannot be put away, so ranking it above the
    // card would let a long-lived operator statement bury a code that stops
    // being worth anything once the campaign closes.
    expect(
      winner({
        WarrenNoticeNotificationProvider: notice(),
        WarrenAnnouncementNotificationProvider: announcement(),
        WarrenEnvStandDownNotificationProvider: new WarrenEnvStandDownNotificationProvider({
          envYield: null,
          clearEnvYield: () => undefined,
        }),
      }),
    ).to.equal('WarrenAnnouncementNotificationProvider');
  });

  it('keeps the stand-down above the connection-state banners', () => {
    expect(
      winner({
        WarrenHostOfflineNotificationProvider: new WarrenHostOfflineNotificationProvider({
          hostOffline: true,
        }),
        WarrenEnvStandDownNotificationProvider: standDown(),
      }),
    ).to.equal('WarrenEnvStandDownNotificationProvider');
  });
});
