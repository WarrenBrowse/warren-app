import { describe, expect, it } from 'vitest';

import { forumPostUrl, parseForumNotifications } from '../../src/shared/forum-notifications';

const row = {
  id: 7,
  kind: 'replied',
  read: false,
  created_at: 1_700_000_000,
  title: 'Port forwarding',
  actor: 'rudop-tijub-sozom',
  excerpt: 'Have you checked the firewall?',
  path: '/t/86/4',
};

describe('reading the forum notification list', () => {
  it('reads a well-formed row', () => {
    expect(parseForumNotifications({ notifications: [row] })).toEqual([
      {
        id: 7,
        kind: 'replied',
        read: false,
        createdAt: 1_700_000_000,
        title: 'Port forwarding',
        actor: 'rudop-tijub-sozom',
        excerpt: 'Have you checked the firewall?',
        path: '/t/86/4',
      },
    ]);
  });

  it('turns an unknown kind into a row rather than dropping it', () => {
    // A Discourse upgrade adding a type must not make a notification
    // vanish from the panel.
    const [parsed] = parseForumNotifications({ notifications: [{ ...row, kind: 'chat_mention' }] });
    expect(parsed.kind).toBe('other');
  });

  it('refuses a path that is not a forum post', () => {
    // The path is opened in the user's browser, so anything but the shape
    // the forum produces would be a way to send them elsewhere.
    for (const path of [
      'https://evil.example/phish',
      '//evil.example',
      '/t/86/4/../../admin',
      'javascript:alert(1)',
      '/u/someone',
    ]) {
      const [parsed] = parseForumNotifications({ notifications: [{ ...row, path }] });
      expect(parsed.path, `must refuse ${path}`).toBeUndefined();
    }
  });

  it('keeps the two shapes the forum does produce', () => {
    expect(parseForumNotifications({ notifications: [{ ...row, path: '/t/86' }] })[0].path).toBe(
      '/t/86',
    );
    expect(parseForumNotifications({ notifications: [{ ...row, path: '/t/86/4' }] })[0].path).toBe(
      '/t/86/4',
    );
  });

  it('drops a row with no usable identity or timestamp', () => {
    expect(parseForumNotifications({ notifications: [{ ...row, id: 'seven' }] })).toEqual([]);
    expect(parseForumNotifications({ notifications: [{ ...row, created_at: null }] })).toEqual([]);
  });

  it('bounds the text a single row can carry', () => {
    const flood = 'x'.repeat(5000);
    const [parsed] = parseForumNotifications({
      notifications: [{ ...row, title: flood, excerpt: flood }],
    });
    expect(parsed.title?.length).toBe(200);
    expect(parsed.excerpt?.length).toBe(400);
  });

  it('reads a body that is not a list as an empty panel', () => {
    // "Nothing here" is the honest rendering of a malformed answer; an
    // error the user cannot act on is not.
    expect(parseForumNotifications({})).toEqual([]);
    expect(parseForumNotifications(null)).toEqual([]);
    expect(parseForumNotifications({ notifications: 'nope' })).toEqual([]);
  });

  it('drops only the malformed rows of an otherwise usable answer', () => {
    expect(
      parseForumNotifications({ notifications: [row, 42, null, { ...row, id: 9 }] }),
    ).toHaveLength(2);
  });
});

describe('opening a notification', () => {
  it('enters through the SSO entry so the reader lands signed in', () => {
    // The forum has no password: a reader not signed in on that browser
    // would land on a page that cannot show them their own notification.
    expect(forumPostUrl('/t/86/4')).toBe(
      'https://forum.warrenbrowse.com/session/sso?return_path=%2Ft%2F86%2F4',
    );
  });
});
