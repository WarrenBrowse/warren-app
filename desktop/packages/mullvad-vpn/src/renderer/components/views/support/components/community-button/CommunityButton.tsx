import { useCallback } from 'react';

import { urls } from '../../../../../../shared/constants';
import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { ListItem } from '../../../../../lib/components/list-item';
import { useSelector } from '../../../../../redux/store';

// Opens the community + support forum. Login there is wallet-based
// (DiscourseConnect wallet SSO); the resulting `warren://forum-login` deep
// link is handled by the main process (forum-login.ts), so no credentials are
// ever entered by the user.
export function CommunityButton() {
  const isOffline = useSelector((state) => state.connection.isBlocked);
  const { openUrl } = useAppContext();

  const openForum = useCallback(() => openUrl(urls.forum), [openUrl]);

  return (
    <ListItem disabled={isOffline}>
      <ListItem.Trigger
        onClick={openForum}
        aria-description={messages.pgettext('accessibility', 'Opens externally')}>
        <ListItem.Item>
          <ListItem.Item.Label>
            {
              // TRANSLATORS: Link to the community + support forum
              messages.pgettext('support-view', 'Community forum')
            }
          </ListItem.Item.Label>
          <ListItem.Item.ActionGroup>
            <ListItem.Item.Icon icon="external" />
          </ListItem.Item.ActionGroup>
        </ListItem.Item>
      </ListItem.Trigger>
    </ListItem>
  );
}
