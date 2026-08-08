import { useCallback, useEffect, useState } from 'react';

import { ForumNotification } from '../../../../shared/forum-notifications';

// IPC goes through the contextBridge-exposed `window.ipc`, never by importing
// `lib/ipc-event-channel`: that module imports the `electron` package, which
// Vite pre-bundles into the sandboxed, node-less renderer, and the bundle then
// throws `__dirname is not defined` on load. That kills the React app before it
// mounts, so the window has nothing to paint and clicking the tray icon appears
// to do nothing (shipped in 1.1.5; the same trap is documented in
// `features/forum-login/components/ForumLoginPrompt.tsx`).

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
    void window.ipc.forumActivity.list().then((result) => {
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
