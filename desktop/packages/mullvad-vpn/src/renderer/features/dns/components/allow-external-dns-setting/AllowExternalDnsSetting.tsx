import { messages } from '../../../../../shared/gettext';
import InfoButton from '../../../../components/InfoButton';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { ListItemProps } from '../../../../lib/components/list-item';
import { AllowExternalDnsSwitch } from '../allow-external-dns-switch/AllowExternalDnsSwitch';

export type AllowExternalDnsSettingProps = Omit<ListItemProps, 'children'>;

export function AllowExternalDnsSetting(props: AllowExternalDnsSettingProps) {
  return (
    <SettingsListItem anchorId="allow-external-dns-setting" {...props}>
      <SettingsListItem.Item>
        <AllowExternalDnsSwitch>
          <AllowExternalDnsSwitch.Label>
            {
              // TRANSLATORS: Label for the advanced setting that allows DNS queries to resolvers
              // TRANSLATORS: other than the configured one.
              messages.pgettext('vpn-settings-view', 'Allow external DNS resolvers')
            }
          </AllowExternalDnsSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <InfoButton>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'For advanced users. When enabled, the firewall stops blocking DNS queries to resolvers other than the configured one, so tools like "dig @1.1.1.1" work while connected.',
                )}
              </ModalMessage>
              <ModalMessage>
                {messages.pgettext(
                  'vpn-settings-view',
                  'The queries still travel through the VPN tunnel, but the resolver you choose will see them. Leave this off unless you are testing remote DNS resolution.',
                )}
              </ModalMessage>
            </InfoButton>

            <AllowExternalDnsSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </AllowExternalDnsSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
