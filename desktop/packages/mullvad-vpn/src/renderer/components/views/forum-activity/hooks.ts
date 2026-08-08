import { useCallback, useEffect, useState } from 'react';

import { ForumNotification } from '../../../../shared/forum-notifications';
import { IpcRendererEventChannel } from '../../../lib/ipc-event-channel';

export type ForumActivityState =
  | { status: 'loading' }
  | { status: 'ready'; notifications: ForumNotification[] }
  | { status: 'error' };

/**
 * Reads the user's own forum notifications once, when the panel opens.
 *
 * Deliberately not a subscription and not a poll: the header badge already
 * comes from the broadcast digest, which asks the server nothing about
 * anybody. This is the only request tied to the account, and it happens
 * only because the user asked to see the content.
 */
export function useForumActivity(): { state: ForumActivityState; reload: () => void } {
  const [state, setState] = useState<ForumActivityState>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setState({ status: 'loading' });
    void IpcRendererEventChannel.forumActivity.list().then((result) => {
      if (cancelled) {
        return;
      }
      setState(
        result.result === 'ok'
          ? { status: 'ready', notifications: result.notifications }
          : { status: 'error' },
      );
    });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  const reload = useCallback(() => setAttempt((n) => n + 1), []);
  return { state, reload };
}
