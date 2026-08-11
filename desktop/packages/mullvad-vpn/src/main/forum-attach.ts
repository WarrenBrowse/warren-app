import { status as grpcStatus } from '@grpc/grpc-js';

import { ForumAttachResult, IForumAttachRequest } from '../shared/forum-attach';
import log from '../shared/logging';
import { DaemonRpc } from './daemon-rpc';
import { ALLOWED_CONNECT_HOSTS, FORUM_DEEP_LINK_SCHEME, getForumSession } from './forum-login';

/**
 * Community-forum attach-logs (warren-core doc 55).
 *
 * A forum bug-report page on the connect host opens a
 * `warren://attach-logs?sid=..&topic=..&host=..` deep link. The app collects
 * the redacted problem report, shows a consent prompt (the user can view the
 * exact report first), and only on explicit approval asks the daemon to sign
 * the `POST /v1/forum/attach-logs` request with the Warren identity key and
 * POSTs it to the connect host. The daemon builds the JSON body itself so the
 * signed bytes and the sent bytes are identical.
 */

/** Client-side cap on the gzipped report: the server rejects bigger uploads
 * (413), so a larger payload could only produce a doomed request.
 *
 * Must track warren-connect's `MAX_LOG_GZ_B64_CHARS` (16 M base64 characters,
 * about 12 MiB of gzip since base64 adds a third). This is the FIRST leg of a
 * four-leg chain, and the one that silently wins: raising the server caps
 * without this one changes nothing at all, because the app never sends the
 * bytes. The other legs are the route's body limit, the decompressed cap, and
 * Discourse's `max_attachment_size_kb`. */
export const MAX_LOG_GZ_BYTES = 12 * 1024 * 1024;

export interface ParsedForumAttach {
  sid: string;
  host: string;
  topicId: number;
}

/**
 * Parses and validates a `warren://attach-logs?sid=..&topic=..&host=..` URL.
 * Returns `undefined` for any URL that is not a well-formed, allowlisted
 * attach-logs link (wrong scheme, wrong action, bad sid, bad topic,
 * non-allowlisted host).
 */
export function parseForumAttachUrl(rawUrl: string): ParsedForumAttach | undefined {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return undefined;
  }
  if (url.protocol !== `${FORUM_DEEP_LINK_SCHEME}:`) {
    return undefined;
  }
  // `warren://attach-logs?...` parses with host = "attach-logs" (no path).
  const action = url.hostname || url.pathname.replace(/^\/+/, '');
  if (action !== 'attach-logs') {
    return undefined;
  }
  const sid = url.searchParams.get('sid') ?? '';
  const host = url.searchParams.get('host') ?? '';
  const topic = url.searchParams.get('topic') ?? '';
  // The sid is interpolated into a signed JSON body daemon side; pin it to
  // the exact session id shape (32 lowercase hex). The daemon re-validates.
  if (!/^[0-9a-f]{32}$/.test(sid)) {
    return undefined;
  }
  // Decimal-only topic id (the regex already excludes negatives), kept
  // within Number's safe-integer range so the value survives the JS number
  // round trip to the daemon unchanged. Topic 0 is the pre-topic variant:
  // the report is still being composed and the forum binds the logs to the
  // topic after creation.
  if (!/^[0-9]+$/.test(topic)) {
    return undefined;
  }
  const topicId = Number(topic);
  if (!Number.isSafeInteger(topicId)) {
    return undefined;
  }
  if (!ALLOWED_CONNECT_HOSTS.includes(host)) {
    return undefined;
  }
  return { sid, host, topicId };
}

/**
 * Signs and submits an APPROVED attach-logs request: ask the daemon to sign
 * the exact body, then POST it verbatim to the connect host. Called only
 * after the user approves the consent prompt. Never throws (logged instead)
 * so a failure cannot crash the main process.
 */
export async function approveForumAttach(
  request: IForumAttachRequest,
  daemonRpc: DaemonRpc,
  logGz: Uint8Array,
): Promise<ForumAttachResult> {
  // Re-validate main-side: the renderer is trusted, but this is the security
  // boundary that signs with the wallet key.
  if (
    !ALLOWED_CONNECT_HOSTS.includes(request.host) ||
    !/^[0-9a-f]{32}$/.test(request.sid) ||
    !Number.isSafeInteger(request.topicId) ||
    request.topicId < 0
  ) {
    log.warn('Refusing forum attach: host not allowlisted or malformed request');
    return 'error';
  }
  if (logGz.byteLength === 0) {
    log.warn('Refusing forum attach: empty log payload');
    return 'error';
  }
  if (logGz.byteLength > MAX_LOG_GZ_BYTES) {
    log.warn(`Refusing forum attach: gzipped report exceeds the ${MAX_LOG_GZ_BYTES} byte cap`);
    return 'too-large';
  }

  let signature;
  try {
    signature = await daemonRpc.signForumAttachLogs(request.sid, request.topicId, logGz);
  } catch (error) {
    // The daemon answers FAILED_PRECONDITION for one reason on this call:
    // there is no Warren identity to sign with yet.
    if ((error as { code?: number }).code === grpcStatus.FAILED_PRECONDITION) {
      log.info('Forum attach: no Warren identity yet, nothing to sign the report with');
      return 'no-identity';
    }
    log.error(`Forum attach: daemon could not sign: ${String(error)}`);
    return 'error';
  }

  try {
    const response = await getForumSession().fetch(`https://${request.host}/v1/forum/attach-logs`, {
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
    });
    switch (response.status) {
      case 403:
        log.info('Forum attach: refused (wallet is not the topic author)');
        return 'not-author';
      case 404:
        log.info('Forum attach: session expired or unknown');
        return 'expired';
      case 413:
        log.info('Forum attach: provider refused the payload size');
        return 'too-large';
      default:
        break;
    }
    if (response.status >= 500) {
      log.error(`Forum attach: provider failed on its own side, HTTP ${response.status}`);
      return 'server-error';
    }
    if (!response.ok) {
      log.error(`Forum attach: provider returned HTTP ${response.status}`);
      return 'error';
    }
    log.info('Forum attach: logs attached to the report');
    return 'attached';
  } catch (error) {
    // Never log the sid/pubkey/signature/log content (no-log policy).
    log.error(`Forum attach: POST to connect host failed: ${String(error)}`);
    return 'error';
  }
}

/** The report bytes to send plus the id of the file they were read from, so
 * the caller can delete exactly that temp file afterwards (it may differ from
 * the previewed id when a fresh collection was needed). */
export interface ResolvedReport {
  bytes: Buffer;
  reportId: string;
}

/**
 * Resolves the report bytes to send on approval: the report previewed at
 * deep-link time when it still exists, otherwise a fresh collection. Retrying
 * covers both a failed deep-link collection (no previewed id) and a previewed
 * file the OS reaped before approval, so neither turns into a dead prompt.
 */
export async function resolveApprovedReport(
  reportId: string | undefined,
  collect: () => Promise<string>,
  read: (id: string) => Promise<Buffer>,
): Promise<ResolvedReport> {
  if (reportId !== undefined) {
    try {
      return { bytes: await read(reportId), reportId };
    } catch {
      // Previewed file vanished (temp reaper); fall through to re-collect.
    }
  }
  const fresh = await collect();
  return { bytes: await read(fresh), reportId: fresh };
}

/**
 * Tells the connect provider the user declined, so the waiting browser page
 * stops polling and shows a "cancelled" message. Best-effort, never throws.
 */
export async function cancelForumAttach(request: IForumAttachRequest): Promise<void> {
  if (!ALLOWED_CONNECT_HOSTS.includes(request.host) || !/^[0-9a-f]{32}$/.test(request.sid)) {
    return;
  }
  try {
    await getForumSession().fetch(`https://${request.host}/v1/attach/${request.sid}/cancel`, {
      method: 'POST',
    });
  } catch (error) {
    log.error(`Forum attach: cancel notification failed: ${String(error)}`);
  }
}
