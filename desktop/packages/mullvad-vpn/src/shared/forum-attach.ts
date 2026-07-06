// Shared types for the community-forum attach-logs flow (warren-core doc 55).
// A `warren://attach-logs` deep link asks the app to attach its redacted
// problem report to a forum topic; the app MUST show an explicit consent
// prompt, with a way to view the exact report, before signing and sending.

export interface IForumAttachRequest {
  // Opaque 32-hex session id from the deep link.
  sid: string;
  // Connect host from the deep link (validated against an allowlist in main).
  host: string;
  // The forum topic the logs get attached to.
  topicId: number;
  // Problem-report id collected when the deep link arrived. It lets the
  // renderer show exactly what would be sent (problemReport.viewLog) and
  // main re-read that same report at approve time. Absent when that first
  // collection failed: the prompt still shows (hiding the preview button)
  // and approve retries the collection, so a transient collector failure
  // never silently swallows the user's request.
  reportId?: string;
}

export type ForumAttachResult =
  // Signed, sent, and attached to the topic by the provider.
  | 'attached'
  // The wallet is not the author of the topic; the provider refused.
  | 'not-author'
  // The attach session expired or is unknown; restart from the forum page.
  | 'expired'
  // The gzipped report exceeds the 1 MiB cap (client- or server-side).
  | 'too-large'
  // Any other failure (no identity, network error, provider error).
  | 'error';
