import React from 'react';

import { messages } from '../../../../../shared/gettext';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { useHasForumAccount } from '../../../../lib/forum-activity';
import { ForumNotificationsSwitch } from '../forum-notifications-switch/ForumNotificationsSwitch';

export type ForumNotificationsSettingProps = Omit<ListItemProps, 'children'>;

/**
 * Banner and tray dot for community-forum activity.
 *
 * Rendered only for a wallet that has a forum account: to everyone else it
 * would be a switch over a feature they have never seen, naming a place
 * they have not signed up to.
 */
export function ForumNotificationsSetting(props: ForumNotificationsSettingProps) {
  const descriptionId = React.useId();
  const hasForumAccount = useHasForumAccount();

  if (!hasForumAccount) {
    return null;
  }

  return (
    <SettingsListItem {...props}>
      <SettingsListItem.Item>
        <ForumNotificationsSwitch descriptionId={descriptionId}>
          <ForumNotificationsSwitch.Label>
            {
              // TRANSLATORS: Label of the setting that turns notifications
              // TRANSLATORS: about community-forum activity on or off.
              messages.pgettext('user-interface-settings-view', 'Forum notifications')
            }
          </ForumNotificationsSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <ForumNotificationsSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </ForumNotificationsSwitch>
      </SettingsListItem.Item>
      <SettingsListItem.Footer>
        <SettingsListItem.Footer.Text id={descriptionId}>
          {
            // TRANSLATORS: Description of the forum notifications setting.
            messages.pgettext(
              'user-interface-settings-view',
              'Be told when someone replies to you on the community forum. Turning this off also hides the forum bell.',
            )
          }
        </SettingsListItem.Footer.Text>
      </SettingsListItem.Footer>
    </SettingsListItem>
  );
}
