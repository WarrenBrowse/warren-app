import { useSelector } from '../redux/store';

// The forum activity badge.
//
// The number is computed in the main process, from the broadcast digest
// matched against this installation's slot, corrected by whatever the app
// last observed for itself when the panel was read or the list marked seen.
// Computed there rather than here so the bell, the tray dot and the desktop
// banner cannot drift apart, and so the badge is right the moment the user
// acts instead of when the digest next catches up.

/** True once the user has signed in to the forum at least once. */
export function useHasForumAccount(): boolean {
  return useSelector((state) => state.account.forumIdentity !== undefined);
}

/**
 * Unread forum notifications for this installation. Zero whenever there
 * is nothing to show, including while the daemon holds no fresh document,
 * which is how the badge clears.
 */
export function useForumUnreadCount(): number {
  return useSelector((state) => state.account.forumUnread);
}
