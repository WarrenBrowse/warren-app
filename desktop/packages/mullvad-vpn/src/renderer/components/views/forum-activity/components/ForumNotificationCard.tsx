import { useCallback } from 'react';
import styled from 'styled-components';

import { ForumNotification, forumPostUrl } from '../../../../../shared/forum-notifications';
import { messages } from '../../../../../shared/gettext';
import { useAppContext } from '../../../../context';
import { Icon } from '../../../../lib/components';
import { colors, Radius, spacings } from '../../../../lib/foundations';
import { useHistory } from '../../../../lib/history';
import { useSelector } from '../../../../redux/store';
import { headlineFor, iconFor, relativeTime } from '../helpers';

export interface ForumNotificationCardProps {
  notification: ForumNotification;
  onOpen: (id: number) => void;
}

// A card of its own rather than the app's `ListItem`. That primitive lays its
// group out in GRID COLUMNS, which turned every notification into three narrow
// columns of shredded text (reported 2026-08-08). Notifications are a stack of
// mixed-length prose, so they need a column layout with hard clamps instead.
const StyledCard = styled.button<{ $unread: boolean; $clickable: boolean }>`
  display: flex;
  gap: ${spacings.small};
  width: 100%;
  // The whole point of the fix: without it the flex child refuses to shrink
  // below its longest word and the card overflows the window sideways.
  min-width: 0;
  padding: ${spacings.small} ${spacings.medium} ${spacings.small} ${spacings.small};
  border: none;
  border-radius: ${Radius.radius12};
  text-align: left;
  background-color: ${({ $unread }) => ($unread ? colors.blue40 : colors.blue10)};
  cursor: ${({ $clickable }) => ($clickable ? 'pointer' : 'default')};

  @media (prefers-reduced-motion: no-preference) {
    transition: background-color 0.15s ease;
  }

  &&:hover {
    background-color: ${({ $clickable, $unread }) =>
      $clickable ? colors.blue50 : $unread ? colors.blue40 : colors.blue10};
  }

  &&:focus-visible {
    outline: 2px solid ${colors.white};
    outline-offset: 2px;
  }
`;

// The kind icon sits in its own disc so the eye can sort a reply from a like
// without reading a word.
const StyledIconDisc = styled.div<{ $unread: boolean }>`
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border-radius: ${Radius.radiusFull};
  background-color: ${({ $unread }) => ($unread ? colors.fur : colors.blue60)};
`;

const StyledBody = styled.div`
  display: flex;
  flex-direction: column;
  gap: 2px;
  // Same reason as the card: this is what lets the clamps below actually
  // clamp instead of stretching the row.
  min-width: 0;
  flex: 1;
`;

const StyledTopRow = styled.div`
  display: flex;
  align-items: baseline;
  gap: ${spacings.tiny};
  min-width: 0;
`;

const StyledHeadline = styled.span`
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: ${colors.white};
  font-size: 13px;
  font-weight: 600;
  line-height: 18px;
`;

const StyledWhen = styled.span`
  flex-shrink: 0;
  color: ${colors.whiteOnDarkBlue40};
  font-size: 11px;
  line-height: 18px;
`;

// Two lines then an ellipsis, and `anywhere` so a pasted URL wraps instead of
// pushing the card past the window edge.
const StyledClamp = styled.span<{ $lines: number; $muted: boolean }>`
  display: -webkit-box;
  -webkit-line-clamp: ${({ $lines }) => $lines};
  -webkit-box-orient: vertical;
  overflow: hidden;
  overflow-wrap: anywhere;
  color: ${({ $muted }) => ($muted ? colors.whiteOnDarkBlue60 : colors.whiteAlpha80)};
  font-size: 12px;
  line-height: 17px;
`;

// On the headline row rather than floating against the card's right edge,
// where it read as an orphan.
const StyledUnreadDot = styled.span`
  flex-shrink: 0;
  align-self: center;
  width: 7px;
  height: 7px;
  border-radius: ${Radius.radiusFull};
  background-color: ${colors.fur};
`;

export function ForumNotificationCard({ notification, onOpen }: ForumNotificationCardProps) {
  const { openUrl } = useAppContext();
  const { pop } = useHistory();
  const locale = useSelector((state) => state.userInterface.locale);
  const { id, path, unread } = notification;

  const open = useCallback(() => {
    if (path === undefined) {
      return;
    }
    onOpen(id);
    void openUrl(forumPostUrl(path));
    // The reading happens in the browser now, so leaving the panel up behind
    // it would be a list the user has to dismiss to get back to the app.
    pop();
  }, [openUrl, pop, onOpen, id, path]);

  const headline = headlineFor(notification);

  return (
    <StyledCard
      $unread={unread}
      $clickable={path !== undefined}
      onClick={path !== undefined ? open : undefined}
      // A notification pointing at no post (a badge award) is not a button.
      as={path === undefined ? 'div' : 'button'}
      aria-label={
        path !== undefined
          ? `${headline}. ${messages.pgettext('accessibility', 'Opens externally')}`
          : headline
      }>
      <StyledIconDisc $unread={unread}>
        <Icon icon={iconFor(notification.kind)} size="small" color="darkBlue" />
      </StyledIconDisc>

      <StyledBody>
        <StyledTopRow>
          {unread && <StyledUnreadDot aria-hidden="true" />}
          <StyledHeadline>{headline}</StyledHeadline>
          <StyledWhen>{relativeTime(notification.createdAt, locale)}</StyledWhen>
        </StyledTopRow>

        {notification.title !== undefined && (
          <StyledClamp $lines={2} $muted={false}>
            {notification.title}
          </StyledClamp>
        )}
        {notification.excerpt !== undefined && (
          <StyledClamp $lines={2} $muted>
            {notification.excerpt}
          </StyledClamp>
        )}
      </StyledBody>
    </StyledCard>
  );
}
