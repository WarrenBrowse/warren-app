import { ForumNotification, ForumNotificationKind } from '../../../../shared/forum-notifications';
import { messages } from '../../../../shared/gettext';
import { icons } from '../../../lib/components/icon/types';

/** Glyph that lets the eye sort a reply from a like before reading a word. */
export function iconFor(kind: ForumNotificationKind): keyof typeof icons {
  switch (kind) {
    case 'liked':
      return 'heart-outline';
    case 'private_message':
      return 'message-outline';
    case 'mentioned':
    case 'quoted':
      return 'account-outline';
    case 'granted_badge':
      return 'checkmark-circle';
    case 'linked':
      return 'external';
    case 'replied':
    case 'posted':
    case 'watching_first_post':
      return 'reply-outline';
    case 'other':
      return 'bell-outline';
  }
}

/** One line saying who did what, falling back when the forum said less. */
export function headlineFor(notification: ForumNotification): string {
  const actor =
    notification.actor ??
    // TRANSLATORS: Stands in for the forum member who caused a
    // TRANSLATORS: notification when the forum did not name them.
    messages.pgettext('forum-activity-view', 'Someone');

  switch (notification.kind) {
    case 'replied':
    case 'posted':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s replied'), actor);
    case 'liked':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s liked your post'), actor);
    case 'mentioned':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s mentioned you'), actor);
    case 'quoted':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s quoted you'), actor);
    case 'private_message':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s sent you a message'), actor);
    case 'linked':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s linked to your post'), actor);
    case 'granted_badge':
      // TRANSLATORS: Shown when the forum awarded the user a badge.
      return messages.pgettext('forum-activity-view', 'You earned a badge');
    case 'watching_first_post':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return fill(messages.pgettext('forum-activity-view', '%(actor)s opened a new topic'), actor);
    case 'other':
      // TRANSLATORS: Shown for a kind of forum notification this version of
      // TRANSLATORS: the app does not have its own wording for.
      return messages.pgettext('forum-activity-view', 'New forum activity');
  }
}

function fill(template: string, actor: string): string {
  return template.replace('%(actor)s', actor);
}

/**
 * Compact relative age, e.g. "2 h ago".
 *
 * `Intl.RelativeTimeFormat` rather than translated strings: it already knows
 * every locale's plural forms and its own wording, so a notification list
 * reads naturally without this app shipping a plural rule per unit per
 * language. Falls back to an absolute date past a week, where "5 weeks ago"
 * stops being the useful thing to say.
 */
export function relativeTime(createdAtUnix: number, locale: string, nowMs = Date.now()): string {
  const seconds = Math.round((createdAtUnix * 1000 - nowMs) / 1000);
  const elapsed = Math.abs(seconds);
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto', style: 'narrow' });

  if (elapsed < 60) {
    return rtf.format(Math.min(seconds, 0), 'second');
  }
  if (elapsed < 3600) {
    return rtf.format(Math.round(seconds / 60), 'minute');
  }
  if (elapsed < 86400) {
    return rtf.format(Math.round(seconds / 3600), 'hour');
  }
  if (elapsed < 7 * 86400) {
    return rtf.format(Math.round(seconds / 86400), 'day');
  }
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(
    new Date(createdAtUnix * 1000),
  );
}
