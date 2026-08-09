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
  | 'announcement'
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
  'announcement',
  'other',
];

export interface ForumNotification {
  id: number;
  kind: ForumNotificationKind;
  // Unread by Discourse's own rule, so a card marked unread is one the
  // header badge counted.
  unread: boolean;
  // Unix epoch seconds.
  createdAt: number;
  title?: string;
  actor?: string;
  excerpt?: string;
  // Forum-relative path the row opens, e.g. `/t/86/4` for a post or
  // `/u/<handle>/messages/group/staff` for a group inbox summary. Absent when
  // the notification points at nothing openable.
  path?: string;
}

// A path is opened in the user's browser, so it is pinned to the exact
// shapes the forum produces rather than trusted: anything else could send
// the user somewhere the app never meant to. Both are anchored at each end and
// allow no slash inside a segment, so neither can climb out of the forum.
const FORUM_PATH_PATTERNS = [
  // A post, or the topic when it is the first one.
  /^\/t\/\d+(\/\d+)?$/,
  // A group's message list: what a group inbox summary points at, being the
  // one notification kind that opens something while carrying no topic.
  /^\/u\/[\w.-]+\/messages\/group\/[\w.-]+$/,
];

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
    unread: row['unread'] === true,
    createdAt,
    title: text(row['title'], MAX_LABEL),
    actor: text(row['actor'], MAX_LABEL),
    excerpt: text(row['excerpt'], MAX_EXCERPT),
    path:
      typeof path === 'string' && FORUM_PATH_PATTERNS.some((p) => p.test(path)) ? path : undefined,
  };
}

function parseKind(value: unknown): ForumNotificationKind {
  return KINDS.includes(value as ForumNotificationKind)
    ? (value as ForumNotificationKind)
    : 'other';
}

/**
 * Browser URL for the post a notification points at.
 *
 * The bare post, never the forum's `/session/sso` entry: that route runs
 * the whole wallet round trip on every visit, including for a browser
 * already signed in to the forum, which is the ordinary case. A reader
 * who is signed out lands on the topic and signs in from there, which is
 * one extra click in the rare case instead of a detour through the
 * identity broker in every case.
 */
export function forumPostUrl(path: string): Url {
  return `${urls.forum}${path.replace(/^\//, '')}`;
}

function text(value: unknown, max: number): string | undefined {
  if (typeof value !== 'string') {
    return undefined;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed.slice(0, max) : undefined;
}
