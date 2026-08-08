import { beforeEach, describe, expect, it } from 'vitest';

import ForumActivityMonitor from '../../src/main/forum-activity-monitor';
import { SystemNotification } from '../../src/shared/notifications';

// A digest is one hex character per slot. Slot 2 is the one under test.
const NOTHING = '000000';
const ONE = '001000';
const TWO = '002000';

function harness() {
  const notified: SystemNotification[] = [];
  const indicator: boolean[] = [];
  const published: number[] = [];
  const monitor = new ForumActivityMonitor({
    notify: (notification) => notified.push(notification),
    showForumActivityIndicator: (unread) => indicator.push(unread),
    publishUnread: (count) => published.push(count),
  });
  return { monitor, notified, indicator, published };
}

describe('the forum activity monitor', () => {
  let h: ReturnType<typeof harness>;

  beforeEach(() => {
    h = harness();
    h.monitor.setEnabled(true);
    h.monitor.setSlot(2);
  });

  it('announces activity that arrived while the app was running', () => {
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(ONE);

    expect(h.notified).toHaveLength(1);
    expect(h.notified[0].message).toMatch(/forum/i);
  });

  it('stays quiet about activity that was already there at startup', () => {
    // Otherwise every relaunch re-announces the same unread notifications.
    // The tray dot still goes up, which is the honest way to carry a state
    // that predates this run.
    h.monitor.setDigest(TWO);

    expect(h.notified).toHaveLength(0);
    expect(h.indicator).toEqual([true]);
  });

  it('does not announce the same count twice', () => {
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(ONE);
    h.monitor.setDigest(ONE);

    expect(h.notified).toHaveLength(1);
  });

  it('announces again when the count rises further', () => {
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(ONE);
    h.monitor.setDigest(TWO);

    expect(h.notified).toHaveLength(2);
  });

  it('re-announces nothing when the digest lapses and comes back', () => {
    // The daemon drops the document when it cannot refresh it, and the
    // count is unknown rather than zero. Treating the gap as "all read"
    // would fire a banner for notifications the user has already seen.
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(TWO);
    expect(h.notified).toHaveLength(1);

    h.monitor.setDigest(null);
    h.monitor.setDigest(TWO);

    expect(h.notified).toHaveLength(1);
  });

  it('clears the indicator once the count reaches zero', () => {
    // Reading on the forum through any other channel advances the reader's
    // own bookmark there, so the very next digest carries zero and the dot
    // goes out without this app being told anything.
    h.monitor.setDigest(ONE);
    h.monitor.setDigest(NOTHING);

    expect(h.indicator).toEqual([true, false]);
  });

  it('raises the indicator only when it actually changes', () => {
    h.monitor.setDigest(ONE);
    h.monitor.setDigest(TWO);

    expect(h.indicator).toEqual([true]);
  });

  it('says nothing and shows nothing without the setting', () => {
    h.monitor.setEnabled(false);
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(ONE);

    expect(h.notified).toHaveLength(0);
    expect(h.indicator).toEqual([]);
  });

  it('does not announce what arrived while the setting was off', () => {
    h.monitor.setDigest(NOTHING);
    h.monitor.setEnabled(false);
    h.monitor.setDigest(TWO);
    h.monitor.setEnabled(true);

    expect(h.notified).toHaveLength(0);
    expect(h.indicator).toEqual([true]);
  });

  it('carries no watermark over from one forum account to the next', () => {
    // Slot 1 already carries 2 when we start watching it. Reading that as a
    // rise from the previous account's zero would announce, on a forum
    // login, notifications that were waiting before it.
    h.monitor.setDigest('020000');
    h.monitor.setSlot(1);

    expect(h.notified).toHaveLength(0);
    expect(h.indicator).toEqual([true]);
  });

  it('goes quiet and dark once the forum identity is gone', () => {
    h.monitor.setDigest(ONE);
    expect(h.indicator).toEqual([true]);

    h.monitor.setSlot(null);

    expect(h.indicator).toEqual([true, false]);
  });

  it('counts one notification and several differently', () => {
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(ONE);
    const single = h.notified[0].message;

    h.monitor.setDigest(TWO);
    const plural = h.notified[1].message;

    expect(single).not.toEqual(plural);
    expect(plural).toMatch(/2/);
  });

  it('publishes the count the app must show', () => {
    // The leading zero is the boot state: the renderer is told the count is
    // known and empty, rather than left guessing.
    h.monitor.setDigest(TWO);

    expect(h.published).toEqual([0, 2]);
  });

  describe('what the app itself just observed', () => {
    it('wins over the digest at once, without waiting for it to catch up', () => {
      // The digest is up to a minute of server refresh plus a client poll
      // behind. Reading the panel, or marking the list seen, tells the truth
      // now, and the badge and the dot must follow now.
      h.monitor.setDigest(TWO);
      expect(h.indicator).toEqual([true]);

      h.monitor.setObservedUnread(0);

      expect(h.indicator).toEqual([true, false]);
      expect(h.published).toEqual([0, 2, 0]);
    });

    it('keeps winning while the digest still says the stale thing', () => {
      h.monitor.setDigest(TWO);
      h.monitor.setObservedUnread(0);

      // The same document again, a minute later: still the one that predates
      // what we did, so it must not undo it.
      h.monitor.setDigest(TWO);

      expect(h.indicator).toEqual([true, false]);
    });

    it('steps aside as soon as the digest is rebuilt', () => {
      // A changed document has seen our write, or carries something newer
      // than what we observed. Either way it is now the better source, and
      // holding the observation would freeze the badge forever.
      h.monitor.setDigest(TWO);
      h.monitor.setObservedUnread(0);
      h.monitor.setDigest(ONE);

      expect(h.indicator).toEqual([true, false, true]);
    });

    it('does not announce what it observed itself', () => {
      // The user is looking at the panel; a banner about it would be absurd.
      h.monitor.setDigest(NOTHING);
      h.monitor.setObservedUnread(3);

      expect(h.notified).toHaveLength(0);
    });

    it('does not re-announce a rise it already accounted for', () => {
      // Observing 3 then seeing the digest catch up to 3 is one event, not
      // two, so the banner must not fire on the digest's arrival either.
      h.monitor.setDigest(NOTHING);
      h.monitor.setObservedUnread(3);
      h.monitor.setDigest('003000');

      expect(h.notified).toHaveLength(0);
    });
  });

  it('opens the activity panel when acted on', () => {
    h.monitor.setDigest(NOTHING);
    h.monitor.setDigest(ONE);

    expect(h.notified[0].action).toEqual({
      type: 'navigate-internal',
      link: { text: expect.any(String), to: '/forum-activity' },
    });
  });
});
