// Shared types for the community-forum in-app report (warren-core doc 55):
// the Settings, Support, "Report a problem" form, filed through the connect
// broker with the wallet signature and the redacted logs, the way the
// Android app files it. The description is public under the anonymous forum
// name; the logs go privately to the support team.

import { ForumIdentity } from './forum-identity';

/** Where the problem happens: the broker's `Area` values, byte for byte. */
export type ForumReportArea = 'browsing' | 'connection' | 'wallet' | 'install' | 'other';

export const FORUM_REPORT_AREAS: readonly ForumReportArea[] = [
  'browsing',
  'connection',
  'wallet',
  'install',
  'other',
];

/** How often it happens: the broker's `Frequency` values. */
export type ForumReportFrequency = 'always' | 'sometimes' | 'once';

export const FORUM_REPORT_FREQUENCIES: readonly ForumReportFrequency[] = [
  'always',
  'sometimes',
  'once',
];

/** Shortest description the broker accepts (its `MIN_MESSAGE_CHARS`). */
export const FORUM_REPORT_MIN_DESCRIPTION_CHARS = 20;
/** Longest description and longest steps the broker accepts. */
export const FORUM_REPORT_MAX_DESCRIPTION_CHARS = 4_000;

/** The form as the renderer hands it to main. */
export interface IForumReportForm {
  area: ForumReportArea;
  frequency: ForumReportFrequency;
  whatHappened: string;
  // Optional; blank is sent as no field at all.
  steps?: string;
  // Collect a fresh redacted problem report and attach it.
  includeLogs: boolean;
}

/** What the broker did with the logs of a created topic. */
export type ForumReportLogs = 'attached' | 'partial' | 'none';

/**
 * The outcome of a send, the Android `ReportSubmitOutcome` vocabulary pinned by
 * `fixtures/client-rules/forum_outcomes.json` (`report`), plus `no-identity`
 * for a desktop whose daemon has no key to sign with yet.
 */
export type ForumReportResult =
  // The topic exists; `logs` says whether the staff delivery completed.
  | {
      kind: 'created';
      topicId: number;
      // The public topic URL, only when the broker's answer points at the
      // forum origin the app vouches for; absent otherwise.
      topicUrl?: string;
      logs: ForumReportLogs;
      // The forum identity the answer carried, when it did.
      identity?: ForumIdentity;
    }
  // Never paid: the website help form is the channel.
  | { kind: 'subscription-required' }
  // This machine's clock is outside the broker's window.
  | { kind: 'clock-skew' }
  // Over the per-wallet or global budget.
  | { kind: 'rate-limited' }
  // The gzipped report is over the size cap (client- or server-side).
  | { kind: 'too-large' }
  // A field is outside its caps: fix the form.
  | { kind: 'invalid' }
  // The broker failed on its own side (5xx): nothing the reporter can do.
  | { kind: 'server-error' }
  // No Warren identity yet, so there is no key to sign the report with.
  | { kind: 'no-identity' }
  // Any other failure, with its class: `transport`, `upload-timeout` (the
  // body-sized deadline ran out with logs attached, so the form offers the
  // resend without them), `build`, or `http-<status>`.
  | { kind: 'failed'; reason: string };

export type ForumReportResultKind = ForumReportResult['kind'];

/** True when the failure is the upload deadline with logs attached. */
export function isForumReportUploadTimeout(result: ForumReportResult): boolean {
  return result.kind === 'failed' && result.reason === 'upload-timeout';
}
