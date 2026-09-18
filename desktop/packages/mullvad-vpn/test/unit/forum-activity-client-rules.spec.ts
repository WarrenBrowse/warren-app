/* eslint-disable @typescript-eslint/naming-convention */
// The property names below are the fixture's own spelling, shared with the
// Kotlin and Swift readers of the same file, so they stay snake_case here.
import { expect, it } from 'vitest';

import ForumActivityMonitor, {
  ForumActivityMonitorDelegate,
} from '../../src/main/forum-activity-monitor';
import {
  forumHeaderButton,
  showsForumActivity,
  UNREAD_SATURATED,
  unreadForSlot,
  unreadLabel,
} from '../../src/shared/forum-identity';
import { loadClientRules } from './client-rules';

// How the broadcast digest becomes one user's badge, replayed from
// `fixtures/client-rules/forum_activity.json`, the file the Android and iOS
// readers replay too. A rule change means changing that file and every reader
// in the same commit; a reader is never loosened to pass.

interface MonitorStep {
  digest?: string | null;
  observed?: number;
  slot?: number | null;
  enabled?: boolean;
}

interface ForumActivityFixture {
  unread_saturated: number;
  unread_for_slot_cases: {
    name: string;
    digest: string | null;
    slot: number | null;
    expect: number;
  }[];
  unread_label_cases: { unread: number; expect: string }[];
  header_button_cases: { name: string; has_account: boolean; enabled: boolean; expect: string }[];
  shows_activity_cases: { has_account: boolean; enabled: boolean; expect: boolean }[];
  monitor_cases: {
    name: string;
    slot: number | null;
    enabled?: boolean;
    steps: MonitorStep[];
    expect: { unread: number; notified: number[]; indicator?: boolean };
  }[];
}

const fixture = loadClientRules<ForumActivityFixture>('forum_activity.json');

it('counts to the saturation ceiling the fixture pins', () => {
  expect(UNREAD_SATURATED).toBe(fixture.unread_saturated);
});

it('indexes every digest the way the fixture states', () => {
  for (const testCase of fixture.unread_for_slot_cases) {
    expect(unreadForSlot(testCase.digest, testCase.slot), testCase.name).toBe(testCase.expect);
  }
});

it('saturates the label rather than growing the badge', () => {
  for (const testCase of fixture.unread_label_cases) {
    expect(unreadLabel(testCase.unread)).toBe(testCase.expect);
  }
});

it('puts in the header slot what the fixture says for every pair', () => {
  for (const testCase of fixture.header_button_cases) {
    expect(forumHeaderButton(testCase.has_account, testCase.enabled), testCase.name).toBe(
      testCase.expect,
    );
  }
});

it('shows activity only for an account whose owner left the setting on', () => {
  for (const testCase of fixture.shows_activity_cases) {
    expect(showsForumActivity(testCase.has_account, testCase.enabled)).toBe(testCase.expect);
  }
});

// The storms the monitor exists to absorb, replayed step by step: what it
// publishes at the end, and every banner it raised on the way.
it('answers every storm the fixture describes', () => {
  for (const testCase of fixture.monitor_cases) {
    const notified: number[] = [];
    let published = 0;
    let indicator = false;
    const delegate: ForumActivityMonitorDelegate = {
      // The provider renders the count; the fixture pins the rise itself, so
      // the number the monitor decided to announce is what is recorded.
      notify: () => notified.push(published),
      showForumActivityIndicator: (unread: boolean) => {
        indicator = unread;
      },
      publishUnread: (count: number) => {
        published = count;
      },
    };
    const monitor = new ForumActivityMonitor(delegate);
    monitor.setEnabled(testCase.enabled ?? true);
    monitor.setSlot(testCase.slot);
    for (const step of testCase.steps) {
      if ('digest' in step) {
        monitor.setDigest(step.digest);
      } else if ('observed' in step) {
        monitor.setObservedUnread(step.observed!);
      } else if ('slot' in step) {
        monitor.setSlot(step.slot!);
      } else if ('enabled' in step) {
        monitor.setEnabled(step.enabled!);
      } else {
        throw new Error(`unknown monitor step in ${testCase.name}`);
      }
    }

    expect(published, testCase.name).toBe(testCase.expect.unread);
    expect(notified, testCase.name).toEqual(testCase.expect.notified);
    if (testCase.expect.indicator !== undefined) {
      expect(indicator, testCase.name).toBe(testCase.expect.indicator);
    }
  }
});
