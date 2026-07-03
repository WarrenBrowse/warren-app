// Shared types for the community-forum wallet login (warren-core doc 55).
// A `warren://forum-login` deep link asks the app to sign a login challenge;
// the app MUST show an explicit consent prompt before signing (never a silent
// external login).

export interface IForumLoginRequest {
  // Opaque 32-hex session id from the deep link.
  sid: string;
  // Connect host from the deep link (validated against an allowlist in main).
  host: string;
}

export type ForumLoginResult =
  // Signed and accepted by the provider; the browser will complete the login.
  | 'approved'
  // The wallet has never subscribed to Warren; forum access is refused.
  | 'subscription-required'
  // Any other failure (no identity, network error, provider error).
  | 'error';
