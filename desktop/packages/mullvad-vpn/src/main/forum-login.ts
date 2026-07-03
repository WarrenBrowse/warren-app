import { Session, session } from 'electron';

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
 * Drives the full forum-login exchange for a validated deep link: ask the
 * daemon to sign, then POST to the connect host. Resolves `true` on a 2xx
 * from the provider, `false` otherwise. Never throws (logged instead) so a
 * malformed link cannot crash the main process.
 */
export async function performForumLogin(rawUrl: string, daemonRpc: DaemonRpc): Promise<boolean> {
  const parsed = parseForumLoginUrl(rawUrl);
  if (!parsed) {
    log.warn('Ignoring malformed or non-allowlisted forum-login deep link');
    return false;
  }

  let signature;
  try {
    signature = await daemonRpc.signForumLogin(parsed.sid);
  } catch (error) {
    log.error(`Forum login: daemon could not sign (identity bootstrapped?): ${String(error)}`);
    return false;
  }

  try {
    const response = await getForumSession().fetch(`https://${parsed.host}/v1/forum/login`, {
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
    if (!response.ok) {
      log.error(`Forum login: provider returned HTTP ${response.status}`);
      return false;
    }
    log.info('Forum login: signed challenge accepted by the provider');
    return true;
  } catch (error) {
    // Never log the sid/pubkey/signature (no-log policy).
    log.error(`Forum login: POST to connect host failed: ${String(error)}`);
    return false;
  }
}
