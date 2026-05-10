import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { WarrenModeSwitch } from '../WarrenModeSwitch';

// Entry de la page VPN settings pour `Settings::warren_mode`.
// Symétrique du pattern `AllowLanSetting`. Le toggle persiste dans
// Settings (gRPC `set_warren_mode`) et nécessite un redémarrage du
// daemon pour appliquer (cf. `warren_mode::resolve`).
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
