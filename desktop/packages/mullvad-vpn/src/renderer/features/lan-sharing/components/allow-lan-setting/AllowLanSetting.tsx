import React, { useCallback, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import log from '../../../../../shared/logging';
import * as Cell from '../../../../components/cell';
import InfoButton from '../../../../components/InfoButton';
import List, { stringValueAsKey } from '../../../../components/List';
import { ModalMessage } from '../../../../components/Modal';
import { SettingsAccordion } from '../../../../components/settings-accordion';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { IconButton } from '../../../../lib/components';
import { AccordionProps } from '../../../../lib/components/accordion';
import { ListItemProps } from '../../../../lib/components/list-item';
import { spacings } from '../../../../lib/foundations';
import { useBoolean, useStyledRef } from '../../../../lib/utility-hooks';
import { useAllowLan, useLanNetworks } from '../../hooks';
import { checkLanNetwork, LanNetworkError } from '../../utils';
import { AllowLanSwitch } from '../allow-lan-switch/AllowLanSwitch';
import {
  StyledActionButton,
  StyledActionContainer,
  StyledLabel,
  StyledNetworkContainer,
} from './AllowLanSettingStyles';

export type AllowLanSettingProps = Omit<AccordionProps, 'children'> &
  Pick<ListItemProps, 'position'>;

const LanIpRanges = styled.ul({
  listStyle: 'disc outside',
  marginLeft: spacings.large,
});

// The Windows firewall compiles the shared ranges in, and the daemon refuses a custom list there.
const customisable = window.env.platform === 'darwin' || window.env.platform === 'linux';

type AddError = LanNetworkError | 'refused';

export function AllowLanSetting({ position, ...props }: AllowLanSettingProps) {
  const { allowLan } = useAllowLan();
  const { lanNetworks, setLanNetworks } = useLanNetworks();

  const [inputVisible, showInput, hideInput] = useBoolean(false);
  const [addError, setAddError] = useState<AddError>();
  const descriptionId = React.useId();

  const inputContainerRef = useStyledRef<HTMLDivElement>();

  const onInputChange = useCallback(() => setAddError(undefined), []);

  // Focus moving to the row's own submit button must not close the row before it submits.
  const onInputBlur = useCallback(
    (event?: React.FocusEvent<HTMLTextAreaElement>) => {
      const relatedTarget = event?.relatedTarget as Node | undefined;
      if (relatedTarget && inputContainerRef.current?.contains(relatedTarget)) {
        event?.target.focus();
      } else {
        hideInput();
        setAddError(undefined);
      }
    },
    [hideInput, inputContainerRef],
  );

  const onAdd = useCallback(
    async (input: string) => {
      const network = input.trim();
      const error = checkLanNetwork(network);
      if (error !== undefined) {
        setAddError(error);
        return;
      }
      try {
        await setLanNetworks([...lanNetworks.networks, network]);
        hideInput();
      } catch (e) {
        log.error('Could not add a local network', e instanceof Error ? e.message : '');
        setAddError('refused');
      }
    },
    [hideInput, lanNetworks.networks, setLanNetworks],
  );

  const onRemove = useCallback(
    async (network: string) => {
      try {
        await setLanNetworks(lanNetworks.networks.filter((item) => item !== network));
      } catch (e) {
        log.error('Could not remove a local network', e instanceof Error ? e.message : '');
      }
    },
    [lanNetworks.networks, setLanNetworks],
  );

  const onReset = useCallback(async () => {
    try {
      await setLanNetworks(undefined);
    } catch (e) {
      log.error('Could not reset the local networks', e instanceof Error ? e.message : '');
    }
  }, [setLanNetworks]);

  return (
    <SettingsAccordion
      accordionId="allow-lan-setting"
      anchorId="allow-lan-setting"
      expanded={customisable && allowLan}
      {...props}>
      <SettingsAccordion.Container>
        <SettingsAccordion.Header position={position}>
          <SettingsAccordion.Header.Item>
            <AllowLanSwitch descriptionId={customisable ? descriptionId : undefined}>
              <AllowLanSwitch.Label>
                {messages.pgettext('vpn-settings-view', 'Local network sharing')}
              </AllowLanSwitch.Label>
              <SettingsAccordion.Header.Item.ActionGroup>
                <InfoButton>
                  <ModalMessage>
                    {messages.pgettext(
                      'vpn-settings-view',
                      'This feature allows access to other devices on the local network, such as for sharing, printing, streaming, etc.',
                    )}
                  </ModalMessage>
                  <ModalMessage>
                    {messages.pgettext(
                      'vpn-settings-view',
                      'It does this by allowing network communication outside the tunnel to local multicast and broadcast ranges as well as to and from these private IP ranges:',
                    )}
                    <LanIpRanges>
                      {lanNetworks.networks.map((network) => (
                        <li key={network}>{network}</li>
                      ))}
                    </LanIpRanges>
                  </ModalMessage>
                </InfoButton>
                <AllowLanSwitch.Input />
              </SettingsAccordion.Header.Item.ActionGroup>
            </AllowLanSwitch>
          </SettingsAccordion.Header.Item>
        </SettingsAccordion.Header>

        {customisable && (
          <SettingsAccordion.Content>
            <Cell.Section role="listbox">
              <List items={lanNetworks.networks} getKey={stringValueAsKey} skipAddTransition>
                {(network) => <NetworkItem network={network} onRemove={onRemove} />}
              </List>
            </Cell.Section>

            {inputVisible && (
              <div ref={inputContainerRef}>
                <Cell.RowInput
                  placeholder={
                    // TRANSLATORS: Placeholder of the field where the user types a network to share
                    // TRANSLATORS: outside the tunnel, in CIDR notation.
                    messages.pgettext('vpn-settings-view', 'Enter a network, e.g. 192.168.1.0/24')
                  }
                  onSubmit={onAdd}
                  onChange={onInputChange}
                  onBlur={onInputBlur}
                  invalid={addError !== undefined}
                  paddingLeft={32}
                  autofocus
                />
              </div>
            )}

            <StyledActionContainer>
              <StyledActionButton onClick={showInput} disabled={inputVisible} tabIndex={-1}>
                <StyledLabel tabIndex={-1}>
                  {messages.pgettext('vpn-settings-view', 'Add a network')}
                </StyledLabel>
              </StyledActionButton>
              <IconButton variant="secondary" onClick={showInput}>
                <IconButton.Icon icon="add-circle" />
              </IconButton>
            </StyledActionContainer>

            {lanNetworks.custom && (
              <StyledActionContainer>
                <StyledActionButton onClick={onReset} tabIndex={-1}>
                  <StyledLabel tabIndex={-1}>
                    {
                      // TRANSLATORS: Restores the list of networks shared outside the tunnel
                      // TRANSLATORS: to the built-in private IP ranges.
                      messages.pgettext('vpn-settings-view', 'Reset to default')
                    }
                  </StyledLabel>
                </StyledActionButton>
                <IconButton
                  variant="secondary"
                  onClick={onReset}
                  aria-label={messages.pgettext('vpn-settings-view', 'Reset to default')}>
                  <IconButton.Icon icon="history-remove" />
                </IconButton>
              </StyledActionContainer>
            )}
          </SettingsAccordion.Content>
        )}

        {customisable && allowLan && (
          <SettingsListItem.Footer>
            <SettingsListItem.Footer.Text id={descriptionId}>
              {addError !== undefined
                ? addErrorMessage(addError)
                : messages.pgettext(
                    'vpn-settings-view',
                    'Traffic to these networks goes outside the VPN tunnel.',
                  )}
            </SettingsListItem.Footer.Text>
          </SettingsListItem.Footer>
        )}
      </SettingsAccordion.Container>
    </SettingsAccordion>
  );
}

function addErrorMessage(error: AddError): string {
  switch (error) {
    case 'invalid':
      return messages.pgettext(
        'vpn-settings-view',
        'Enter a network in CIDR notation, such as 192.168.1.0/24 or fd00::/8.',
      );
    case 'too-broad':
      return messages.pgettext(
        'vpn-settings-view',
        'This network is too broad to be shared outside the tunnel.',
      );
    case 'refused':
      return messages.pgettext('vpn-settings-view', 'This network cannot be shared.');
  }
}

interface NetworkItemProps {
  network: string;
  onRemove: (network: string) => void;
}

function NetworkItem({ network, onRemove }: NetworkItemProps) {
  const remove = useCallback(() => onRemove(network), [network, onRemove]);

  return (
    <StyledNetworkContainer>
      <StyledLabel>{network}</StyledLabel>
      <IconButton
        variant="secondary"
        onClick={remove}
        aria-label={messages.pgettext('accessibility', 'Remove item')}>
        <IconButton.Icon icon="cross-circle" />
      </IconButton>
    </StyledNetworkContainer>
  );
}
