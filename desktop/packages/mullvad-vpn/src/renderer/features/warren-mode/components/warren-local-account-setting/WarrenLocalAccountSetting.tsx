import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { WarrenLocalAccountSwitch } from '../WarrenLocalAccountSwitch';

// Entry of the VPN settings page for
// `Settings::warren_local_account`. The toggle switches the Warren
// identity (BIP39 mnemonic) into self-hosted local mode
// (LocalAccountBackend + LocalDeviceBackend) instead of calling the
// remote API on daemon startup. Requires a daemon restart.
export type WarrenLocalAccountSettingProps = Omit<ListItemProps, 'children'>;

export function WarrenLocalAccountSetting(props: WarrenLocalAccountSettingProps) {
  return (
    <SettingsListItem anchorId="warren-local-account-setting" {...props}>
      <SettingsListItem.Item>
        <WarrenLocalAccountSwitch>
          <WarrenLocalAccountSwitch.Label>
            {messages.pgettext('vpn-settings-view', 'Warren local account')}
          </WarrenLocalAccountSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <InfoButton>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'When enabled, the daemon uses a self-hosted account derived from the local Warren mnemonic instead of contacting a remote account API.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'Restart the daemon for the change to take effect.',
                )}
              </ModalMessage>
            </InfoButton>

            <WarrenLocalAccountSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </WarrenLocalAccountSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
