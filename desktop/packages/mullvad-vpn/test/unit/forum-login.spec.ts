import { describe, expect, it } from 'vitest';

import { findForumLoginArg, parseForumLoginUrl } from '../../src/main/forum-login';

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
      parseForumLoginUrl(`warren://forum-login?sid=NOTHEX&host=connect.warrenbrowse.com`),
    ).toBeUndefined();
    expect(
      parseForumLoginUrl(`warren://forum-login?sid=${'A'.repeat(32)}&host=connect.warrenbrowse.com`),
    ).toBeUndefined();
  });

  it('rejects the wrong scheme or action', () => {
    expect(parseForumLoginUrl(`https://forum-login?sid=${sid}&host=connect.warrenbrowse.com`)).toBeUndefined();
    expect(parseForumLoginUrl(`warren://something-else?sid=${sid}&host=connect.warrenbrowse.com`)).toBeUndefined();
  });

  it('rejects a non-URL string without throwing', () => {
    expect(parseForumLoginUrl('not a url')).toBeUndefined();
  });

  it('finds a forum-login arg among process argv (Windows/Linux delivery)', () => {
    expect(findForumLoginArg(['/path/to/app', '--flag', good])).toBe(good);
    expect(findForumLoginArg(['/path/to/app', '--flag'])).toBeUndefined();
  });
});
