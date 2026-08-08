import { describe, expect, it } from 'vitest';

import { iconFor, relativeTime } from '../../src/renderer/components/views/forum-activity/helpers';

describe('the glyph that sorts a notification at a glance', () => {
  it('gives replies, likes and messages each their own', () => {
    expect(iconFor('replied')).toBe('reply-outline');
    expect(iconFor('liked')).toBe('heart-outline');
    expect(iconFor('private_message')).toBe('message-outline');
  });

  it('still has one for a kind this version does not know', () => {
    expect(iconFor('other')).toBe('bell-outline');
  });
});

describe('the age shown on a notification', () => {
  const now = Date.UTC(2026, 7, 8, 12, 0, 0);
  const at = (secondsAgo: number) => Math.floor(now / 1000) - secondsAgo;

  it('reads in minutes and hours while that is the useful thing to say', () => {
    expect(relativeTime(at(5 * 60), 'en', now)).toMatch(/^5 ?m/);
    expect(relativeTime(at(2 * 3600), 'en', now)).toMatch(/^2 ?h/);
  });

  it('says yesterday rather than counting hours', () => {
    // `numeric: auto` is what buys the natural wording.
    expect(relativeTime(at(26 * 3600), 'en', now)).toBe('yesterday');
  });

  it('follows the locale without this app shipping plural rules', () => {
    expect(relativeTime(at(2 * 3600), 'fr', now)).toMatch(/2/);
  });

  it('never reads as a negative duration, in any locale', () => {
    // French `narrow` renders "2 hours ago" as "-2 h", which reads as minus
    // two hours. Reported on 2026-08-08 against a French app.
    for (const locale of ['fr', 'en', 'de', 'es', 'it', 'pt', 'nb', 'fi', 'ru', 'tr', 'ja', 'zh']) {
      for (const secondsAgo of [90, 2 * 3600, 3 * 86400]) {
        expect(relativeTime(at(secondsAgo), locale, now)).not.toMatch(/^[-−]/);
      }
    }
  });

  it('falls back to a date once the age stops being informative', () => {
    // "5 weeks ago" tells a reader less than the day it happened.
    expect(relativeTime(at(40 * 86400), 'en', now)).toMatch(/2026/);
  });

  it('never renders a future notification as a countdown', () => {
    // A clock skew between the forum host and this machine must not produce
    // "in 3 seconds" on a notification that already exists.
    expect(relativeTime(at(-3), 'en', now)).toBe('now');
  });
});
