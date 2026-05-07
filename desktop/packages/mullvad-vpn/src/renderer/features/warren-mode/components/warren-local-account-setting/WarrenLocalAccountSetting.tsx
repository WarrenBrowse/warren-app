import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { WarrenLocalAccountSwitch } from '../WarrenLocalAccountSwitch';

// Warren fork — Phase H : entry de la page VPN settings pour
// `Settings::warren_local_account`. Le toggle bascule l'identité Warren
// (mnémonique BIP39) en mode local self-hosted (LocalAccountBackend +
// LocalDeviceBackend) au lieu d'appeler l'API distante au démarrage du
// daemon. Nécessite un redémarrage du daemon.
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
