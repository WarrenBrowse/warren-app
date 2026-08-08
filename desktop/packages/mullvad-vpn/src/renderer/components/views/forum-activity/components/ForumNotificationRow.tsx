import { useCallback } from 'react';

import { ForumNotification, forumPostUrl } from '../../../../../shared/forum-notifications';
import { messages } from '../../../../../shared/gettext';
import { useAppContext } from '../../../../context';
import { BodySmall, LabelTiny } from '../../../../lib/components';
import { ListItem } from '../../../../lib/components/list-item';
import { useSelector } from '../../../../redux/store';

export interface ForumNotificationRowProps {
  notification: ForumNotification;
}

export function ForumNotificationRow({ notification }: ForumNotificationRowProps) {
  const { openUrl } = useAppContext();
  const locale = useSelector((state) => state.userInterface.locale);
  const { path } = notification;

  const open = useCallback(() => {
    if (path !== undefined) {
      void openUrl(forumPostUrl(path));
    }
  }, [openUrl, path]);

  const body = (
    <ListItem.Item>
      <ListItem.Item.Group>
        <ListItem.Item.Label>{headline(notification)}</ListItem.Item.Label>
        {notification.title !== undefined && (
          <BodySmall color="whiteAlpha80">{notification.title}</BodySmall>
        )}
        {notification.excerpt !== undefined && (
          <BodySmall color="whiteAlpha60">{notification.excerpt}</BodySmall>
        )}
        <LabelTiny color="whiteAlpha40">{when(notification.createdAt, locale)}</LabelTiny>
      </ListItem.Item.Group>
      {path !== undefined && (
        <ListItem.Item.ActionGroup>
          <ListItem.Item.Icon icon="external" />
        </ListItem.Item.ActionGroup>
      )}
    </ListItem.Item>
  );

  // A notification that points at no post (a badge award) has nothing to
  // open, so it stays a row rather than becoming a dead button.
  return (
    <ListItem>
      {path === undefined ? (
        body
      ) : (
        <ListItem.Trigger
          onClick={open}
          aria-description={messages.pgettext('accessibility', 'Opens externally')}>
          {body}
        </ListItem.Trigger>
      )}
    </ListItem>
  );
}

/** One line saying who did what, falling back when the provider said less. */
function headline(notification: ForumNotification): string {
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
      return format(messages.pgettext('forum-activity-view', '%(actor)s replied'), actor);
    case 'liked':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return format(messages.pgettext('forum-activity-view', '%(actor)s liked your post'), actor);
    case 'mentioned':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return format(messages.pgettext('forum-activity-view', '%(actor)s mentioned you'), actor);
    case 'quoted':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return format(messages.pgettext('forum-activity-view', '%(actor)s quoted you'), actor);
    case 'private_message':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return format(
        messages.pgettext('forum-activity-view', '%(actor)s sent you a message'),
        actor,
      );
    case 'linked':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return format(
        messages.pgettext('forum-activity-view', '%(actor)s linked to your post'),
        actor,
      );
    case 'granted_badge':
      // TRANSLATORS: Shown when the forum awarded the user a badge.
      return messages.pgettext('forum-activity-view', 'You earned a badge');
    case 'watching_first_post':
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(actor)s - the forum member's public name
      return format(
        messages.pgettext('forum-activity-view', '%(actor)s opened a new topic'),
        actor,
      );
    case 'other':
      // TRANSLATORS: Shown for a kind of forum notification this version of
      // TRANSLATORS: the app does not have its own wording for.
      return messages.pgettext('forum-activity-view', 'New forum activity');
  }
}

function format(template: string, actor: string): string {
  return template.replace('%(actor)s', actor);
}

/**
 * Absolute date and time in the app's locale. Absolute rather than
 * relative: a relative wording needs its own translated plural forms for
 * every unit, and a support conversation is easier to follow against a
 * clock anyway.
 */
function when(createdAtUnix: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'short', timeStyle: 'short' }).format(
    new Date(createdAtUnix * 1000),
  );
}
