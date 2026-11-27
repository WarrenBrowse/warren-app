// Shared types for the community-forum in-app report:
// the "Report a problem" form behind the home lifebuoy, filed through the connect
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

/** Shortest description the broker accepts (its `MIN_REPORT_DESCRIPTION_CHARS`). */
export const FORUM_REPORT_MIN_DESCRIPTION_CHARS = 10;
/** Longest description and longest steps the broker accepts. */
export const FORUM_REPORT_MAX_DESCRIPTION_CHARS = 4_000;

/** The form as the renderer hands it to main. */
export interface IForumReportForm {
  // Both optional: the broker files a report without them, and the logs
  // answer the question better than a guess picked to get past the form.
  area?: ForumReportArea;
  frequency?: ForumReportFrequency;
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
      // The broker's one-shot link that opens the topic already signed in,
      // only when it points at the connect page the app vouches for. Single
      // use: once opened, the topic URL is what reopens the topic.
      loginUrl?: string;
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

/**
 * What became of a request to keep the collected logs on this machine: the
 * person chose a file and it was written, closed the dialog, or the write (or
 * the collection behind it) failed.
 */
export type ForumReportSaveOutcome = 'saved' | 'cancelled' | 'failed';

/** True when the failure is the upload deadline with logs attached. */
export function isForumReportUploadTimeout(result: ForumReportResult): boolean {
  return result.kind === 'failed' && result.reason === 'upload-timeout';
}

/**
 * What became of new logs sent to one of the reporter's own topics
 * (`POST /v1/forum/report/logs`).
 */
export type ForumReportLogsResult =
  | { kind: 'attached' }
  // The topic is not this wallet's: another device's wallet filed it.
  | { kind: 'not-author' }
  // The topic is closed: a new report is the way to reach the team.
  | { kind: 'topic-closed' }
  | { kind: 'subscription-required' }
  | { kind: 'clock-skew' }
  | { kind: 'rate-limited' }
  | { kind: 'too-large' }
  | { kind: 'server-error' }
  | { kind: 'no-identity' }
  // The logs could not be collected on this machine.
  | { kind: 'collect-failed' }
  | { kind: 'failed'; reason: string };

/**
 * A report this app filed, kept so the person can find it again and send new
 * logs to it. All of it is public on the forum already: the topic, and the
 * first words of the description its title is made of.
 */
export interface IForumReportRecord {
  topicId: number;
  summary: string;
  // ISO 8601, when the report was filed.
  createdAt: string;
}

/** How many filed reports the app remembers. */
export const FORUM_REPORT_RECORDS_MAX = 10;

const SUMMARY_CHARS = 60;

/** The list line of a report: its description on one line, clipped. */
export function forumReportSummary(whatHappened: string): string {
  const chars = Array.from(whatHappened.split(/\s+/).filter(Boolean).join(' '));
  return chars.length > SUMMARY_CHARS
    ? `${chars.slice(0, SUMMARY_CHARS).join('')}…`
    : chars.join('');
}

/** `list` with `record` on top, one entry per topic, capped. */
export function recordForumReport(
  list: readonly IForumReportRecord[],
  record: IForumReportRecord,
): IForumReportRecord[] {
  return [record, ...list.filter((r) => r.topicId !== record.topicId)].slice(
    0,
    FORUM_REPORT_RECORDS_MAX,
  );
}

function isForumReportRecord(value: unknown): value is IForumReportRecord {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const { topicId, summary, createdAt } = value as Record<string, unknown>;
  return (
    typeof topicId === 'number' &&
    Number.isSafeInteger(topicId) &&
    topicId > 0 &&
    typeof summary === 'string' &&
    typeof createdAt === 'string' &&
    !Number.isNaN(Date.parse(createdAt))
  );
}

/**
 * The stored list as the app may use it. Read entry by entry rather than
 * validated whole: the settings loader throws away the WHOLE settings file on
 * one malformed value, and a damaged report list must not reset the app.
 */
export function sanitizeForumReportRecords(value: unknown): IForumReportRecord[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter(isForumReportRecord)
    .map(({ topicId, summary, createdAt }) => ({ topicId, summary, createdAt }))
    .slice(0, FORUM_REPORT_RECORDS_MAX);
}
