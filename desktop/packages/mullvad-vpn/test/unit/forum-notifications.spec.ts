import { describe, expect, it } from 'vitest';

import { forumPostUrl, parseForumNotifications } from '../../src/shared/forum-notifications';

const row = {
  id: 7,
  kind: 'replied',
  unread: true,
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
        unread: true,
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

  it('opens a group inbox summary, which points at no post', () => {
    // A group message summary carries no topic, so the group's message list is
    // the only thing it has to open. Without this shape the row rendered as a
    // dead entry the reader could not click.
    expect(
      parseForumNotifications({
        notifications: [{ ...row, path: '/u/gunak-sibuf-havon/messages/group/staff' }],
      })[0].path,
    ).toBe('/u/gunak-sibuf-havon/messages/group/staff');
  });

  it('refuses anything that only resembles a group inbox', () => {
    for (const path of [
      '/u/someone/messages/group/staff/../../admin',
      '/u/someone/messages/group/staff/extra',
      '/u/someone/messages/group/',
      '/u/someone/messages',
      '//evil.example/messages/group/staff',
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
  it('goes straight to the post', () => {
    expect(forumPostUrl('/t/86/4')).toBe('https://forum.warrenbrowse.com/t/86/4');
  });

  it('never routes through the sign-in handshake', () => {
    // `/session/sso` runs the whole wallet round trip every time, including
    // for a browser already signed in to the forum, which is the common case
    // and what made every click detour through the identity broker.
    expect(forumPostUrl('/t/86/4')).not.toContain('sso');
  });
});
