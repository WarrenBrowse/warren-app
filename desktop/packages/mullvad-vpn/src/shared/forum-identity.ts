// The user's community-forum identity, and how a broadcast activity
// digest becomes one user's badge.
//
// Both values are minted server side and learned only from an approved
// login: the handle is derived under a secret this app does not hold, and
// the digest slot is drawn at random so that the published document stays
// anonymous. Keeping the slot here, next to the handle, is what lets the
// app read its own activity out of a document identical for every client,
// without ever asking the server about this account.
//
// Deliberately free of any electron import: the codec is the part worth
// testing, and it must not need a window to run.

/** Frozen shape of a forum handle: three proquint quints. */
export const FORUM_HANDLE_PATTERN = /^[a-z]{5}-[a-z]{5}-[a-z]{5}$/;

/** Highest count one digest character can carry, above which it saturates. */
export const UNREAD_SATURATED = 15;

export interface ForumIdentity {
  handle: string;
  // `null` for an account that logged in before slots existed, or whose
  // login found no room. It costs the badge, nothing else.
  notifySlot: number | null;
}

/** Serializes an identity for the sealed on-disk store. */
export function serializeForumIdentity(identity: ForumIdentity): string {
  return JSON.stringify({ handle: identity.handle, notifySlot: identity.notifySlot });
}

/**
 * Reads back a sealed identity blob.
 *
 * A blob sealed under another keychain state decrypts to garbage, so
 * nothing is trusted without the handle passing its frozen shape gate.
 * A bare handle is accepted as well: that is what the store wrote before
 * slots existed, and rejecting it would log the user out of their own
 * forum name for no reason.
 */
export function parseForumIdentity(blob: string): ForumIdentity | undefined {
  const trimmed = blob.trim();
  if (FORUM_HANDLE_PATTERN.test(trimmed)) {
    return { handle: trimmed, notifySlot: null };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return undefined;
  }
  if (typeof parsed !== 'object' || parsed === null) {
    return undefined;
  }
  const { handle, notifySlot } = parsed as { handle?: unknown; notifySlot?: unknown };
  if (typeof handle !== 'string' || !FORUM_HANDLE_PATTERN.test(handle)) {
    return undefined;
  }
  return { handle, notifySlot: isSlot(notifySlot) ? notifySlot : null };
}

/**
 * Whether the app shows forum ACTIVITY at all: the header bell, the desktop
 * banner and the mark on the tray icon alike.
 *
 * One rule for the three, so none of them can be left behind the others.
 * Off means off everywhere: a bell surviving the setting would keep offering
 * the activity the user just asked not to be shown, and a bell shown to a
 * wallet with no forum account would invite a click into a login flow nobody
 * asked for.
 *
 * What the header puts in the bell's place for that accountless wallet is
 * `forumHeaderButton`, below. The tray and the banner have no equivalent:
 * they report activity, and there is none to report without an account.
 */
export function showsForumActivity(hasAccount: boolean, enabled: boolean): boolean {
  return hasAccount && enabled;
}

/** What the header's forum slot carries: the bell, the lifebuoy, or nothing. */
export type ForumHeaderButton = 'activity' | 'community' | 'none';

/**
 * Which button the header's forum slot shows.
 *
 * A wallet with no forum account gets a lifebuoy straight to the forum rather
 * than an empty slot: the bell would be inert for them, but the forum is the
 * one thing they might actually want, and it is where an account comes from.
 * The setting still governs the whole slot, lifebuoy included, so a user who
 * turned the forum off gets no forum in their header either.
 */
export function forumHeaderButton(hasAccount: boolean, enabled: boolean): ForumHeaderButton {
  if (!enabled) {
    return 'none';
  }
  return hasAccount ? 'activity' : 'community';
}

/** True for a value usable as a digest slot index. */
export function isSlot(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0;
}

/**
 * Unread count this installation's slot carries in `digest`.
 *
 * Zero whenever there is nothing to show: no fresh document, no slot yet,
 * or a slot past what the server has published (the normal state of an
 * account that registered since the last rebuild). The daemon has already
 * checked the signature and the freshness, so this only indexes.
 */
export function unreadForSlot(digest: string | null | undefined, slot: number | null): number {
  if (!digest || !isSlot(slot) || slot >= digest.length) {
    return 0;
  }
  const count = parseInt(digest[slot], 16);
  return Number.isNaN(count) ? 0 : count;
}
