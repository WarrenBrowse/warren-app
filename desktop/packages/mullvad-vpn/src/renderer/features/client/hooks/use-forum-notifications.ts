import React from 'react';

import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

export function useForumNotifications() {
  // Absent in a settings file written before the setting existed, and on
  // there: a user who has a forum account wants their replies.
  const forumNotifications = useSelector(
    (state) => state.settings.guiSettings.forumNotifications ?? true,
  );

  const { setForumNotifications: contextSetForumNotifications } = useAppContext();

  const setForumNotifications = React.useCallback(
    (value: boolean) => {
      try {
        contextSetForumNotifications(value);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set forum notifications', message);
      }
    },
    [contextSetForumNotifications],
  );

  return { forumNotifications, setForumNotifications };
}
