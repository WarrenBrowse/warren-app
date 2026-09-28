import { useCallback, useId, useMemo } from 'react';
import styled from 'styled-components';

import { IpVersion, wrapConstraint } from '../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../shared/gettext';
import log from '../../../../../../shared/logging';
import { AccordionProps } from '../../../../../lib/components/accordion';
import { ListItem } from '../../../../../lib/components/list-item';
import { useRelaySettingsUpdater } from '../../../../../lib/constraint-updater';
import { spacings } from '../../../../../lib/foundations';
import { useSelector } from '../../../../../redux/store';
import InfoButton from '../../../../InfoButton';
import { ModalMessage } from '../../../../Modal';
import { SettingsAccordion } from '../../../../settings-accordion';
import { SettingsListbox } from '../../../../settings-listbox';

export type IpVersionSettingProps = Omit<AccordionProps, 'children'>;

const StyledAccordionTrigger = styled(SettingsAccordion.Header.AccordionTrigger)`
  display: grid;
  place-items: center;
  align-self: stretch;
  width: 48px;
`;

// The shared action group lays out two columns at most; this row carries a value, an info
// button and the chevron.
const StyledTrailing = styled.div`
  display: flex;
  align-items: center;
  align-self: stretch;
  gap: ${spacings.small};
`;

// Folded by default: the automatic choice is almost always right, so the three options only
// cost screen space until someone needs to override it.
export function IpVersionSetting(props: IpVersionSettingProps) {
  const titleId = useId();
  const relaySettingsUpdater = useRelaySettingsUpdater();
  const relaySettings = useSelector((state) => state.settings.relaySettings);
  const ipVersion = useMemo(() => {
    const ipVersion = 'normal' in relaySettings ? relaySettings.normal.wireguard.ipVersion : 'any';
    return ipVersion === 'any' ? null : ipVersion;
  }, [relaySettings]);

  const setIpVersion = useCallback(
    async (ipVersion: IpVersion | null) => {
      try {
        await relaySettingsUpdater((settings) => {
          settings.wireguardConstraints.ipVersion = wrapConstraint(ipVersion);
          return settings;
        });
      } catch (e) {
        const error = e as Error;
        log.error('Failed to update relay settings', error.message);
      }
    },
    [relaySettingsUpdater],
  );

  const automatic = messages.gettext('Automatic');
  const ipv4 = messages.gettext('IPv4');
  const ipv6 = messages.gettext('IPv6');
  const currentLabel = ipVersion === 'ipv4' ? ipv4 : ipVersion === 'ipv6' ? ipv6 : automatic;

  return (
    <SettingsAccordion accordionId="ip-version-setting" titleId={titleId} {...props}>
      <SettingsAccordion.Container>
        <SettingsAccordion.Header>
          <SettingsAccordion.Header.Item>
            <SettingsAccordion.Header.Item.Title>
              {
                // TRANSLATORS: Title for device IP version setting.
                messages.pgettext('wireguard-settings-view', 'Device IP version')
              }
            </SettingsAccordion.Header.Item.Title>
            <StyledTrailing>
              <ListItem.Item.Text>{currentLabel}</ListItem.Item.Text>
              <InfoButton>
                <ModalMessage>
                  {
                    // TRANSLATORS: A description for the setting Device IP version,
                    // TRANSLATORS: explaining how the user can configure the setting.
                    messages.pgettext(
                      'vpn-settings-view',
                      'This feature allows you to choose whether to use only IPv4, only IPv6, or allow the app to automatically decide the best option when connecting to a server.',
                    )
                  }
                </ModalMessage>
                <ModalMessage>
                  {
                    // TRANSLATORS: A complimentary description for the setting Device IP version,
                    // TRANSLATORS: explaining why the user might want to configure the setting.
                    messages.pgettext(
                      'vpn-settings-view',
                      'It can be useful when you are aware of problems caused by a certain IP version.',
                    )
                  }
                </ModalMessage>
              </InfoButton>

              <StyledAccordionTrigger>
                <SettingsAccordion.Header.Item.Chevron />
              </StyledAccordionTrigger>
            </StyledTrailing>
          </SettingsAccordion.Header.Item>
        </SettingsAccordion.Header>
        <SettingsAccordion.Content>
          <SettingsListbox value={ipVersion} onValueChange={setIpVersion} labelId={titleId}>
            <SettingsListbox.Options>
              <SettingsListbox.Options.BaseOption value={null}>
                {automatic}
              </SettingsListbox.Options.BaseOption>
              <SettingsListbox.Options.BaseOption value={'ipv4'}>
                {ipv4}
              </SettingsListbox.Options.BaseOption>
              <SettingsListbox.Options.BaseOption value={'ipv6'}>
                {ipv6}
              </SettingsListbox.Options.BaseOption>
            </SettingsListbox.Options>
          </SettingsListbox>
        </SettingsAccordion.Content>
      </SettingsAccordion.Container>
    </SettingsAccordion>
  );
}
