import { Session, session } from 'electron';

import { productAnchors } from '../shared/constants/product-env';
import { ForumIdentity, isSlot } from '../shared/forum-identity';
import { ForumLoginResult, IForumLoginRequest } from '../shared/forum-login';
import log from '../shared/logging';
import { DaemonRpc } from './daemon-rpc';

/**
 * Community-forum wallet login (warren-core doc 55).
 *
 * The forum (`forum.warrenbrowse.com`) authenticates users through
 * DiscourseConnect against our `warren-connect` provider. When the user
 * signs in on the forum, the provider shows an approval page carrying a
 * `warren://forum-login?sid=..&host=..` deep link. The OS hands that link to
 * this app; we ask the daemon to sign the fixed `POST /v1/forum/login`
 * challenge with the Warren identity key, then POST it to the connect host.
 * The browser's approval page (polling) then completes the login.
 *
 * The signing key never leaves the daemon; the renderer/main only ferry the
 * opaque `sid` and the resulting signature headers (which are not long-lived
 * secrets: bound to the fixed path + body + nonce + 60 s timestamp window,
 * single-use server side).
 */

// Per product environment (warren / warren-staging / warren-beta) so a beta
// install never steals the prod install's deep links, or vice versa.
export const FORUM_DEEP_LINK_SCHEME = productAnchors.deepLinkScheme;

// The forum POSTs must NOT ride `session.defaultSession`: that session is
// hardened to cancel every outbound web request (the renderer never phones
// home). These are deliberate, user-initiated requests to our own connect
// host, so they use an isolated session the default block filter does not
// touch. Shared with the attach-logs flow (forum-attach.ts).
let forumSession: Session | undefined;
export function getForumSession(): Session {
  forumSession ??= session.fromPartition('warren-forum-login');
  return forumSession;
}

/** The host we accept in a deep link. Hard allowlist: a hostile link must
 * not be able to point a signed request at an attacker-controlled server. */
export const ALLOWED_CONNECT_HOSTS = ['connect.warrenbrowse.com'];

interface ParsedForumLogin {
  sid: string;
  host: string;
  crossDevice: boolean;
}

/**
 * Parses and validates a `warren://forum-login?sid=..&host=..` URL. Returns
 * `undefined` for any URL that is not a well-formed, allowlisted forum-login
 * link (wrong scheme, wrong action, bad sid, non-allowlisted host).
 */
export function parseForumLoginUrl(rawUrl: string): ParsedForumLogin | undefined {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return undefined;
  }
  if (url.protocol !== `${FORUM_DEEP_LINK_SCHEME}:`) {
    return undefined;
  }
  // `warren://forum-login?...` parses with host = "forum-login" (no path).
  const action = url.hostname || url.pathname.replace(/^\/+/, '');
  if (action !== 'forum-login') {
    return undefined;
  }
  const sid = url.searchParams.get('sid') ?? '';
  const host = url.searchParams.get('host') ?? '';
  // The sid is interpolated into a signed JSON body server side; pin it to the
  // exact Discourse nonce shape (32 lowercase hex). The daemon re-validates.
  if (!/^[0-9a-f]{32}$/.test(sid)) {
    return undefined;
  }
  if (!ALLOWED_CONNECT_HOSTS.includes(host)) {
    return undefined;
  }
  // The provider sets `xd=1` on the QR's link only. Anything else, including
  // an absent parameter, is the same-device button, so an older provider that
  // does not send it degrades to the ordinary prompt rather than to a warning
  // nobody can act on.
  return { sid, host, crossDevice: url.searchParams.get('xd') === '1' };
}

/**
 * Finds the first `warren://` forum deep link (forum-login or attach-logs) in
 * a process argv list. Windows and Linux deliver a deep link as a
 * command-line argument (to the initial process or, for a running instance,
 * via the `second-instance` event).
 */
export function findForumDeepLinkArg(argv: readonly string[]): string | undefined {
  const prefixes = ['forum-login', 'attach-logs'].map(
    (action) => `${FORUM_DEEP_LINK_SCHEME}://${action}`,
  );
  return argv.find((arg) => prefixes.some((prefix) => arg.startsWith(prefix)));
}

/**
 * Pulls the derived forum handle out of an approved-login response. The
 * derivation is keyed by a server-side secret, so this response is the only
 * way the app can know the user's forum name.
 *
 * The value is persisted and rendered, so it is validated against the frozen
 * three-quint proquint shape rather than trusted: anything else is dropped and
 * the account view simply shows no handle. A provider that predates the field
 * yields `undefined`, which keeps the login itself working.
 */
export function parseForumHandle(body: unknown): string | undefined {
  if (typeof body !== 'object' || body === null) {
    return undefined;
  }
  const handle = (body as { handle?: unknown }).handle;
  if (typeof handle !== 'string' || !/^[a-z]{5}-[a-z]{5}-[a-z]{5}$/.test(handle)) {
    return undefined;
  }
  return handle;
}

/**
 * Pulls the digest slot out of an approved-login response.
 *
 * The slot is drawn server side so the published activity document stays
 * anonymous, which makes this response the only way the app can learn
 * which position of it is its own. Absent when the allocator had no room,
 * or when the provider predates the field: both cost the badge and
 * nothing else, so neither may fail a login the provider accepted.
 */
export function parseForumNotifySlot(body: unknown): number | undefined {
  if (typeof body !== 'object' || body === null) {
    return undefined;
  }
  // Indexed rather than declared as a property: the key is the server's
  // wire spelling, which is snake_case and must not be renamed here.
  const slot = (body as Record<string, unknown>)['notify_slot'];
  return isSlot(slot) ? slot : undefined;
}

/** Handle plus slot, as far as the provider's answer allows. */
export function parseForumIdentityResponse(body: unknown): ForumIdentity | undefined {
  const handle = parseForumHandle(body);
  if (handle === undefined) {
    return undefined;
  }
  return { handle, notifySlot: parseForumNotifySlot(body) ?? null };
}

// A request buffered past the life of the session it names could only produce
// a doomed signature, and connect gives its two session kinds different lives:
// 5 minutes for a login, 30 for an attach (the attach one covers a human
// reading the consent prompt). One shared value was wrong for both.
export const PENDING_LOGIN_MAX_AGE_MS = 5 * 60 * 1000;
export const PENDING_ATTACH_MAX_AGE_MS = 30 * 60 * 1000;

/**
 * Holds the latest unanswered forum request (login or attach-logs) so it
 * survives the window not existing yet. A deep link that cold-starts the app
 * arrives before the renderer has subscribed to the request push; the
 * renderer fetches this buffer when the prompt mounts instead. The request
 * stays buffered until approved/cancelled so reopening the window re-shows an
 * unanswered prompt.
 */
export class PendingForumRequest<T> {
  private request?: T;
  private receivedAt = 0;

  public constructor(private readonly maxAgeMs: number) {}

  public set(request: T, now: number): void {
    this.request = request;
    this.receivedAt = now;
  }

  public get(now: number): T | undefined {
    if (this.request && now - this.receivedAt > this.maxAgeMs) {
      this.clear();
    }
    return this.request;
  }

  public clear(): void {
    this.request = undefined;
    this.receivedAt = 0;
  }
}

/**
 * Maps the provider's HTTP response to the login result. The clock_skew token
 * is a frozen wire detail (warren-connect's sso_flow test asserts the exact
 * response bytes `{"error":"clock_skew"}`) and is only honored on a 401, where
 * it is the auth verifier speaking. A 404 is the session store speaking: the
 * sid is unknown (expired, cancelled or already consumed), which no retry on
 * the same link can change. The table is pinned by
 * `fixtures/client-rules/forum_outcomes.json`, shared with the Rust crate.
 */
export function resultForProviderResponse(status: number, bodyText: string): ForumLoginResult {
  if (status >= 200 && status < 300) {
    return 'approved';
  }
  if (status === 403) {
    return 'subscription-required';
  }
  if (status === 401 && bodyText.includes('"error":"clock_skew"')) {
    return 'clock-skew';
  }
  if (status === 404) {
    return 'expired';
  }
  return 'error';
}

/** Outcome of an approved login, plus the identity the provider echoed back. */
export interface ApprovedForumLogin {
  result: ForumLoginResult;
  identity?: ForumIdentity;
}

/**
 * Signs and submits an APPROVED forum login: ask the daemon to sign, then POST
 * to the connect host. Called only after the user approves the consent prompt.
 * Never throws (logged instead) so a failure cannot crash the main process.
 */
export async function approveForumLogin(
  request: IForumLoginRequest,
  daemonRpc: DaemonRpc,
): Promise<ApprovedForumLogin> {
  // Re-validate host + sid main-side: the renderer is trusted, but this is the
  // security boundary that signs with the wallet key.
  if (!ALLOWED_CONNECT_HOSTS.includes(request.host) || !/^[0-9a-f]{32}$/.test(request.sid)) {
    log.warn('Refusing forum login: host not allowlisted or malformed sid');
    return { result: 'error' };
  }

  let signature;
  try {
    signature = await daemonRpc.signForumLogin(request.sid);
  } catch (error) {
    log.error(`Forum login: daemon could not sign (identity bootstrapped?): ${String(error)}`);
    return { result: 'error' };
  }

  try {
    const response = await getForumSession().fetch(`https://${request.host}/v1/forum/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-Warren-PubKey': signature.pubkeySs58,
        'X-Warren-Sig': signature.signatureHex,
        'X-Warren-Timestamp': String(signature.timestamp),
        'X-Warren-Nonce': signature.nonceHex,
      },
      body: signature.body,
    });
    const bodyText = await response.text().catch(() => '');
    const result = resultForProviderResponse(response.status, bodyText);
    if (result === 'subscription-required') {
      log.info('Forum login: refused (wallet has never subscribed)');
      return { result };
    }
    if (result === 'clock-skew') {
      // A duration-class cause, no identity material: safe to name in the log.
      log.info("Forum login: refused (this machine's clock is outside the accepted window)");
      return { result };
    }
    if (result === 'expired') {
      log.info('Forum login: the provider no longer knows this session (expired or cancelled)');
      return { result };
    }
    if (result === 'error') {
      log.error(`Forum login: provider returned HTTP ${response.status}`);
      return { result };
    }
    log.info('Forum login: signed challenge accepted by the provider');
    // A malformed body must not fail a login the provider already accepted:
    // the handle is a display convenience and the slot only drives a badge,
    // while the login itself is done.
    let identity: ForumIdentity | undefined;
    try {
      identity = parseForumIdentityResponse(JSON.parse(bodyText));
    } catch {
      identity = undefined;
    }
    return { result: 'approved', identity };
  } catch (error) {
    // Never log the sid/pubkey/signature (no-log policy).
    log.error(`Forum login: POST to connect host failed: ${String(error)}`);
    return { result: 'error' };
  }
}

/**
 * Tells the connect provider the user declined, so the waiting browser page
 * stops polling and shows a "cancelled" message. Best-effort, never throws.
 */
export async function cancelForumLogin(request: IForumLoginRequest): Promise<void> {
  if (!ALLOWED_CONNECT_HOSTS.includes(request.host) || !/^[0-9a-f]{32}$/.test(request.sid)) {
    return;
  }
  try {
    await getForumSession().fetch(`https://${request.host}/v1/session/${request.sid}/cancel`, {
      method: 'POST',
    });
  } catch (error) {
    log.error(`Forum login: cancel notification failed: ${String(error)}`);
  }
}
