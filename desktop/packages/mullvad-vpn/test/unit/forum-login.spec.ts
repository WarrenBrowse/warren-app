import { describe, expect, it } from 'vitest';

import {
  ALLOWED_CONNECT_HOSTS,
  findForumDeepLinkArg,
  parseForumHandle,
  parseForumIdentityResponse,
  parseForumLoginUrl,
  PENDING_ATTACH_MAX_AGE_MS,
  PENDING_LOGIN_MAX_AGE_MS,
  PendingForumRequest,
  resultForProviderResponse,
} from '../../src/main/forum-login';
import { ForumIdentity } from '../../src/shared/forum-identity';
import {
  ForumLoginResult,
  IForumLoginRequest,
  isTerminalForumLoginResult,
} from '../../src/shared/forum-login';
import {
  ForumLinkFixture,
  ForumOutcomesFixture,
  importForProductEnv,
  LinkCase,
  loadClientRules,
  skippedOnDesktop,
} from './client-rules';

describe('forum-login deep link parsing', () => {
  const sid = 'a'.repeat(32);
  const good = `warren://forum-login?sid=${sid}&host=connect.warrenbrowse.com`;

  it('accepts a well-formed allowlisted link', () => {
    expect(parseForumLoginUrl(good)).toEqual({
      sid,
      host: 'connect.warrenbrowse.com',
      crossDevice: false,
    });
  });

  it('marks the QR link as cross-device so the prompt can say so', () => {
    // The provider sets xd=1 on the QR only. A relayed sign-in is
    // indistinguishable from a legitimate cross-device one on the wire, so
    // this flag is what lets the person approving tell them apart.
    expect(parseForumLoginUrl(`${good}&xd=1`)).toEqual({
      sid,
      host: 'connect.warrenbrowse.com',
      crossDevice: true,
    });
  });

  it('treats anything but xd=1 as the same-device button', () => {
    // An older provider sends no flag at all, and must degrade to the
    // ordinary prompt rather than to a warning nobody can act on.
    for (const suffix of ['', '&xd=0', '&xd=true', '&xd=']) {
      expect(parseForumLoginUrl(`${good}${suffix}`)?.crossDevice).toBe(false);
    }
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
  const request = {
    sid: 'a'.repeat(32),
    host: 'connect.warrenbrowse.com',
    crossDevice: false,
  };
  const later = {
    sid: 'b'.repeat(32),
    host: 'connect.warrenbrowse.com',
    crossDevice: false,
  };
  const t0 = 1_000_000;

  it('replays a buffered request to a renderer that subscribes later', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>(PENDING_LOGIN_MAX_AGE_MS);
    pending.set(request, t0);
    expect(pending.get(t0 + 5_000)).toEqual(request);
  });

  it('keeps the request across repeated reads so a window reload re-shows an unanswered prompt', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>(PENDING_LOGIN_MAX_AGE_MS);
    pending.set(request, t0);
    pending.get(t0 + 1_000);
    expect(pending.get(t0 + 2_000)).toEqual(request);
  });

  it('drops a request older than the server session lifetime instead of prompting for a doomed sid', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>(PENDING_LOGIN_MAX_AGE_MS);
    pending.set(request, t0);
    expect(pending.get(t0 + PENDING_LOGIN_MAX_AGE_MS + 1)).toBeUndefined();
  });

  it('keeps only the newest link when the user clicks twice', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>(PENDING_LOGIN_MAX_AGE_MS);
    pending.set(request, t0);
    pending.set(later, t0 + 1_000);
    expect(pending.get(t0 + 2_000)).toEqual(later);
  });

  it('returns nothing once cleared by an approve or cancel', () => {
    const pending = new PendingForumRequest<IForumLoginRequest>(PENDING_LOGIN_MAX_AGE_MS);
    pending.set(request, t0);
    pending.clear();
    expect(pending.get(t0 + 1_000)).toBeUndefined();
  });

  it('starts empty', () => {
    expect(
      new PendingForumRequest<IForumLoginRequest>(PENDING_LOGIN_MAX_AGE_MS).get(t0),
    ).toBeUndefined();
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

describe('provider response mapping', () => {
  it('maps a 401 carrying the clock_skew token to clock-skew', () => {
    // The one failure the user can repair themselves: connect answers the
    // frozen token {"error":"clock_skew"} (pinned in its sso_flow test) and
    // the prompt must say "fix your clock", not "try again in a moment",
    // which is the dead end every 2026-08-18 reporter hit.
    expect(resultForProviderResponse(401, '{"error":"clock_skew"}')).toBe('clock-skew');
  });

  it('keeps any other 401 and any other status generic', () => {
    expect(resultForProviderResponse(401, 'timestamp outside accepted window')).toBe('error');
    expect(resultForProviderResponse(500, '{"error":"clock_skew"}')).toBe('error');
  });

  it('maps 403 to subscription-required and 2xx to approved', () => {
    expect(resultForProviderResponse(403, '')).toBe('subscription-required');
    expect(resultForProviderResponse(200, '')).toBe('approved');
    expect(resultForProviderResponse(204, '')).toBe('approved');
  });

  it('maps 404 to expired, the session connect no longer knows', () => {
    // Expired, cancelled or already consumed: a retry on the same sid can
    // only answer 404 again, so the prompt must end rather than offer one.
    expect(resultForProviderResponse(404, 'unknown or expired session')).toBe('expired');
  });
});

describe('the terminal outcomes', () => {
  it('disarm the prompt after a refusal connect closed the session on', () => {
    expect(isTerminalForumLoginResult('subscription-required')).toBe(true);
    expect(isTerminalForumLoginResult('clock-skew')).toBe(true);
    expect(isTerminalForumLoginResult('expired')).toBe(true);
  });

  it('leave a transient failure and a success alone', () => {
    expect(isTerminalForumLoginResult('error')).toBe(false);
    expect(isTerminalForumLoginResult('approved')).toBe(false);
  });
});

// The cross-platform fixtures: one file each, replayed here, in the Rust
// crates and in the Android JVM suite (fixtures/client-rules/README.md).
describe('forum_link.json, the desktop reader', () => {
  const link = loadClientRules<ForumLinkFixture>('forum_link.json');

  it('pins the host allowlist and the two pending lifetimes', () => {
    expect(ALLOWED_CONNECT_HOSTS).toEqual(link.allowed_hosts);
    expect(PENDING_LOGIN_MAX_AGE_MS).toBe(link.pending_ttl_secs.login * 1000);
    expect(PENDING_ATTACH_MAX_AGE_MS).toBe(link.pending_ttl_secs.attach * 1000);
  });

  // The parsers answer the scheme the build registers, so the cases are
  // replayed per scheme against the modules loaded for that environment.
  function environmentOf(scheme: string): string {
    const entry = Object.entries(link.schemes).find(([, s]) => s === scheme);
    if (entry === undefined) {
      throw new Error(`no product environment registers the scheme ${scheme}`);
    }
    return entry[0];
  }

  function urlOf(fixtureCase: LinkCase<unknown>): string {
    if (fixtureCase.url === null) {
      throw new Error(`${fixtureCase.name}: a null link has no desktop input, skip it`);
    }
    return fixtureCase.url;
  }

  const schemes = [...new Set(link.login_cases.map((c) => c.expected_scheme))];

  it.each(schemes)('classifies every login case a %s build receives', async (scheme) => {
    const login = await importForProductEnv<typeof import('../../src/main/forum-login')>(
      environmentOf(scheme),
      '../../src/main/forum-login',
    );
    expect(login.FORUM_DEEP_LINK_SCHEME).toBe(scheme);
    const cases = link.login_cases.filter(
      (c) => c.expected_scheme === scheme && !skippedOnDesktop(c),
    );
    expect(cases.length).toBeGreaterThan(0);
    for (const fixtureCase of cases) {
      const parsed = login.parseForumLoginUrl(urlOf(fixtureCase));
      const accepted = fixtureCase.expect.accepted;
      if (accepted !== undefined) {
        expect(parsed, fixtureCase.name).toEqual({
          sid: accepted.sid,
          host: accepted.host,
          crossDevice: accepted.cross_device,
        });
      } else {
        // The desktop has no rejection classes: a rejected link is undefined.
        expect(fixtureCase.expect.rejected, fixtureCase.name).toBeDefined();
        expect(parsed, fixtureCase.name).toBeUndefined();
      }
    }
  });

  const attachSchemes = [...new Set(link.attach_cases.map((c) => c.expected_scheme))];

  it.each(attachSchemes)('classifies every attach case a %s build receives', async (scheme) => {
    const attach = await importForProductEnv<typeof import('../../src/main/forum-attach')>(
      environmentOf(scheme),
      '../../src/main/forum-attach',
    );
    const cases = link.attach_cases.filter(
      (c) => c.expected_scheme === scheme && !skippedOnDesktop(c),
    );
    expect(cases.length).toBeGreaterThan(0);
    for (const fixtureCase of cases) {
      const parsed = attach.parseForumAttachUrl(urlOf(fixtureCase));
      const accepted = fixtureCase.expect.accepted;
      if (accepted !== undefined) {
        expect(parsed, fixtureCase.name).toEqual({
          sid: accepted.sid,
          host: accepted.host,
          topicId: accepted.topic_id,
        });
      } else {
        expect(fixtureCase.expect.rejected, fixtureCase.name).toBeDefined();
        expect(parsed, fixtureCase.name).toBeUndefined();
      }
    }
  });
});

describe('forum_outcomes.json, the desktop reader', () => {
  const outcomes = loadClientRules<ForumOutcomesFixture>('forum_outcomes.json');

  // The desktop result for a fixture kind. A kind with no desktop result yet
  // is a divergence the fixture names in a skip list; reaching it here means
  // the skip list and the desktop drifted apart.
  function desktopResultFor(kind: string, name: string): ForumLoginResult {
    switch (kind) {
      case 'approved':
      case 'subscription-required':
      case 'clock-skew':
      case 'expired':
        return kind;
      case 'failed':
        return 'error';
      default:
        throw new Error(`${name}: the desktop has no result for the ${kind} outcome`);
    }
  }

  it('maps every login answer to the desktop result and reads the identity it carries', () => {
    const cases = outcomes.login.cases.filter((c) => !skippedOnDesktop(c));
    expect(cases.length).toBeGreaterThanOrEqual(10);
    for (const fixtureCase of cases) {
      const { name, status, body, expect: expected } = fixtureCase;
      expect(resultForProviderResponse(status, body), name).toBe(
        desktopResultFor(expected.kind, name),
      );
      if (expected.kind !== 'approved') {
        continue;
      }
      let identity: ForumIdentity | undefined;
      try {
        identity = parseForumIdentityResponse(JSON.parse(body));
      } catch {
        identity = undefined;
      }
      const wanted =
        expected.handle === undefined
          ? undefined
          : { handle: expected.handle, notifySlot: expected.notify_slot ?? null };
      expect(identity, name).toEqual(wanted);
    }
  });

  it('leaves no desktop skip on a login case: the desktop knows every outcome kind', () => {
    expect(outcomes.login.cases.filter(skippedOnDesktop)).toEqual([]);
  });

  it('disarms the prompt on exactly the terminal kinds', () => {
    expect(outcomes.login.terminal_kinds.length).toBeGreaterThan(0);
    for (const fixtureCase of outcomes.login.cases) {
      const { name, expect: expected } = fixtureCase;
      const result = desktopResultFor(expected.kind, name);
      expect(isTerminalForumLoginResult(result), name).toBe(
        outcomes.login.terminal_kinds.includes(expected.kind),
      );
    }
  });
});
