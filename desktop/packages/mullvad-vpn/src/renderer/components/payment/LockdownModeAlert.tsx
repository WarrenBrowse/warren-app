import styled from 'styled-components';

import { messages } from '../../../shared/gettext';
import { LockdownModeSwitch } from '../../features/tunnel/components';
import { Button } from '../../lib/components';
import { spacings } from '../../lib/foundations';
import { ModalAlert, ModalAlertType, ModalMessage } from '../Modal';
import { SettingsListItem } from '../settings-list-item';

const StyledSettingsToggleListItem = styled(SettingsListItem)`
  margin-top: ${spacings.medium};
`;

interface LockdownModeAlertProps {
  isOpen: boolean;
  onClose: () => void;
}

// Lockdown mode is a deliberate security choice: never auto-disable
// it to open the checkout. Explain and hand the user the switch.
export function LockdownModeAlert({ isOpen, onClose }: LockdownModeAlertProps) {
  return (
    <ModalAlert
      isOpen={isOpen}
      type={ModalAlertType.caution}
      buttons={[
        <Button key="cancel" onClick={onClose}>
          <Button.Text>{messages.gettext('Close')}</Button.Text>
        </Button>,
      ]}
      close={onClose}>
      <ModalMessage>
        {messages.pgettext(
          'connect-view',
          'You need to disable "Lockdown mode" in order to access the Internet to add time.',
        )}
      </ModalMessage>
      <ModalMessage>
        {messages.pgettext(
          'connect-view',
          'Remember, turning it off will allow network traffic while the VPN is disconnected until you turn it back on under Advanced settings.',
        )}
      </ModalMessage>
      <StyledSettingsToggleListItem>
        <SettingsListItem.Item>
          <LockdownModeSwitch>
            <LockdownModeSwitch.Label variant="titleMedium">
              {messages.pgettext('vpn-settings-view', 'Lockdown mode')}
            </LockdownModeSwitch.Label>
            <SettingsListItem.Item.ActionGroup>
              <LockdownModeSwitch.Input />
            </SettingsListItem.Item.ActionGroup>
          </LockdownModeSwitch>
        </SettingsListItem.Item>
      </StyledSettingsToggleListItem>
    </ModalAlert>
  );
}
