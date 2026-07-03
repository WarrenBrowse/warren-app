import { Session, session } from 'electron';

import { ForumLoginResult, IForumLoginRequest } from '../shared/forum-login';
import log from '../shared/logging';
import { DaemonRpc } from './daemon-rpc';

/**
 * Community-forum wallet login (warren-core doc 55).
 *
 * The forum (`forum.warrenbrowse.com`) authenticates users through
 * DiscourseConnect against our `warren-forum-auth` provider. When the user
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

export const FORUM_DEEP_LINK_SCHEME = 'warren';

// The forum-login POST must NOT ride `session.defaultSession`: that session is
// hardened to cancel every outbound web request (the renderer never phones
// home). This is a deliberate, user-initiated request to our own connect host,
// so it uses an isolated session that the default block filter does not touch.
let forumSession: Session | undefined;
function getForumSession(): Session {
  forumSession ??= session.fromPartition('warren-forum-login');
  return forumSession;
}

/** The host we accept in the deep link. Hard allowlist: a hostile link must
 * not be able to point a signed request at an attacker-controlled server. */
const ALLOWED_CONNECT_HOSTS = ['connect.warrenbrowse.com'];

interface ParsedForumLogin {
  sid: string;
  host: string;
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
  return { sid, host };
}

/**
 * Finds the first `warren://forum-login` URL in a process argv list. Windows
 * and Linux deliver the deep link as a command-line argument (to the initial
 * process or, for a running instance, via the `second-instance` event).
 */
export function findForumLoginArg(argv: readonly string[]): string | undefined {
  return argv.find((arg) => arg.startsWith(`${FORUM_DEEP_LINK_SCHEME}://forum-login`));
}

/**
 * Signs and submits an APPROVED forum login: ask the daemon to sign, then POST
 * to the connect host. Called only after the user approves the consent prompt.
 * Never throws (logged instead) so a failure cannot crash the main process.
 */
export async function approveForumLogin(
  request: IForumLoginRequest,
  daemonRpc: DaemonRpc,
): Promise<ForumLoginResult> {
  // Re-validate host + sid main-side: the renderer is trusted, but this is the
  // security boundary that signs with the wallet key.
  if (!ALLOWED_CONNECT_HOSTS.includes(request.host) || !/^[0-9a-f]{32}$/.test(request.sid)) {
    log.warn('Refusing forum login: host not allowlisted or malformed sid');
    return 'error';
  }

  let signature;
  try {
    signature = await daemonRpc.signForumLogin(request.sid);
  } catch (error) {
    log.error(`Forum login: daemon could not sign (identity bootstrapped?): ${String(error)}`);
    return 'error';
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
    if (response.status === 403) {
      log.info('Forum login: refused (wallet has never subscribed)');
      return 'subscription-required';
    }
    if (!response.ok) {
      log.error(`Forum login: provider returned HTTP ${response.status}`);
      return 'error';
    }
    log.info('Forum login: signed challenge accepted by the provider');
    return 'approved';
  } catch (error) {
    // Never log the sid/pubkey/signature (no-log policy).
    log.error(`Forum login: POST to connect host failed: ${String(error)}`);
    return 'error';
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
