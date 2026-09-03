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
