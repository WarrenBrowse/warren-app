import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { WarrenModeSwitch } from '../WarrenModeSwitch';

// Entry of the VPN settings page for `Settings::warren_mode`.
// Symmetric to the `AllowLanSetting` pattern. The toggle persists in
// Settings (gRPC `set_warren_mode`) and requires a daemon restart to
// apply (cf. `warren_mode::resolve`).
export type WarrenModeSettingProps = Omit<ListItemProps, 'children'>;

export function WarrenModeSetting(props: WarrenModeSettingProps) {
  return (
    <SettingsListItem anchorId="warren-mode-setting" {...props}>
      <SettingsListItem.Item>
        <WarrenModeSwitch>
          <WarrenModeSwitch.Label>
            {messages.pgettext('vpn-settings-view', 'Warren tunnel mode')}
          </WarrenModeSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <InfoButton>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'When enabled, traffic is routed through Warren exits over an Iroh QUIC tunnel instead of WireGuard / OpenVPN.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'Restart the daemon for the change to take effect.',
                )}
              </ModalMessage>
            </InfoButton>

            <WarrenModeSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </WarrenModeSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
