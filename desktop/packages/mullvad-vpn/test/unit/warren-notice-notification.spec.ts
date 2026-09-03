import { describe, expect, it } from 'vitest';

import {
  noticeDismissalKey,
  WarrenNoticeNotificationProvider,
} from '../../src/renderer/lib/notifications/warren-notice';
import { WarrenNotice } from '../../src/shared/daemon-rpc-types';

function notice(overrides: Partial<WarrenNotice> = {}): WarrenNotice {
  return {
    id: '00000000000000a1',
    message: 'Scheduled maintenance tonight 22:00 UTC.',
    level: 'info',
    ...overrides,
  };
}

function provider(
  notices: WarrenNotice[],
  dismissedKeys: string[] = [],
  dismiss: (key: string) => void = () => undefined,
) {
  return new WarrenNoticeNotificationProvider({ notices, dismissedKeys, dismiss });
}

describe('Warren broadcast notice banner', () => {
  it('stays hidden when the operator has published nothing', () => {
    expect(provider([]).mayDisplay()).to.be.false;
  });

  it('shows the operator message verbatim', () => {
    const banner = provider([notice({ message: 'Payments are down, we are on it.' })]);
    expect(banner.mayDisplay()).to.be.true;

    const notification = banner.getInAppNotification();
    expect(notification.subtitle).to.equal('Payments are down, we are on it.');
    expect(notification.action?.type).to.equal('expand-text');
  });

  it('offers the full message for a banner that has to clamp it', () => {
    // The banner shows a few lines; anything longer would be cut with no way
    // to read the rest, so the untruncated text travels with the
    // notification and the view decides whether to offer it.
    const long = 'Lorem ipsum dolor sit amet. '.repeat(20);
    const banner = provider([notice({ message: long, level: 'warning' })]);

    const action = banner.getInAppNotification().action;

    expect(action?.type).to.equal('expand-text');
    if (action?.type === 'expand-text') {
      expect(action.expand.content).to.equal(long);
      expect(action.expand.title).to.equal(
        banner.getInAppNotification().title,
        'the modal reuses the banner label so the two cannot drift',
      );
    }
  });

  it('maps the level onto the banner indicator', () => {
    const indicatorFor = (level: WarrenNotice['level']) =>
      provider([notice({ level })]).getInAppNotification().indicator;

    expect(indicatorFor('error')).to.equal('error');
    expect(indicatorFor('warning')).to.equal('warning');
    expect(indicatorFor('info')).to.equal('success');
  });

  it('shows the first notice when several are published', () => {
    // The banner area renders exactly one notification, so a second
    // notice must not silently replace the first one published.
    const banner = provider([
      notice({ message: 'first' }),
      notice({ id: 'b2', message: 'second' }),
    ]);
    expect(banner.getInAppNotification().subtitle).to.equal('first');
  });

  it('yields the slot once the reader puts an informational notice away', () => {
    // The provider is ranked first of all, so a message the operator leaves
    // up for a week would otherwise hide the update prompt and the expiry
    // warning for that whole week.
    const read = notice({ message: 'The free beta runs one more month.' });

    expect(provider([read]).mayDisplay()).to.be.true;
    expect(provider([read], [noticeDismissalKey(read)]).mayDisplay()).to.be.false;
  });

  it('reveals the next notice when the first is put away', () => {
    const first = notice({ message: 'first' });
    const second = notice({ id: 'b2', message: 'second' });

    const banner = provider([first, second], [noticeDismissalKey(first)]);

    expect(banner.getInAppNotification().subtitle).to.equal('second');
  });

  it('raises a notice the operator rewrote in place', () => {
    const read = notice({ message: 'The free beta runs one more month.' });
    const rewritten = notice({ message: 'The free beta runs one more week.' });

    expect(provider([rewritten], [noticeDismissalKey(read)]).mayDisplay()).to.be.true;
  });

  it('keeps the slot for a warning or an error whatever the reader put away', () => {
    // Both describe something live that the user cannot act on by hiding it.
    for (const level of ['warning', 'error'] as const) {
      const alarm = notice({ level });
      const banner = provider([alarm], [noticeDismissalKey(alarm)]);

      expect(banner.mayDisplay(), level).to.be.true;

      const action = banner.getInAppNotification().action;
      expect(action?.type).to.equal('expand-text');
      if (action?.type === 'expand-text') {
        expect(action.dismiss, level).to.be.undefined;
      }
    }
  });

  it('carries a dismiss the banner can render for an informational notice', () => {
    const read = notice();
    const dismissed: string[] = [];

    const action = provider([read], [], (key) => dismissed.push(key)).getInAppNotification().action;

    expect(action?.type).to.equal('expand-text');
    if (action?.type === 'expand-text') {
      action.dismiss?.();
    }
    expect(dismissed).to.deep.equal([noticeDismissalKey(read)]);
  });
});
