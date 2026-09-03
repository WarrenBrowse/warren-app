import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import { announcementUrlToOpen } from '../../src/main/warren-announcement-links';
import { WarrenAnnouncement } from '../../src/shared/daemon-rpc-types';

const NOTIFICATION_AREA_SOURCE = path.resolve(
  __dirname,
  '../../src/renderer/components/NotificationArea.tsx',
);

function announcement(url: string | null): WarrenAnnouncement {
  return {
    id: 'prod-launch-2026',
    headline: 'Warren production is open',
    body: 'The production network is live.',
    level: 'info',
    cta: url === null ? null : { label: 'Read more', url },
    voucherCode: null,
  };
}

describe('opening an announcement call to action', () => {
  it('opens a destination the published announcement carries', () => {
    // The operator picks the destination, so it is never one of the app's own
    // ten links; the signed snapshot is what admits it.
    const opened = announcementUrlToOpen('https://warren.ro/production', [
      announcement('https://warren.ro/production'),
    ]);

    expect(opened).to.equal('https://warren.ro/production');
  });

  it('opens the host root the card normalised', () => {
    // The card hands over the href of the URL it parsed, which appends the
    // root path the operator did not write.
    const opened = announcementUrlToOpen('https://warren.ro/', [announcement('https://warren.ro')]);

    expect(opened).to.equal('https://warren.ro/');
  });

  it('opens nothing that no published announcement offered', () => {
    expect(announcementUrlToOpen('https://example.com/', [announcement('https://warren.ro/')])).to
      .be.undefined;
    expect(announcementUrlToOpen('https://warren.ro/', [announcement(null)])).to.be.undefined;
    expect(announcementUrlToOpen('https://warren.ro/', [])).to.be.undefined;
    expect(announcementUrlToOpen('https://warren.ro/', undefined)).to.be.undefined;
  });

  it('opens nothing that is not https, whatever the snapshot says', () => {
    // A snapshot is verified, not trusted to be sane: the scheme gate is the
    // one thing that stops a card from launching a local handler.
    expect(announcementUrlToOpen('file:///etc/passwd', [announcement('file:///etc/passwd')])).to.be
      .undefined;
    expect(announcementUrlToOpen('not a url', [announcement('not a url')])).to.be.undefined;
  });

  it('is the channel the notification area sends the card through', () => {
    // The app's own `openUrl` drops anything outside its fixed allowlist, in
    // silence, which is what made the button dead.
    const source = fs.readFileSync(NOTIFICATION_AREA_SOURCE, 'utf8');
    const opener = source.slice(
      source.indexOf('const openAnnouncementCta'),
      source.indexOf('const clearEnvYieldNow'),
    );

    expect(opener).to.include('openAnnouncementUrl');
    expect(opener).to.not.include('openUrl(');
  });
});
