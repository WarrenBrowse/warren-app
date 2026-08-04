import { describe, expect, it } from 'vitest';

import {
  findForumDeepLinkArg,
  parseForumHandle,
  parseForumLoginUrl,
  PendingForumRequest,
} from '../../src/main/forum-login';
import { IForumLoginRequest } from '../../src/shared/forum-login';

describe('forum-login deep link parsing', () => {
  const sid = 'a'.repeat(32);
  const good = `warren://forum-login?sid=${sid}&host=connect.warrenbrowse.com`;

  it('accepts a well-formed allowlisted link', () => {
    expect(parseForumLoginUrl(good)).toEqual({ sid, host: 'connect.warrenbrowse.com' });
  });

  it('rejects a non-allowlisted host so a hostile link cannot redirect a signed request', () => {
    const evil = `warren://forum-login?sid=${sid}&host=evil.example.com`;
    expect(parseForumLoginUrl(evil)).toBeUndefined();
  });

  it('rejects a malformed sid (not 32 lowercase hex)', () => {
    expect(
      parseForumLoginUrl('warren://forum-login?sid=NOTHEX&host=connect.warrenbrowse.com'),
    ).toBeUndefined();
    expect(
      parseForumLoginUrl(
        `warren://forum-login?sid=${'A'.repeat(32)}&host=connect.warrenbrowse.com`,
      ),
    ).toBeUndefined();
  });

  it('rejects the wrong scheme or action', () => {
    expect(
      parseForumLoginUrl(`https://forum-login?sid=${sid}&host=connect.warrenbrowse.com`),
    ).toBeUndefined();
    expect(
      parseForumLoginUrl(`warren://something-else?sid=${sid}&host=connect.warrenbrowse.com`),
    ).toBeUndefined();
  });

  it('rejects a non-URL string without throwing', () => {
    expect(parseForumLoginUrl('not a url')).toBeUndefined();
  });

  it('finds a forum-login arg among process argv (Windows/Linux delivery)', () => {
    expect(findForumDeepLinkArg(['/path/to/app', '--flag', good])).toBe(good);
    expect(findForumDeepLinkArg(['/path/to/app', '--flag'])).toBeUndefined();
  });
});

describe('pending forum-login buffer (cold-start delivery)', () => {
  const request = { sid: 'a'.repeat(32), host: 'connect.warrenbrowse.com' };
  const later = { sid: 'b'.repeat(32), host: 'connect.warrenbrowse.com' };
  const t0 = 1_000_000;

  it('replays a buffered request to a renderer that subscribes later', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>();
    pending.set(request, t0);
    expect(pending.get(t0 + 5_000)).toEqual(request);
  });

  it('keeps the request across repeated reads so a window reload re-shows an unanswered prompt', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>();
    pending.set(request, t0);
    pending.get(t0 + 1_000);
    expect(pending.get(t0 + 2_000)).toEqual(request);
  });

  it('drops a request older than the server session lifetime instead of prompting for a doomed sid', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>();
    pending.set(request, t0);
    expect(pending.get(t0 + 10 * 60 * 1000 + 1)).toBeUndefined();
  });

  it('keeps only the newest link when the user clicks twice', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>();
    pending.set(request, t0);
    pending.set(later, t0 + 1_000);
    expect(pending.get(t0 + 2_000)).toEqual(later);
  });

  it('returns nothing once cleared by an approve or cancel', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>();
    pending.set(request, t0);
    pending.clear();
    expect(pending.get(t0 + 1_000)).toBeUndefined();
  });

  it('starts empty', () => {
    expect(new PendingForumRequest<IForumLoginRequest>().get(t0)).toBeUndefined();
  });
});

describe('forum handle returned by an approved login', () => {
  it('accepts the frozen three-quint proquint shape', () => {
    expect(parseForumHandle({ status: 'approved', handle: 'lusab-babad-dovok' })).toBe(
      'lusab-babad-dovok',
    );
  });

  it('ignores a response without a handle so an older provider still logs in', () => {
    expect(parseForumHandle({ status: 'approved' })).toBeUndefined();
  });

  it('rejects anything that is not the derived shape before it reaches disk and the UI', () => {
    // The handle is persisted and rendered, so a provider answer that does not
    // look like a derived handle is dropped rather than displayed.
    expect(parseForumHandle({ handle: 'lusab-babad' })).toBeUndefined();
    expect(parseForumHandle({ handle: 'LUSAB-BABAD-DOVOK' })).toBeUndefined();
    expect(parseForumHandle({ handle: '<script>alert(1)</script>' })).toBeUndefined();
    expect(parseForumHandle({ handle: 42 })).toBeUndefined();
    expect(parseForumHandle(undefined)).toBeUndefined();
    expect(parseForumHandle(null)).toBeUndefined();
    expect(parseForumHandle('lusab-babad-dovok')).toBeUndefined();
  });
});
