import { useCallback } from 'react';

import { urls } from '../../../../shared/constants';
import { unreadLabel } from '../../../../shared/forum-identity';
import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { IconButton, IconButtonProps, MainHeader } from '../../../lib/components';
import { useForumHeaderButton, useForumUnreadCount } from '../../../lib/forum-activity';
import { TransitionType, useHistory } from '../../../lib/history';
import { ToolbarBadge, ToolbarGlyph } from './ToolbarBadge';

export type MainHeaderForumButtonProps = Omit<IconButtonProps, 'icon'>;

/**
 * The header's forum slot, which carries one of two buttons or none at all
 * (see `forumHeaderButton`).
 *
 * With a forum account, the activity bell and its unread badge, opening the
 * activity panel. Without one, a lifebuoy opening the forum itself in the
 * browser: the bell would be inert for a wallet that has never signed in,
 * while the forum is where help and an account both come from. Forum
 * notifications off removes the slot entirely, lifebuoy included.
 */
export function AppMainHeaderForumButton(props: MainHeaderForumButtonProps) {
  const history = useHistory();
  const button = useForumHeaderButton();
  const unread = useForumUnreadCount();
  const { openUrl } = useAppContext();

  const openForumActivity = useCallback(
    () => history.push(RoutePath.forumActivity, { transition: TransitionType.show }),
    [history],
  );
  const openForum = useCallback(() => openUrl(urls.forum), [openUrl]);

  if (button === 'none') {
    return null;
  }

  if (button === 'community') {
    return (
      <MainHeader.IconButton
        onClick={openForum}
        data-testid="forum-community-button"
        aria-label={
          // TRANSLATORS: Accessible name of the header button that opens the
          // TRANSLATORS: community forum, shown before the user has a forum
          // TRANSLATORS: account.
          messages.pgettext('accessibility', 'Community forum')
        }
        aria-description={messages.pgettext('accessibility', 'Opens externally')}
        {...props}>
        {/* Same outline weight as the bell it stands in for, so the header
            keeps one stroke across its three buttons. */}
        <IconButton.Icon icon="lifebuoy-outline" />
      </MainHeader.IconButton>
    );
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
