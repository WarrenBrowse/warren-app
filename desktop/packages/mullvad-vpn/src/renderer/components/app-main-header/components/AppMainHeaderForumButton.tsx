import { useCallback } from 'react';

import { unreadLabel } from '../../../../shared/forum-identity';
import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { IconButton, IconButtonProps, MainHeader } from '../../../lib/components';
import { useForumHeaderButton, useForumUnreadCount } from '../../../lib/forum-activity';
import { TransitionType, useHistory } from '../../../lib/history';
import { ToolbarBadge, ToolbarGlyph } from './ToolbarBadge';

export type MainHeaderForumButtonProps = Omit<IconButtonProps, 'icon'>;

/**
 * The header's forum slot: the activity bell and its unread badge for a
 * wallet with a forum account, opening the activity panel, or nothing.
 *
 * The shared rule (`forumHeaderButton`) puts a lifebuoy into the forum in
 * this slot for a wallet without an account. The desktop header carries its
 * own lifebuoy next to it on every state, opening the in-app report (see
 * `AppMainHeaderSupportButton`), which is where an account now comes from,
 * so the slot draws nothing for that case rather than a second lifebuoy
 * going somewhere else. The forum itself stays one click away in Settings,
 * Support.
 */
export function AppMainHeaderForumButton(props: MainHeaderForumButtonProps) {
  const history = useHistory();
  const button = useForumHeaderButton();
  const unread = useForumUnreadCount();

  const openForumActivity = useCallback(
    () => history.push(RoutePath.forumActivity, { transition: TransitionType.show }),
    [history],
  );

  if (button !== 'activity') {
    return null;
  }

  const label =
    unread > 0
      ? // TRANSLATORS: Accessible name of the forum bell when the user has
        // TRANSLATORS: unread activity.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(count)s - number of unread notifications
        messages
          .pgettext('accessibility', 'Forum, %(count)s new')
          .replace('%(count)s', unreadLabel(unread))
      : messages.pgettext('accessibility', 'Forum');

  return (
    <MainHeader.IconButton
      onClick={openForumActivity}
      data-testid="forum-button"
      aria-label={label}
      {...props}>
      {/* Outline rather than filled, matching the wordmark's thin stroke
          and the two buttons beside it. */}
      <ToolbarGlyph>
        <IconButton.Icon icon="bell-outline" />
        {unread > 0 && <ToolbarBadge aria-hidden="true" data-testid="forum-unread-badge" />}
      </ToolbarGlyph>
    </MainHeader.IconButton>
  );
}
