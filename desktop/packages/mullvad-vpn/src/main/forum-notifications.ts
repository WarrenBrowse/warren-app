import { ForumNotificationsResult, parseForumNotifications } from '../shared/forum-notifications';
import log from '../shared/logging';
import { DaemonRpc } from './daemon-rpc';
import { ALLOWED_CONNECT_HOSTS, getForumSession } from './forum-login';

// Reading the user's own forum notifications, for the activity panel.
//
// Called when the user opens the panel, never on a timer. The badge itself
// comes from the broadcast digest, which asks the server nothing about
// anybody, so this is the only request tied to an account and it happens
// only when the user asks to see the content.
//
// The account read is derived from the signature: the request names no
// account, and the daemon builds the signed body itself, so there is
// nothing here that could be pointed at somebody else.

export async function fetchForumNotifications(
  daemonRpc: DaemonRpc,
): Promise<ForumNotificationsResult> {
  let signature;
  try {
    signature = await daemonRpc.signForumNotifications();
  } catch (error) {
    log.error(`Forum notifications: daemon could not sign: ${String(error)}`);
    return { result: 'error' };
  }

  const host = ALLOWED_CONNECT_HOSTS[0];
  try {
    const response = await getForumSession().fetch(`https://${host}/v1/forum/notifications`, {
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
      log.error(`Forum notifications: provider returned HTTP ${response.status}`);
      return { result: 'error' };
    }
    return { result: 'ok', notifications: parseForumNotifications(await response.json()) };
  } catch (error) {
    // Never log the pubkey or the signature (no-log policy), and never the
    // response body: it carries the user's own forum content.
    log.error(`Forum notifications: request to the connect host failed: ${String(error)}`);
    return { result: 'error' };
  }
}
