// Shared types for the community-forum attach-logs flow (warren-core doc 55).
// A `warren://attach-logs` deep link asks the app to attach its redacted
// problem report to a forum topic; the app MUST show an explicit consent
// prompt, with a way to view the exact report, before signing and sending.

export interface IForumAttachRequest {
  // Opaque 32-hex session id from the deep link.
  sid: string;
  // Connect host from the deep link (validated against an allowlist in main).
  host: string;
  // The forum topic the logs get attached to. 0 means a pre-topic session:
  // the report is still being composed and the forum binds the logs to the
  // topic after creation.
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
  // The gzipped report exceeds the size cap (client- or server-side).
  | 'too-large'
  // No Warren identity yet, so there is no key to sign the report with. A
  // first-run user meets this before the account exists, and telling them to
  // "try again in a moment" sends them round a loop that cannot close.
  | 'no-identity'
  // The provider accepted the request and failed on its own side (5xx). The
  // reporter can do nothing about it and retrying now changes nothing, which
  // the generic wording promised the opposite of: four such failures over
  // three days read to the reporter as a fault of their machine.
  | 'server-error'
  // Any other failure (network error, refused request).
  | 'error';
