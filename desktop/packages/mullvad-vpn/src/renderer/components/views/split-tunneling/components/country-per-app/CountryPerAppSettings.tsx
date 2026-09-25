import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import {
  appExitFor,
  appRouteLine,
  exitChoicesInUse,
  MAX_APP_EXITS,
  resolveApplications,
  wouldExceedAppExitLimit,
} from '../../../../../../shared/app-routing';
import { ISplitTunnelingApplication } from '../../../../../../shared/application-types';
import { ExitChoice } from '../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../shared/gettext';
import { CountryPickerDialog } from '../../../../../features/app-routing/components';
import { useAppRouting, useExitChoiceNames } from '../../../../../features/app-routing/hooks';
import { appExitLimitText } from '../../../../../features/app-routing/strings';
import { Button, Flex, Spinner } from '../../../../../lib/components';
import { Section, SectionTitle } from '../../../../cell';
import { HeaderSubTitle } from '../../../../SettingsHeader';
import { useFilePicker, useRoutingApplications } from '../../hooks';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { includesSearchTerm } from '../../utils';
import { tabLabel } from '../app-routing-tabs';
import { ApplicationSearchBar } from '../application-search-bar';
import { ApplicationSearchNoResult } from '../application-search-no-result';
import { getFilePickerOptionsForPlatform } from '../split-tunneling-settings/utils';
import { TabHeader } from '../tab-header';
import { AppCountryRow } from './AppCountryRow';

const StyledList = styled.div({
  display: 'flex',
  flexDirection: 'column',
});

type Picking = {
  application: ISplitTunnelingApplication | string;
  id: string;
  name: string;
};

function basename(filePath: string) {
  return filePath.split(/[\\/]/).pop() || filePath;
}

export function CountryPerAppSettings() {
  const {
    routing,
    statuses,
    applications: metadata,
    platform,
    setAppExitsEnabled,
    setAppExit,
    clearAppExit,
  } = useAppRouting();
  const { applications: catalog, reload } = useRoutingApplications();
  const { setBrowsing, scrollbarsRef } = useSplitTunnelingContext();
  const exitNames = useExitChoiceNames();
  const [searchTerm, setSearchTerm] = React.useState('');
  const [picking, setPicking] = React.useState<Picking>();
  const [refused, setRefused] = React.useState(false);

  const exitLabel = React.useCallback(
    (exit: ExitChoice) => {
      const { country, city } = exitNames(exit);
      return city === undefined ? country : `${city}, ${country}`;
    },
    [exitNames],
  );

  const routed = React.useMemo(
    () =>
      resolveApplications(
        routing.appExits.map((entry) => entry.app),
        metadata,
        catalog ?? [],
        platform,
      )
        .map((application) => ({ ...application, deletable: false }))
        .filter((application) => includesSearchTerm(application, searchTerm))
        .sort((a, b) => a.name.localeCompare(b.name)),
    [catalog, metadata, platform, routing.appExits, searchTerm],
  );

  const others = React.useMemo(
    () =>
      (catalog ?? [])
        .filter(
          (application) => appExitFor(routing, application.absolutepath, platform) === undefined,
        )
        .filter((application) => includesSearchTerm(application, searchTerm))
        .sort((a, b) => a.name.localeCompare(b.name)),
    [catalog, platform, routing, searchTerm],
  );

  const pick = React.useCallback((application: ISplitTunnelingApplication) => {
    setRefused(false);
    setPicking({ application, id: application.absolutepath, name: application.name });
  }, []);

  const pickFile = useFilePicker(
    messages.pgettext('split-tunneling-view', 'Choose'),
    setBrowsing,
    (filePath: string) => {
      setRefused(false);
      setPicking({ application: filePath, id: filePath, name: basename(filePath) });
    },
    getFilePickerOptionsForPlatform(),
  );

  const choose = React.useCallback(
    async (exit: ExitChoice) => {
      if (picking === undefined) return;
      setPicking(undefined);
      const outcome = await setAppExit(picking.application, exit);
      if (outcome?.result === 'limit-reached') {
        setRefused(true);
        return;
      }
      // Choosing a country is asking for it: a switched off tab would make
      // the choice do nothing.
      if (outcome?.result === 'ok' && !routing.appExitsEnabled) {
        await setAppExitsEnabled(true);
      }
      if (typeof picking.application === 'string') {
        scrollbarsRef.current?.scrollToTop(true);
        await reload();
      }
    },
    [picking, reload, routing.appExitsEnabled, scrollbarsRef, setAppExit, setAppExitsEnabled],
  );

  const removePicked = React.useCallback(async () => {
    if (picking === undefined) return;
    setPicking(undefined);
    await clearAppExit(picking.id);
  }, [clearAppExit, picking]);

  const clearApplication = React.useCallback(
    (application: ISplitTunnelingApplication) => void clearAppExit(application.absolutepath),
    [clearAppExit],
  );

  const onPickerOpenChange = React.useCallback((open: boolean) => {
    if (!open) {
      setPicking(undefined);
    }
  }, []);

  const pickedExit = picking ? appExitFor(routing, picking.id, platform) : undefined;
  const isBlocked = React.useCallback(
    (exit: ExitChoice) =>
      picking !== undefined &&
      wouldExceedAppExitLimit(routing.appExits, picking.id, exit, platform),
    [picking, platform, routing.appExits],
  );

  const showNoResult = searchTerm !== '' && routed.length === 0 && others.length === 0;

  return (
    <>
      <TabHeader
        label={tabLabel('countries')}
        description={sprintf(
          // TRANSLATORS: Description of the "Country per app" tab.
          // TRANSLATORS: Available placeholders:
          // TRANSLATORS: %(limit)d - how many countries apps can use at once
          messages.pgettext(
            'split-tunneling-view',
            'Choose the country an app appears from. Other apps keep your main connection. Up to %(limit)d countries at a time.',
          ),
          { limit: MAX_APP_EXITS },
        )}
        checked={routing.appExitsEnabled}
        onCheckedChange={setAppExitsEnabled}>
        {refused && <HeaderSubTitle role="alert">{appExitLimitText()}</HeaderSubTitle>}
      </TabHeader>

      <ApplicationSearchBar searchTerm={searchTerm} onSearch={setSearchTerm} disableAutoFocus />
      {showNoResult && <ApplicationSearchNoResult searchTerm={searchTerm} />}

      <Flex flexDirection="column" gap="medium">
        {routed.length > 0 && (
          <Section
            sectionTitle={
              <SectionTitle>
                {messages.pgettext('split-tunneling-view', 'With a country')}
              </SectionTitle>
            }>
            <StyledList data-testid="apps-with-country">
              {routed.map((application) => {
                const exit = appExitFor(routing, application.absolutepath, platform);
                return (
                  <AppCountryRow
                    key={application.absolutepath}
                    application={application}
                    exit={exit}
                    exitLabel={exit ? exitLabel(exit) : undefined}
                    line={appRouteLine(routing, statuses, application.absolutepath, platform)}
                    onPick={pick}
                    onClear={clearApplication}
                  />
                );
              })}
            </StyledList>
          </Section>
        )}

        {catalog === undefined ? (
          <Flex justifyContent="center" margin={{ top: 'large' }}>
            <Spinner size="big" />
          </Flex>
        ) : (
          others.length > 0 && (
            <Section
              sectionTitle={
                <SectionTitle>{messages.pgettext('split-tunneling-view', 'All apps')}</SectionTitle>
              }>
              <StyledList data-testid="apps-without-country">
                {others.map((application) => (
                  <AppCountryRow
                    key={application.absolutepath}
                    application={application}
                    onPick={pick}
                  />
                ))}
              </StyledList>
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

      {picking && (
        <CountryPickerDialog
          open
          onOpenChange={onPickerOpenChange}
          applicationName={picking.name}
          current={pickedExit}
          exitsInUse={exitChoicesInUse(routing.appExits)}
          isBlocked={isBlocked}
          onSelect={choose}
          onRemove={pickedExit ? removePicked : undefined}
        />
      )}
    </>
  );
}
