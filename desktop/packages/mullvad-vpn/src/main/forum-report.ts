import { status as grpcStatus } from '@grpc/grpc-js';

import { urls } from '../shared/constants/urls';
import { ForumIdentity } from '../shared/forum-identity';
import {
  ForumReportLogs,
  ForumReportLogsResult,
  ForumReportResult,
  IForumReportForm,
} from '../shared/forum-report';
import log from '../shared/logging';
import { DaemonRpc, ForumLoginSignature } from './daemon-rpc';
import { MAX_LOG_GZ_BYTES } from './forum-attach';
import { ALLOWED_CONNECT_HOSTS, getForumSession, parseForumIdentityResponse } from './forum-login';

/**
 * Community-forum in-app report, the desktop twin of
 * the Android "Report a problem" screen.
 *
 * The renderer hands main the form; main collects a fresh redacted problem
 * report when asked, gzips it, asks the daemon to build and sign the
 * canonical `POST /v1/forum/report` body with the Warren identity key
 * (through the crate the mobile clients sign with, so the bytes agree), and
 * POSTs that body verbatim to the allowlisted connect host under a deadline
 * sized to the body. The signing key never leaves the daemon; the renderer
 * never sees the log content, only the outcome.
 */

/** The forum's `Platform` tags a desktop build can carry. */
export type ForumReportPlatform = 'android' | 'ios' | 'linux' | 'macos' | 'windows';

/** The machine facts the body carries next to the form, all public. */
export interface ForumReportFacts {
  platform: ForumReportPlatform;
  appVersion: string;
  osVersion: string;
  // The app's display locale; the broker applies it at account creation.
  locale: string;
}

/** warren-connect's `MAX_FACT_CHARS`: the cap on `app_version` and `os_version`. */
const MAX_FACT_CHARS = 80;
/** warren-connect's `MAX_LOCALE_CHARS`, and the shape it enforces below. */
const MAX_LOCALE_CHARS = 10;
const MIN_LOCALE_CHARS = 2;

/** The forum tag for a Node platform name, or `undefined` off the desktop three. */
export function forumReportPlatform(platform: NodeJS.Platform): ForumReportPlatform | undefined {
  switch (platform) {
    case 'darwin':
      return 'macos';
    case 'win32':
      return 'windows';
    case 'linux':
      return 'linux';
    default:
      return undefined;
  }
}

/**
 * The device line of the report body: the operating system as Node names it,
 * its release and the CPU architecture. The report header carries the precise
 * OS version; this field is the broker's fallback when the header has none.
 */
export function desktopOsVersion(platform: NodeJS.Platform, release: string, arch: string): string {
  switch (platform) {
    case 'darwin':
      return `macOS (Darwin ${release}, ${arch})`;
    case 'win32':
      return `Windows ${release} (${arch})`;
    case 'linux':
      return `Linux ${release} (${arch})`;
    default:
      return `${platform} ${release} (${arch})`;
  }
}

/**
 * The connect contract's fields as one JSON object, the same object the
 * Android app assembles (`WarrenSupportReporterImpl.reportJson`) and the
 * shared vector pins: the daemon adds the log field and serialises. Blank
 * steps are no field at all; the facts are clipped to the broker's caps and
 * the locale to the shape it accepts, since a 422 on a fact the reporter
 * never typed would lose the whole report.
 */
export function buildForumReportJson(form: IForumReportForm, facts: ForumReportFacts): string {
  const fields: Record<string, string> = {
    platform: facts.platform,
    what_happened: form.whatHappened.trim(),
    app_version: clipFact(facts.appVersion),
    os_version: clipFact(facts.osVersion),
  };
  // Left out rather than guessed when the reporter did not pick them: the
  // support team reads a value here as the reporter's own answer.
  if (form.area !== undefined) {
    fields.area = form.area;
  }
  if (form.frequency !== undefined) {
    fields.frequency = form.frequency;
  }
  const steps = form.steps?.trim() ?? '';
  if (steps.length > 0) {
    fields.steps = steps;
  }
  const locale = localeForBroker(facts.locale);
  if (locale !== undefined) {
    fields.locale = locale;
  }
  return JSON.stringify(fields);
}

function clipFact(fact: string): string {
  return Array.from(fact).slice(0, MAX_FACT_CHARS).join('');
}

// Letters and dashes only, 2 to 10 of them (the broker's `validate`): a
// system locale can read `zh_Hans-CN.UTF-8`, which the broker refuses whole.
function localeForBroker(locale: string): string | undefined {
  const cleaned = locale.replace(/[^A-Za-z-]/g, '').slice(0, MAX_LOCALE_CHARS);
  return cleaned.length >= MIN_LOCALE_CHARS ? cleaned : undefined;
}

/**
 * The total deadline of a report upload, from the body it sends: 20 s for
 * the exchange itself plus 10 s per MiB of body started, the Android rule
 * (`warren_jni::forum::upload_deadline`). A no-log report keeps roughly a
 * mint's bound; a report at the log cap gets a little over two minutes, a
 * floor of about 0.8 Mbit/s on the uplink a report is filed from.
 */
export function uploadDeadlineMs(bodyBytes: number): number {
  const mib = 1024 * 1024;
  return (20 + 10 * Math.ceil(bodyBytes / mib)) * 1000;
}

/**
 * The name a saved copy of the collected report is offered under: the app,
 * then the local date and time of the collection down to the second, so two
 * saves in the same minute do not land on one name and the support staff can
 * see when it was taken. Only digits and dashes: a `:` from an ISO stamp is
 * refused by Windows, and the dialog then opens on a name to fix by hand.
 */
export function reportSaveFileName(now: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  const day = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `warren-report-${day}-${time}.log`;
}

/** The forum origin's host: the only one a topic link is opened on. */
const FORUM_HOST = new URL(urls.forum).host;

// True iff `url` is an https URL whose authority is exactly the forum host:
// no userinfo, no port, no look-alike suffix. The form opens the topic URL
// with one click as a link the app vouched for, so a broker answer steered
// anywhere else must not become that link. The same rule as
// `warren_forum::is_trusted_topic_url`.
function isTrustedTopicUrl(url: string): boolean {
  if (!url.startsWith('https://')) {
    return false;
  }
  const authority = url.slice('https://'.length).split(/[/?#]/, 1)[0];
  if (authority.length === 0 || authority.includes('@') || authority.includes(':')) {
    return false;
  }
  return authority.toLowerCase() === FORUM_HOST.toLowerCase();
}

const CONNECT_HOST = ALLOWED_CONNECT_HOSTS[0];

// True iff `url` is the broker's one-shot sign-in page on the connect host,
// carrying its ticket in the fragment: the app opens it in the browser, where
// it signs the person in, so nothing else may ride on that click.
function isTrustedReportLoginUrl(url: string): boolean {
  const prefix = `https://${CONNECT_HOST}/report-login#`;
  return url.startsWith(prefix) && url.length > prefix.length;
}

/**
 * Maps the broker's answer to the outcome: the table
 * `fixtures/client-rules/forum_outcomes.json` pins for every client, the same
 * one as `warren_forum::report_outcome_for_response`.
 */
export function forumReportResultForResponse(status: number, bodyText: string): ForumReportResult {
  if (status >= 200 && status < 300) {
    let body: unknown;
    try {
      body = JSON.parse(bodyText);
    } catch {
      body = undefined;
    }
    const record =
      typeof body === 'object' && body !== null ? (body as Record<string, unknown>) : {};
    const topicId = record['topic_id'];
    if (typeof topicId !== 'number' || !Number.isSafeInteger(topicId) || topicId < 0) {
      return { kind: 'failed', reason: `http-${status}` };
    }
    const topicUrl = record['topic_url'];
    const loginUrl = record['login_url'];
    const logs = record['logs'];
    const identity: ForumIdentity | undefined = parseForumIdentityResponse(body);
    return {
      kind: 'created',
      topicId,
      ...(typeof topicUrl === 'string' && isTrustedTopicUrl(topicUrl) ? { topicUrl } : {}),
      logs: logs === 'attached' || logs === 'partial' ? (logs as ForumReportLogs) : 'none',
      ...(identity ? { identity } : {}),
      ...(typeof loginUrl === 'string' && isTrustedReportLoginUrl(loginUrl) ? { loginUrl } : {}),
    };
  }
  switch (status) {
    case 403:
      return { kind: 'subscription-required' };
    case 401:
      return bodyText.includes('"error":"clock_skew"')
        ? { kind: 'clock-skew' }
        : { kind: 'failed', reason: 'http-401' };
    case 429:
      return { kind: 'rate-limited' };
    case 413:
      return { kind: 'too-large' };
    case 422:
      return { kind: 'invalid' };
    default:
      return status >= 500 && status < 600
        ? { kind: 'server-error' }
        : { kind: 'failed', reason: `http-${status}` };
  }
}

/**
 * Signs and sends one report: the daemon signs the body, main POSTs it
 * verbatim to the allowlisted broker under the body-sized deadline. Never
 * throws (logged instead), and never logs the pubkey, the signature, the
 * report text or the log content: only the class of what happened.
 */
export async function sendForumReport(
  form: IForumReportForm,
  logGz: Uint8Array | undefined,
  facts: ForumReportFacts,
  daemonRpc: DaemonRpc,
): Promise<ForumReportResult> {
  if (logGz !== undefined && logGz.byteLength > MAX_LOG_GZ_BYTES) {
    // The broker's 413 is a round trip away with 16 MB of body; the daemon
    // and the shared crate guard too, this is the leg that spends nothing.
    log.warn(`Forum report: gzipped report exceeds the ${MAX_LOG_GZ_BYTES} byte cap, not sent`);
    return { kind: 'too-large' };
  }
  const reportJson = buildForumReportJson(form, facts);

  let signature: ForumLoginSignature;
  try {
    signature = await daemonRpc.signForumReport(reportJson, logGz ?? new Uint8Array(0));
  } catch (error) {
    const code = (error as { code?: number }).code;
    if (code === grpcStatus.FAILED_PRECONDITION) {
      log.info('Forum report: no Warren identity yet, nothing to sign the report with');
      return { kind: 'no-identity' };
    }
    if (code === grpcStatus.INVALID_ARGUMENT) {
      log.warn(`Forum report: the daemon refused the fields: ${String(error)}`);
      return { kind: 'invalid' };
    }
    log.error(`Forum report: daemon could not sign: ${String(error)}`);
    return { kind: 'failed', reason: 'build' };
  }

  const answer = await postSigned(
    '/v1/forum/report',
    signature,
    logGz !== undefined,
    'Forum report',
  );
  if (answer.kind === 'answered') {
    return forumReportResultForResponse(answer.status, answer.bodyText);
  }
  return { kind: 'failed', reason: answer.kind };
}

/**
 * Maps the broker's answer to new logs for a topic. Only the 403 carrying
 * `not_author` is the topic question; any other 403 is the paywall gate.
 */
export function forumReportLogsResultForResponse(
  status: number,
  bodyText: string,
): ForumReportLogsResult {
  if (status >= 200 && status < 300) {
    return { kind: 'attached' };
  }
  switch (status) {
    case 401:
      return bodyText.includes('"error":"clock_skew"')
        ? { kind: 'clock-skew' }
        : { kind: 'failed', reason: 'http-401' };
    case 403:
      return bodyText.includes('"error":"not_author"')
        ? { kind: 'not-author' }
        : { kind: 'subscription-required' };
    case 409:
      return bodyText.includes('"error":"topic_closed"')
        ? { kind: 'topic-closed' }
        : { kind: 'failed', reason: 'http-409' };
    case 413:
      return { kind: 'too-large' };
    case 429:
      return { kind: 'rate-limited' };
    default:
      return status >= 500 && status < 600
        ? { kind: 'server-error' }
        : { kind: 'failed', reason: `http-${status}` };
  }
}

/**
 * Signs and sends new logs for one of the reporter's own topics. Never
 * throws, and logs neither the topic, the pubkey, the signature nor the log:
 * a topic id next to this machine's activity is the link the forum handle
 * exists to break.
 */
export async function sendForumReportLogs(
  topicId: number,
  logGz: Uint8Array,
  daemonRpc: DaemonRpc,
): Promise<ForumReportLogsResult> {
  if (logGz.byteLength > MAX_LOG_GZ_BYTES) {
    log.warn(
      `Forum report logs: gzipped report exceeds the ${MAX_LOG_GZ_BYTES} byte cap, not sent`,
    );
    return { kind: 'too-large' };
  }
  let signature: ForumLoginSignature;
  try {
    signature = await daemonRpc.signForumReportLogs(topicId, logGz);
  } catch (error) {
    const code = (error as { code?: number }).code;
    if (code === grpcStatus.FAILED_PRECONDITION) {
      log.info('Forum report logs: no Warren identity yet, nothing to sign with');
      return { kind: 'no-identity' };
    }
    log.error(`Forum report logs: daemon could not sign (code ${code ?? 'none'})`);
    return { kind: 'failed', reason: 'build' };
  }
  const answer = await postSigned('/v1/forum/report/logs', signature, true, 'Forum report logs');
  if (answer.kind === 'answered') {
    return forumReportLogsResultForResponse(answer.status, answer.bodyText);
  }
  return { kind: 'failed', reason: answer.kind };
}

type PostAnswer =
  | { kind: 'answered'; status: number; bodyText: string }
  | { kind: 'transport' }
  | { kind: 'upload-timeout' };

/**
 * POSTs a daemon-signed body verbatim to `path` on the allowlisted broker,
 * under a deadline sized to the body. A deadline run out while logs were
 * attached is the uplink, which the forms offer to work around, so it is
 * named apart from a plain transport failure.
 */
async function postSigned(
  path: string,
  signature: ForumLoginSignature,
  carriesLogs: boolean,
  what: string,
): Promise<PostAnswer> {
  const bodyBytes = Buffer.byteLength(signature.body);
  const deadline = uploadDeadlineMs(bodyBytes);
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), deadline);
  const started = Date.now();
  try {
    const response = await getForumSession().fetch(`https://${CONNECT_HOST}${path}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-Warren-PubKey': signature.pubkeySs58,
        'X-Warren-Sig': signature.signatureHex,
        'X-Warren-Timestamp': String(signature.timestamp),
        'X-Warren-Nonce': signature.nonceHex,
      },
      // Sent verbatim: the daemon signed exactly these bytes.
      body: signature.body,
      signal: abort.signal,
    });
    // The deadline bounds the UPLOAD, and the upload is over the moment the
    // broker answers. Leaving it armed over the body read cut a created topic
    // into an `http-201` failure whenever the answer started just under the
    // deadline, and the reporter then filed the same report, and the same
    // logs, a second time.
    clearTimeout(timer);
    const bodyText = await response.text().catch(() => '');
    log.info(
      `${what}: broker answered ${response.status} in ${Date.now() - started} ms for a ${bodyBytes} byte body`,
    );
    return { kind: 'answered', status: response.status, bodyText };
  } catch {
    // The cause is not logged: a transport error can quote the request, and
    // the class is all the log needs.
    const timedOut = abort.signal.aborted;
    log.warn(
      `${what}: POST to the broker failed (${timedOut ? 'deadline' : 'transport'}) after ${Date.now() - started} ms of a ${deadline / 1000} s deadline for a ${bodyBytes} byte body`,
    );
    return timedOut && carriesLogs ? { kind: 'upload-timeout' } : { kind: 'transport' };
  } finally {
    clearTimeout(timer);
  }
}
