import { unreadForSlot } from '../../shared/forum-identity';
import { useSelector } from '../redux/store';

// The forum activity badge, read out of the broadcast digest.
//
// The daemon holds a document identical for every client and has already
// checked its signature and its freshness. Only this process knows which
// slot belongs to this installation, so the count is computed here and
// the server is never asked about this account.

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
  const digest = useSelector((state) => state.settings.warrenStatus?.forumDigest ?? null);
  const slot = useSelector((state) => state.account.forumIdentity?.notifySlot ?? null);
  return unreadForSlot(digest, slot);
}
