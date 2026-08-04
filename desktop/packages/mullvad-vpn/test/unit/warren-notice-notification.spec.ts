import { describe, expect, it } from 'vitest';

import { WarrenNoticeNotificationProvider } from '../../src/renderer/lib/notifications';
import { WarrenNotice } from '../../src/shared/daemon-rpc-types';

function notice(overrides: Partial<WarrenNotice> = {}): WarrenNotice {
  return {
    id: '00000000000000a1',
    message: 'Scheduled maintenance tonight 22:00 UTC.',
    level: 'info',
    ...overrides,
  };
}

describe('Warren broadcast notice banner', () => {
  it('stays hidden when the operator has published nothing', () => {
    const provider = new WarrenNoticeNotificationProvider({ notices: [] });
    expect(provider.mayDisplay()).to.be.false;
  });

  it('shows the operator message verbatim', () => {
    const provider = new WarrenNoticeNotificationProvider({
      notices: [notice({ message: 'Payments are down, we are on it.' })],
    });
    expect(provider.mayDisplay()).to.be.true;

    const notification = provider.getInAppNotification();
    expect(notification.subtitle).to.equal('Payments are down, we are on it.');
    // No close action: the banner clears when the daemon pushes an empty
    // list (erased or expired), never because the user dismissed it. The one
    // action it carries opens the full text.
    expect(notification.action?.type).to.equal('expand-text');
  });

  it('offers the full message for a banner that has to clamp it', () => {
    // The banner shows a few lines; anything longer would be cut with no way
    // to read the rest, so the untruncated text travels with the
    // notification and the view decides whether to offer it.
    const long = 'Lorem ipsum dolor sit amet. '.repeat(20);
    const provider = new WarrenNoticeNotificationProvider({
      notices: [notice({ message: long, level: 'warning' })],
    });

    const action = provider.getInAppNotification().action;

    expect(action?.type).to.equal('expand-text');
    if (action?.type === 'expand-text') {
      expect(action.expand.content).to.equal(long);
      expect(action.expand.title).to.equal(
        provider.getInAppNotification().title,
        'the modal reuses the banner label so the two cannot drift',
      );
    }
  });

  it('maps the level onto the banner indicator', () => {
    const indicatorFor = (level: WarrenNotice['level']) =>
      new WarrenNoticeNotificationProvider({
        notices: [notice({ level })],
      }).getInAppNotification().indicator;

    expect(indicatorFor('error')).to.equal('error');
    expect(indicatorFor('warning')).to.equal('warning');
    expect(indicatorFor('info')).to.equal('success');
  });

  it('shows the first notice when several are published', () => {
    // The banner area renders exactly one notification, so a second
    // notice must not silently replace the first one published.
    const provider = new WarrenNoticeNotificationProvider({
      notices: [notice({ message: 'first' }), notice({ id: 'b2', message: 'second' })],
    });
    expect(provider.getInAppNotification().subtitle).to.equal('first');
  });
});
