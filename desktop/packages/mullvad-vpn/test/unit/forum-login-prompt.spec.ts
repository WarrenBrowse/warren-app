import { describe, expect, it } from 'vitest';

import {
  beginForumLoginAttempt,
  bindForumLoginRequest,
  completeForumLoginAttempt,
  forumLoginCodeExpired,
  initialForumLoginPromptState,
  noticeForForumLoginResult,
  revealForumLoginCode,
  settleForumLoginAttempt,
} from '../../src/renderer/features/forum-login/prompt-state';
import {
  FORUM_LOGIN_CODE_LIFETIME_MS,
  ForumLoginCodeScreen,
  IForumLoginRequest,
} from '../../src/shared/forum-login';

const request: IForumLoginRequest = {
  sid: 'a'.repeat(32),
  host: 'connect.warrenbrowse.com',
  crossDevice: false,
};
const another: IForumLoginRequest = { ...request, sid: 'b'.repeat(32) };

describe('the consent prompt state for one pending link', () => {
  it('starts with nothing to show', () => {
    expect(initialForumLoginPromptState.request).toBeUndefined();
    expect(initialForumLoginPromptState.busy).toBe(false);
    expect(initialForumLoginPromptState.terminal).toBe(false);
    expect(initialForumLoginPromptState.notice).toBeUndefined();
  });

  it('adopts a link with a clean slate', () => {
    const state = bindForumLoginRequest(initialForumLoginPromptState, request);
    expect(state.request).toEqual(request);
    expect(state.busy).toBe(false);
    expect(state.notice).toBeUndefined();
    expect(state.terminal).toBe(false);
  });

  it('marks a signature in flight and forgets the last notice', () => {
    const bound = bindForumLoginRequest(initialForumLoginPromptState, request);
    const refused = settleForumLoginAttempt(bound, 'error');
    const state = beginForumLoginAttempt(refused);
    expect(state.busy).toBe(true);
    expect(state.notice).toBeUndefined();
  });

  it('keeps Approve armed after a transient failure so the person can retry', () => {
    const bound = beginForumLoginAttempt(
      bindForumLoginRequest(initialForumLoginPromptState, request),
    );
    const state = settleForumLoginAttempt(bound, 'error');
    expect(state.busy).toBe(false);
    expect(state.notice).toBe('error');
    expect(state.terminal).toBe(false);
  });

  it('disarms Approve once connect has closed the session behind the outcome', () => {
    // A retry on the same sid after a clock-skew or subscription refusal, or
    // on an expired session, can only land on "unknown session".
    for (const result of ['clock-skew', 'subscription-required', 'expired'] as const) {
      const bound = beginForumLoginAttempt(
        bindForumLoginRequest(initialForumLoginPromptState, request),
      );
      const state = settleForumLoginAttempt(bound, result);
      expect(state.terminal, result).toBe(true);
      expect(state.notice, result).toBe(result);
      expect(state.busy, result).toBe(false);
    }
  });

  it('gives a fresh link a clean prompt after a terminal refusal', () => {
    // The person started again from the browser page: the new sid must not
    // inherit the disarmed Approve of the one connect cancelled.
    const bound = bindForumLoginRequest(initialForumLoginPromptState, request);
    const refused = settleForumLoginAttempt(bound, 'expired');
    const state = bindForumLoginRequest(refused, another);
    expect(state.request).toEqual(another);
    expect(state.terminal).toBe(false);
    expect(state.notice).toBeUndefined();
  });

  it('leaves the prompt alone when the same link is bound again', () => {
    const bound = bindForumLoginRequest(initialForumLoginPromptState, request);
    const refused = settleForumLoginAttempt(bound, 'expired');
    expect(bindForumLoginRequest(refused, request)).toBe(refused);
  });
});

describe('the inline notice of a non-approved outcome', () => {
  it('names the expired session and sends the person back to the browser page', () => {
    expect(noticeForForumLoginResult('expired')).toBe(
      'This sign-in request has expired. Start again from the browser page.',
    );
  });

  it('has its own words for every refusal and one generic line for the rest', () => {
    expect(noticeForForumLoginResult('subscription-required')).toMatch(/subscription/);
    expect(noticeForForumLoginResult('clock-skew')).toMatch(/clock/);
    expect(noticeForForumLoginResult('error')).toBe(
      'Sign-in failed. Please try again in a moment.',
    );
  });

  it('shows nothing for an approval, which closes the prompt instead', () => {
    expect(noticeForForumLoginResult('approved')).toBeUndefined();
  });
});

describe('the completion screen of a bound approval', () => {
  const handoff: ForumLoginCodeScreen = {
    screen: 'finishing-in-browser',
    code: '042917',
    finishInBrowser: false,
  };
  const showCode: ForumLoginCodeScreen = {
    screen: 'show-code',
    code: '042917',
    finishInBrowser: true,
  };
  const inFlight = beginForumLoginAttempt(
    bindForumLoginRequest(initialForumLoginPromptState, request),
  );

  it('keeps the code behind "Show the code" while the browser finishes the sign-in', () => {
    const state = completeForumLoginAttempt(inFlight, handoff, 1_000);
    expect(state.busy).toBe(false);
    expect(state.completion?.screen).toBe('finishing-in-browser');
    expect(state.completion?.codeRevealed).toBe(false);
    expect(revealForumLoginCode(state).completion?.codeRevealed).toBe(true);
  });

  it('shows the code at once on the code screen, with the finish action it was given', () => {
    const state = completeForumLoginAttempt(inFlight, showCode, 1_000);
    expect(state.completion?.codeRevealed).toBe(true);
    expect(state.completion?.code).toBe('042917');
    expect(state.completion?.finishInBrowser).toBe(true);
  });

  it('shows the code at once under the relay warning', () => {
    const relayed: ForumLoginCodeScreen = { ...showCode, screen: 'show-code-relayed' };
    const state = completeForumLoginAttempt(inFlight, relayed, 1_000);
    expect(state.completion?.screen).toBe('show-code-relayed');
    expect(state.completion?.codeRevealed).toBe(true);
  });

  it('forgets the code once the session behind it is dead', () => {
    const state = completeForumLoginAttempt(inFlight, showCode, 1_000);
    expect(forumLoginCodeExpired(state, 1_000 + FORUM_LOGIN_CODE_LIFETIME_MS - 1)).toBe(false);
    expect(forumLoginCodeExpired(state, 1_000 + FORUM_LOGIN_CODE_LIFETIME_MS)).toBe(true);
    expect(forumLoginCodeExpired(initialForumLoginPromptState, 0)).toBe(false);
  });

  it('gives a new link a clean prompt instead of the last code', () => {
    const state = bindForumLoginRequest(
      completeForumLoginAttempt(inFlight, showCode, 1_000),
      another,
    );
    expect(state.completion).toBeUndefined();
    expect(state.request).toEqual(another);
  });
});
