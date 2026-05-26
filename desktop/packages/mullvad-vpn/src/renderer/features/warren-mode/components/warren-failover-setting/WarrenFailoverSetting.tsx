import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { WarrenFailoverSwitch } from '../WarrenFailoverSwitch';

// Entry of the VPN settings page for the multi-exit auto-failover
// toggle (M5.B.2). The daemon handles failover unconditionally;
// this GUI toggle controls whether the failover notification toast
// is shown and persists in gui_settings.json. No daemon restart needed.
export type WarrenFailoverSettingProps = Omit<ListItemProps, 'children'>;

export function WarrenFailoverSetting(props: WarrenFailoverSettingProps) {
  return (
    <SettingsListItem anchorId="warren-failover-setting" {...props}>
      <SettingsListItem.Item>
        <WarrenFailoverSwitch>
          <WarrenFailoverSwitch.Label>
            {messages.pgettext('vpn-settings-view', 'Automatic failover')}
          </WarrenFailoverSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <InfoButton>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'Automatically switch to another exit server if the current one becomes unreachable.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'This setting takes effect immediately without a daemon restart.',
                )}
              </ModalMessage>
            </InfoButton>

            <WarrenFailoverSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </WarrenFailoverSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
