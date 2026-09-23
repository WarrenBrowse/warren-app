// Shared types for the community-forum wallet login (warren-core doc 55).
// A `warren://forum-login` deep link asks the app to sign a login challenge;
// the app MUST show an explicit consent prompt before signing (never a silent
// external login).

export interface IForumLoginRequest {
  // Opaque 32-hex session id from the deep link.
  sid: string;
  // Connect host from the deep link (validated against an allowlist in main).
  host: string;
  // The link came from the QR on the approval page, so the browser signing in
  // is on ANOTHER device. That is also the shape of a relayed (phished)
  // approval, and the user is the only one who can tell the two apart, so the
  // prompt says it plainly instead of asking the same question either way.
  crossDevice: boolean;
  // The sid was typed under Settings rather than delivered by a link. It is
  // the same-device id, but the page it was read off may be on another
  // device, so the completion code is shown and the handoff waits for the
  // person to ask for it.
  typedCode?: boolean;
}

/** How the sid reached the app, which decides what a bound approval shows. */
export type ForumLoginApproach = 'same-device-link' | 'cross-device-link' | 'typed-code';

export function forumLoginApproach(request: IForumLoginRequest): ForumLoginApproach {
  if (request.typedCode === true) {
    return 'typed-code';
  }
  return request.crossDevice ? 'cross-device-link' : 'same-device-link';
}

/**
 * What a bound approval hands back, validated by main: the one-time code the
 * browser that opened the sign-in must present, and on a same-device approval
 * the handoff URL that carries it to this machine's default browser. Both are
 * live credentials until the session ends: never logged, never persisted.
 */
export interface ForumLoginCompletion {
  code: string;
  handoffUrl?: string;
}

/**
 * How long after the provider's answer the code may still be shown: the
 * session's own lifetime, after which the code completes nothing.
 */
export const FORUM_LOGIN_CODE_LIFETIME_MS = 300_000;

export type ForumCompletionScreen =
  | 'returned-to-browser'
  | 'finishing-in-browser'
  | 'show-code'
  | 'show-code-relayed';
export type ForumHandoff = 'open-at-once' | 'on-button' | 'never';

/**
 * The screen and the handoff of an approved login, from how its sid arrived
 * and the completion the answer carried. Pinned by the `login.completion`
 * table of `fixtures/client-rules/forum_outcomes.json`. A QR approval never
 * opens a handoff: the browser signing in is on another device, and one a
 * provider sent anyway would carry the code to this machine's browser. A
 * same-device link answered without a handoff got the answer to a QR's id:
 * the link lost its `xd=1` on the way, which is how a relayed approval
 * reaches someone's own device, so the code comes with that warning.
 */
export function forumCompletionPlan(
  approach: ForumLoginApproach,
  completion: ForumLoginCompletion | undefined,
): { screen: ForumCompletionScreen; handoff: ForumHandoff } {
  if (completion === undefined) {
    return { screen: 'returned-to-browser', handoff: 'never' };
  }
  const hasHandoff = completion.handoffUrl !== undefined;
  switch (approach) {
    case 'same-device-link':
      return hasHandoff
        ? { screen: 'finishing-in-browser', handoff: 'open-at-once' }
        : { screen: 'show-code-relayed', handoff: 'never' };
    case 'typed-code':
      return { screen: 'show-code', handoff: hasHandoff ? 'on-button' : 'never' };
    case 'cross-device-link':
      return { screen: 'show-code', handoff: 'never' };
  }
}

/**
 * The completion screen main hands the renderer: the code, and whether the
 * "Finish in this device's browser" action has a handoff behind it. The
 * handoff URL itself stays in main.
 */
export interface ForumLoginCodeScreen {
  screen: 'finishing-in-browser' | 'show-code' | 'show-code-relayed';
  code: string;
  finishInBrowser: boolean;
}

/** The answer to an approval: the result, and the completion screen of a bound one. */
export interface ForumLoginApproval {
  result: ForumLoginResult;
  completion?: ForumLoginCodeScreen;
}

export type ForumLoginResult =
  // Signed and accepted by the provider; the browser will complete the login.
  | 'approved'
  // The wallet has never subscribed to Warren; forum access is refused.
  | 'subscription-required'
  // The provider refused the signature because this machine's clock is off by
  // more than its accepted window. The one failure the user repairs themselves.
  | 'clock-skew'
  // The provider no longer knows the session (expired, cancelled, or already
  // consumed): a retry on the same sid can only fail the same way.
  | 'expired'
  // Any other failure (no identity, network error, provider error).
  | 'error';

/**
 * True when the provider has closed the session behind this result, so the
 * same sid cannot be approved any more whatever the user changes on the
 * machine: connect cancels the session on a clock-skew or subscription
 * refusal, and an expired one is gone by definition. The prompt disarms
 * Approve on these; a transient `error` keeps it armed for a retry.
 */
export function isTerminalForumLoginResult(result: ForumLoginResult): boolean {
  return result === 'subscription-required' || result === 'clock-skew' || result === 'expired';
}

/**
 * A sign-in code as a person types it: the 32 hex characters of the session
 * id, in any case, with any spaces or dashes a display may have grouped them
 * with. Returns the canonical sid, or `undefined` for anything else. The same
 * rule as `warren_forum::normalize_sign_in_code`, pinned by the
 * `sign_in_code_cases` of `fixtures/client-rules/forum_link.json`.
 */
export function normalizeForumSignInCode(typed: string): string | undefined {
  const cleaned = typed.replace(/[\s-]/g, '').toLowerCase();
  return /^[0-9a-f]{32}$/.test(cleaned) ? cleaned : undefined;
}
