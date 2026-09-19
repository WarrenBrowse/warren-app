import React from 'react';

import { messages } from '../../../../../shared/gettext';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { PortForwardingNotificationsSwitch } from '../port-forwarding-notifications-switch/PortForwardingNotificationsSwitch';

export type PortForwardingNotificationsSettingProps = Omit<ListItemProps, 'children'>;

/**
 * Toast when the exit grants, moves or drops a public port.
 *
 * Sits under the port-forwarding toggle because it is only about ports: the
 * global notification setting governs the tunnel toasts, and a user who opened
 * a port has a different appetite for one than for the other.
 */
export function PortForwardingNotificationsSetting(props: PortForwardingNotificationsSettingProps) {
  const descriptionId = React.useId();

  return (
    <SettingsListItem {...props}>
      <SettingsListItem.Item>
        <PortForwardingNotificationsSwitch descriptionId={descriptionId}>
          <PortForwardingNotificationsSwitch.Label>
            {
              // TRANSLATORS: Label of the setting that turns the notification
              // TRANSLATORS: about a changed public port on or off.
              messages.pgettext('port-forwarding-view', 'Notify me when a public port changes')
            }
          </PortForwardingNotificationsSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <PortForwardingNotificationsSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </PortForwardingNotificationsSwitch>
      </SettingsListItem.Item>
      <SettingsListItem.Footer>
        <SettingsListItem.Footer.Text id={descriptionId}>
          {
            // TRANSLATORS: Description of the port-forwarding notification setting.
            messages.pgettext(
              'port-forwarding-view',
              'A system notification shows the new port number.',
            )
          }
        </SettingsListItem.Footer.Text>
      </SettingsListItem.Footer>
    </SettingsListItem>
  );
}
