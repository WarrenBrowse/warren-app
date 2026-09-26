import React from 'react';
import styled from 'styled-components';

import { resolveApplications, sameAppId } from '../../../../../../shared/app-routing';
import { ISplitTunnelingApplication } from '../../../../../../shared/application-types';
import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { CountryFlag } from '../../../../../features/app-routing/components';
import { useAppRouting } from '../../../../../features/app-routing/hooks';
import { Button, Flex, Spinner } from '../../../../../lib/components';
import { colors, spacings } from '../../../../../lib/foundations';
import { Container, Section, SectionTitle } from '../../../../cell';
import { normalText, tinyText } from '../../../../common-styles';
import { useFilePicker, useRoutingApplications } from '../../hooks';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { includesSearchTerm } from '../../utils';
import { ApplicationIcon } from '../application-icon';
import { ApplicationList } from '../application-list';
import { ApplicationRow } from '../application-row';
import { ApplicationSearchBar } from '../application-search-bar';
import { ApplicationSearchNoResult } from '../application-search-no-result';
import { SplitModeHeader } from '../split-tunneling-settings/components';
import {
  useFullDiskAccessCheck,
  useSplitModeAvailability,
} from '../split-tunneling-settings/hooks';
import { SplitTunnelingSettingsContextProvider } from '../split-tunneling-settings/SplitTunnelingSettingsContext';
import { getFilePickerOptionsForPlatform } from '../split-tunneling-settings/utils';

const StyledViaCountryRow = styled(Container)({
  backgroundColor: colors.blue40,
  minHeight: '56px',
});

const StyledText = styled.div({
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  paddingInlineEnd: spacings.small,
});

const StyledName = styled.span({
  ...normalText,
  color: colors.white,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});

const StyledNote = styled.span({
  ...tinyText,
  fontWeight: 400,
  color: colors.whiteAlpha60,
});

// An app with a country in force is in the VPN in this mode too: choosing a
// country for an app is enough to put it there. It is managed from its tab.
function ViaCountryRow({
  application,
  country,
}: {
  application: ISplitTunnelingApplication;
  country: string;
}) {
  return (
    <StyledViaCountryRow>
      <ApplicationIcon icon={application.icon} />
      <StyledText>
        <StyledName>{application.name}</StyledName>
        <StyledNote>
          {messages.pgettext('split-tunneling-view', 'Uses the VPN through its country')}
        </StyledNote>
      </StyledText>
      <CountryFlag country={country} />
    </StyledViaCountryRow>
  );
}

function IncludeOnlyInner() {
  const {
    routing,
    applications: metadata,
    platform,
    addIncludedApp,
    removeIncludedApp,
  } = useAppRouting();
  const { forgetManuallyAddedSplitTunnelingApplication } = useAppContext();
  const { applications: catalog, reload } = useRoutingApplications();
  const { setBrowsing, scrollbarsRef } = useSplitTunnelingContext();
  const availability = useSplitModeAvailability();
  const [searchTerm, setSearchTerm] = React.useState('');
  useFullDiskAccessCheck();

  const canEdit = availability === 'available';
  const matches = React.useCallback(
    (application: ISplitTunnelingApplication) => includesSearchTerm(application, searchTerm),
    [searchTerm],
  );

  const included = React.useMemo(
    () =>
      resolveApplications(routing.includedApps, metadata, catalog ?? [], platform)
        .map((application) => ({ ...application, deletable: false }))
        .filter(matches),
    [catalog, matches, metadata, platform, routing.includedApps],
  );

  const viaCountry = React.useMemo(() => {
    if (!routing.appExitsEnabled) return [];
    const exits = routing.appExits.filter(
      (entry) => !routing.includedApps.some((app) => sameAppId(app, entry.app, platform)),
    );
    return resolveApplications(
      exits.map((entry) => entry.app),
      metadata,
      catalog ?? [],
      platform,
    )
      .map((application, index) => ({
        application: { ...application, deletable: false },
        country: exits[index].exit.country,
      }))
      .filter(({ application }) => matches(application));
  }, [catalog, matches, metadata, platform, routing]);

  const others = React.useMemo(
    () =>
      catalog
        ?.filter(
          (application) =>
            !routing.includedApps.some((app) =>
              sameAppId(app, application.absolutepath, platform),
            ) &&
            !viaCountry.some((entry) =>
              sameAppId(entry.application.absolutepath, application.absolutepath, platform),
            ),
        )
        .filter(matches),
    [catalog, matches, platform, routing.includedApps, viaCountry],
  );

  const add = React.useCallback(
    (application: ISplitTunnelingApplication) => void addIncludedApp(application),
    [addIncludedApp],
  );
  const remove = React.useCallback(
    (application: ISplitTunnelingApplication) => void removeIncludedApp(application.absolutepath),
    [removeIncludedApp],
  );
  const forget = React.useCallback(
    async (application: ISplitTunnelingApplication) => {
      await forgetManuallyAddedSplitTunnelingApplication(application);
      await reload();
    },
    [forgetManuallyAddedSplitTunnelingApplication, reload],
  );

  const renderOtherRow = React.useCallback(
    (application: ISplitTunnelingApplication) => (
      <ApplicationRow
        application={application}
        onAdd={add}
        onDelete={application.deletable ? forget : undefined}
      />
    ),
    [add, forget],
  );

  const pickFile = useFilePicker(
    messages.pgettext('split-tunneling-view', 'Add'),
    setBrowsing,
    async (filePath: string) => {
      scrollbarsRef.current?.scrollToTop(true);
      await addIncludedApp(filePath);
      await reload();
    },
    getFilePickerOptionsForPlatform(),
  );

  const hasIncluded = included.length > 0 || viaCountry.length > 0;
  const showNoResult = searchTerm !== '' && !hasIncluded && (others?.length ?? 0) === 0;

  return (
    <>
      <SplitModeHeader mode="include-only" />
      {canEdit && (
        <>
          <ApplicationSearchBar searchTerm={searchTerm} onSearch={setSearchTerm} disableAutoFocus />
          {showNoResult && <ApplicationSearchNoResult searchTerm={searchTerm} />}
          <Flex flexDirection="column" gap="medium">
            {hasIncluded && (
              <Section
                sectionTitle={
                  <SectionTitle>
                    {messages.pgettext('split-tunneling-view', 'Using the VPN')}
                  </SectionTitle>
                }>
                <Flex flexDirection="column" data-testid="included-applications">
                  {included.map((application) => (
                    <ApplicationRow
                      key={application.absolutepath}
                      application={application}
                      onRemove={remove}
                    />
                  ))}
                  {viaCountry.map(({ application, country }) => (
                    <ViaCountryRow
                      key={application.absolutepath}
                      application={application}
                      country={country}
                    />
                  ))}
                </Flex>
              </Section>
            )}
            {others === undefined ? (
              <Flex justifyContent="center" margin={{ top: 'large' }}>
                <Spinner size="big" />
              </Flex>
            ) : (
              others.length > 0 && (
                <Section
                  sectionTitle={
                    <SectionTitle>
                      {messages.pgettext('split-tunneling-view', 'All apps')}
                    </SectionTitle>
                  }>
                  <ApplicationList
                    data-testid="not-included-applications"
                    applications={others}
                    rowRenderer={renderOtherRow}
                  />
                </Section>
              )
            )}
            <Flex flexDirection="column" margin={{ horizontal: 'medium', bottom: 'large' }}>
              <Button onClick={pickFile}>
                <Button.Text>
                  {messages.pgettext('split-tunneling-view', 'Find another app')}
                </Button.Text>
              </Button>
            </Flex>
          </Flex>
        </>
      )}
    </>
  );
}

export function IncludeOnlySettings() {
  return (
    <SplitTunnelingSettingsContextProvider>
      <IncludeOnlyInner />
    </SplitTunnelingSettingsContextProvider>
  );
}
