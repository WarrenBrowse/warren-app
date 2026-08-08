import { Url, urls } from './constants';
// One row of the forum activity panel, and how a provider answer becomes
// one.
//
// Everything here is untrusted input rendered in the app, so each field is
// validated rather than cast: the provider is ours, but a compromised or
// confused one must not be able to put arbitrary markup, or a link to
// somewhere else entirely, in front of the user.

/** Outcome of one panel read. */
export type ForumNotificationsResult =
  | { result: 'ok'; notifications: ForumNotification[] }
  | { result: 'error' };

/** What happened, as far as the panel needs to distinguish it. */
export type ForumNotificationKind =
  | 'mentioned'
  | 'replied'
  | 'quoted'
  | 'liked'
  | 'private_message'
  | 'posted'
  | 'linked'
  | 'granted_badge'
  | 'watching_first_post'
  | 'other';

const KINDS: readonly ForumNotificationKind[] = [
  'mentioned',
  'replied',
  'quoted',
  'liked',
  'private_message',
  'posted',
  'linked',
  'granted_badge',
  'watching_first_post',
  'other',
];

export interface ForumNotification {
  id: number;
  kind: ForumNotificationKind;
  read: boolean;
  // Unix epoch seconds.
  createdAt: number;
  title?: string;
  actor?: string;
  excerpt?: string;
  // Forum-relative path of the post, e.g. `/t/86/4`. Absent when the
  // notification points at no post (a badge award).
  path?: string;
}

// A path is opened in the user's browser, so it is pinned to the exact
// shape the forum produces rather than trusted: anything else could send
// the user somewhere the app never meant to.
const TOPIC_PATH_PATTERN = /^\/t\/\d+(\/\d+)?$/;

/** Longest excerpt kept. The provider already caps it; this is the guard. */
const MAX_EXCERPT = 400;

/** Longest title or actor kept, so one row cannot flood the panel. */
const MAX_LABEL = 200;

/**
 * Reads the provider's answer into rows, dropping anything malformed
 * rather than rendering it. A body that is not a list at all yields an
 * empty panel, which reads as "nothing here" and never as an error the
 * user can act on.
 */
export function parseForumNotifications(body: unknown): ForumNotification[] {
  if (typeof body !== 'object' || body === null) {
    return [];
  }
  const list = (body as { notifications?: unknown }).notifications;
  if (!Array.isArray(list)) {
    return [];
  }
  return list.flatMap((raw) => {
    const parsed = parseOne(raw);
    return parsed ? [parsed] : [];
  });
}

function parseOne(raw: unknown): ForumNotification | undefined {
  if (typeof raw !== 'object' || raw === null) {
    return undefined;
  }
  const row = raw as Record<string, unknown>;
  const id = row['id'];
  const createdAt = row['created_at'];
  if (typeof id !== 'number' || !Number.isFinite(id)) {
    return undefined;
  }
  if (typeof createdAt !== 'number' || !Number.isFinite(createdAt)) {
    return undefined;
  }
  const path = row['path'];
  return {
    id,
    kind: parseKind(row['kind']),
    read: row['read'] === true,
    createdAt,
    title: text(row['title'], MAX_LABEL),
    actor: text(row['actor'], MAX_LABEL),
    excerpt: text(row['excerpt'], MAX_EXCERPT),
    path: typeof path === 'string' && TOPIC_PATH_PATTERN.test(path) ? path : undefined,
  };
}

function parseKind(value: unknown): ForumNotificationKind {
  return KINDS.includes(value as ForumNotificationKind)
    ? (value as ForumNotificationKind)
    : 'other';
}

/**
 * Browser URL that opens `path` with the reader already signed in.
 *
 * Entering through the forum's own SSO entry rather than the bare post:
 * the forum has no password, so a reader who is not signed in on that
 * browser would land on a page that cannot show them their own
 * notification. The wallet SSO round trip is the existing one.
 */
export function forumPostUrl(path: string): Url {
  return `${urls.forum}session/sso?return_path=${encodeURIComponent(path)}`;
}

function text(value: unknown, max: number): string | undefined {
  if (typeof value !== 'string') {
    return undefined;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed.slice(0, max) : undefined;
}
