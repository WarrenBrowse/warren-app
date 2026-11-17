import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { builtAppSourceUrl, builtEngineSourceUrl } from '../../../../shared/source-links';
import { useAppContext } from '../../../context';
import { BodySmall, TitleBig } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { ListItem } from '../../../lib/components/list-item';
import { View } from '../../../lib/components/view';
import { colors } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';

const LicenseText = styled.pre({
  margin: 0,
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere',
  fontFamily: 'inherit',
  fontSize: '12px',
  lineHeight: '18px',
  color: colors.whiteAlpha60,
});

export function licensesTitle() {
  // TRANSLATORS: Title of the view listing the licences of the app and its parts.
  return messages.pgettext('licenses-view', 'Licenses');
}

interface SourceLinkProps {
  label: string;
  onClick: () => void;
  disabled?: boolean;
}

function SourceLink({ label, onClick, disabled }: SourceLinkProps) {
  return (
    <ListItem disabled={disabled}>
      <ListItem.Trigger
        onClick={onClick}
        aria-description={messages.pgettext('accessibility', 'Opens externally')}>
        <ListItem.Item>
          <ListItem.Item.Label>{label}</ListItem.Item.Label>
          <ListItem.Item.ActionGroup>
            <ListItem.Item.Icon icon="external" />
          </ListItem.Item.ActionGroup>
        </ListItem.Item>
      </ListItem.Trigger>
    </ListItem>
  );
}

export interface LicensesContentProps {
  licenseText: string;
  openAppSource: () => void;
  openEngineSource: () => void;
  linksDisabled?: boolean;
}

export function LicensesContent({
  licenseText,
  openAppSource,
  openEngineSource,
  linksDisabled,
}: LicensesContentProps) {
  return (
    <FlexColumn gap="medium">
      <FlexColumn gap="small">
        <BodySmall as="p">
          {
            // TRANSLATORS: First paragraph of the licences view.
            messages.pgettext(
              'licenses-view',
              'This app is free software: you can share it and change it under the terms of the GNU General Public License, version 3 or any later version. The full text is below.',
            )
          }
        </BodySmall>
        <BodySmall as="p">
          {
            // TRANSLATORS: Credit to the project the app is derived from.
            messages.pgettext(
              'licenses-view',
              'It is based on the Mullvad VPN app, copyright Mullvad VPN AB, released under the same license.',
            )
          }
        </BodySmall>
        <BodySmall as="p">
          {
            // TRANSLATORS: Licence of the separate tunnel engine the app runs.
            messages.pgettext(
              'licenses-view',
              'The tunnel engine, warrenguard, is released under the GNU Affero General Public License, version 3.',
            )
          }
        </BodySmall>
      </FlexColumn>
      <FlexColumn>
        <SourceLink
          label={
            // TRANSLATORS: Link to the source code of the exact version that is installed.
            messages.pgettext('licenses-view', 'Source code of this version')
          }
          onClick={openAppSource}
          disabled={linksDisabled}
        />
        <SourceLink
          label={
            // TRANSLATORS: Link to the source code of the tunnel engine of this version.
            messages.pgettext('licenses-view', 'Source code of the tunnel engine')
          }
          onClick={openEngineSource}
          disabled={linksDisabled}
        />
      </FlexColumn>
      {licenseText !== '' && <LicenseText>{licenseText}</LicenseText>}
    </FlexColumn>
  );
}

export function LicensesView() {
  const { pop } = useHistory();
  const { openUrl } = useAppContext();
  const isOffline = useSelector((state) => state.connection.isBlocked);
  const [licenseText, setLicenseText] = useState('');

  useEffect(() => {
    let mounted = true;
    window.ipc.app
      .getLicenseText()
      .then((text) => {
        if (mounted) {
          setLicenseText(text);
        }
      })
      .catch((e: unknown) => log.error(`Failed to read the license text: ${(e as Error).message}`));
    return () => {
      mounted = false;
    };
  }, []);

  const openAppSource = useCallback(() => openUrl(builtAppSourceUrl), [openUrl]);
  const openEngineSource = useCallback(() => openUrl(builtEngineSourceUrl), [openUrl]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader title={licensesTitle()} />
          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <TitleBig as="h1">{licensesTitle()}</TitleBig>
                <LicensesContent
                  licenseText={licenseText}
                  openAppSource={openAppSource}
                  openEngineSource={openEngineSource}
                  linksDisabled={isOffline}
                />
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
