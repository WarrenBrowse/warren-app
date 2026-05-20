import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { WarrenMultiHopSwitch } from '../WarrenMultiHopSwitch';

// Toggle entry for `Settings::warren_multi_hop` (M4.E.D two-relayed
// QUIC). Surfaces in the dedicated Warren multi-hop view. A daemon
// restart is required for a change to take effect (the supervisor is
// wired at boot from the settings file + WARREN_MULTI_HOP env var).
export type WarrenMultiHopSettingProps = Omit<ListItemProps, 'children'>;

export function WarrenMultiHopSetting(props: WarrenMultiHopSettingProps) {
  return (
    <SettingsListItem anchorId="warren-multi-hop-setting" {...props}>
      <SettingsListItem.Item>
        <WarrenMultiHopSwitch>
          <WarrenMultiHopSwitch.Label>
            {messages.pgettext('warren-multi-hop-view', 'Warren multi-hop')}
          </WarrenMultiHopSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <InfoButton>
              <ModalMessage>
                {messages.pgettext(
                  'warren-multi-hop-view',
                  'When enabled, traffic is routed through two Warren relays (entry then exit) using HPKE end-to-end encryption between the client and the exit. The entry relay only sees ciphertext; the exit only sees the decrypted payload, never the user IP.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'warren-multi-hop-view',
                  'Expect roughly half the single-hop bandwidth in exchange for unlinkability between the user IP and the destination IP. Off by default; opt-in for privacy.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'warren-multi-hop-view',
                  'Restart the Warren daemon after changing this setting.',
                )}
              </ModalMessage>
            </InfoButton>
            <WarrenMultiHopSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </WarrenMultiHopSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
