import { describe, expect, it } from 'vitest';

import {
  ForumDigestDemand,
  forumDigestDemand,
  ForumDigestDemandReporter,
} from '../../src/main/forum-digest-demand';

describe('forumDigestDemand', () => {
  const reader = { notificationsEnabled: true, hasForumAccount: true, windowVisible: true };

  it('asks for nothing without a forum account', () => {
    expect(forumDigestDemand({ ...reader, hasForumAccount: false })).toBe('none');
  });

  it('asks for nothing with the forum notifications setting off', () => {
    expect(forumDigestDemand({ ...reader, notificationsEnabled: false })).toBe('none');
  });

  it('asks for the slow cadence while the window is hidden', () => {
    expect(forumDigestDemand({ ...reader, windowVisible: false })).toBe('background');
  });

  it('asks for the minute cadence while the window is on screen', () => {
    expect(forumDigestDemand(reader)).toBe('foreground');
  });
});

describe('ForumDigestDemandReporter', () => {
  function reporter(fail = false) {
    const sent: ForumDigestDemand[] = [];
    const subject = new ForumDigestDemandReporter((demand) => {
      sent.push(demand);
      return fail ? Promise.reject(new Error('no daemon')) : Promise.resolve();
    });
    return { sent, subject };
  }

  it('reports a demand once, however often it is recomputed', async () => {
    const { sent, subject } = reporter();

    subject.update('foreground');
    subject.update('foreground');
    await Promise.resolve();

    expect(sent).toEqual(['foreground']);
  });

  it('reports every change', () => {
    const { sent, subject } = reporter();

    subject.update('foreground');
    subject.update('background');
    subject.update('none');

    expect(sent).toEqual(['foreground', 'background', 'none']);
  });

  it('reports again after the daemon reconnects, which starts from nothing', () => {
    const { sent, subject } = reporter();

    subject.update('background');
    subject.reset();
    subject.update('background');

    expect(sent).toEqual(['background', 'background']);
  });

  it('retries a demand the daemon never received', async () => {
    const { sent, subject } = reporter(true);

    subject.update('foreground');
    await new Promise((resolve) => setTimeout(resolve, 0));
    subject.update('foreground');

    expect(sent).toEqual(['foreground', 'foreground']);
  });
});
