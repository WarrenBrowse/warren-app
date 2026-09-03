import {
  ForumLoginResult,
  IForumLoginRequest,
  isTerminalForumLoginResult,
} from '../../../shared/forum-login';
import { messages } from '../../../shared/gettext';

/**
 * The consent prompt's state for one pending link, kept as plain data so the
 * transitions are unit-tested off Electron. Keyed on the link's sid: a link
 * that replaces another while the prompt is open (the user started again from
 * the browser after a terminal refusal) gets a clean prompt, and binding the
 * same link again changes nothing.
 */
export interface ForumLoginPromptState {
  request?: IForumLoginRequest;
  // A signature is out: Approve and Cancel are disabled.
  busy: boolean;
  // The last non-approved outcome, rendered inline; an approval closes the
  // prompt instead of leaving a notice.
  notice?: ForumLoginResult;
  // The provider has closed the door on this sid, so Approve is disarmed: a
  // retry could only answer "unknown session" and land on the generic line.
  terminal: boolean;
}

export const initialForumLoginPromptState: ForumLoginPromptState = {
  request: undefined,
  busy: false,
  notice: undefined,
  terminal: false,
};

/** Adopt `request`; a different sid than the current one resets everything. */
export function bindForumLoginRequest(
  state: ForumLoginPromptState,
  request: IForumLoginRequest,
): ForumLoginPromptState {
  if (state.request?.sid === request.sid) {
    return state;
  }
  return { request, busy: false, notice: undefined, terminal: false };
}

/** The user approved: the signature is in flight. */
export function beginForumLoginAttempt(state: ForumLoginPromptState): ForumLoginPromptState {
  return { ...state, busy: true, notice: undefined };
}

/** A non-approved `result` came back. */
export function settleForumLoginAttempt(
  state: ForumLoginPromptState,
  result: ForumLoginResult,
): ForumLoginPromptState {
  return { ...state, busy: false, notice: result, terminal: isTerminalForumLoginResult(result) };
}

/**
 * The inline notice for a non-approved outcome. The same words as the Android
 * prompt, so a person reading the forum's help for one platform recognises
 * the other; the clock line names the machine kind because that is what the
 * person has to fix.
 */
export function noticeForForumLoginResult(result: ForumLoginResult): string | undefined {
  switch (result) {
    case 'approved':
      return undefined;
    case 'subscription-required':
      return messages.pgettext(
        'forum-login',
        'Forum access requires a Warren subscription. This wallet has never subscribed.',
      );
    case 'clock-skew':
      return messages.pgettext(
        'forum-login',
        "Sign-in refused: this computer's clock is off by more than a minute. Enable automatic date and time, then start again from the browser page.",
      );
    case 'expired':
      return messages.pgettext(
        'forum-login',
        'This sign-in request has expired. Start again from the browser page.',
      );
    case 'error':
      return messages.pgettext('forum-login', 'Sign-in failed. Please try again in a moment.');
  }
}
