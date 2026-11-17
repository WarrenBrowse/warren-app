import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { QuantumResistantSwitch } from '../quantum-resistant-switch';

export type QuantumResistantSettingProps = Omit<ListItemProps, 'children'>;

export function QuantumResistantSetting(props: QuantumResistantSettingProps) {
  return (
    <SettingsListItem anchorId="quantum-resistant-setting" {...props}>
      <SettingsListItem.Item>
        <QuantumResistantSwitch>
          <QuantumResistantSwitch.Label>
            {
              // TRANSLATORS: The title for the WireGuard quantum resistance selector. This setting
              // TRANSLATORS: makes the cryptography resistant to the future abilities of quantum
              // TRANSLATORS: computers.
              messages.pgettext('wireguard-settings-view', 'Quantum-resistant tunnel')
            }
          </QuantumResistantSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <InfoButton>
              <>
                <ModalMessage>
                  {messages.pgettext(
                    'wireguard-settings-view',
                    'This feature protects your traffic to the exit server against potential attacks from quantum computers.',
                  )}
                </ModalMessage>
                <ModalMessage>
                  {messages.pgettext(
                    'wireguard-settings-view',
                    'It combines a quantum-safe key exchange with the classical one. When it is on, the app only connects through servers that offer it. When it is off, it is still used wherever a server offers it.',
                  )}
                </ModalMessage>
              </>
            </InfoButton>

            <QuantumResistantSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </QuantumResistantSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
