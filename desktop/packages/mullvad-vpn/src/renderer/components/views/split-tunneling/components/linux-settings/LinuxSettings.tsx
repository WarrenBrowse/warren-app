import React, { useEffect } from 'react';

import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { useAppRouting } from '../../../../../features/app-routing/hooks';
import { Flex, Spinner } from '../../../../../lib/components';
import { FlexColumn } from '../../../../../lib/components/flex-column';
import { useAfterTransition } from '../../../../../lib/transition-hooks';
import { useEffectEvent } from '../../../../../lib/utility-hooks';
import { HeaderSubTitle } from '../../../../SettingsHeader';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { tabLabel } from '../app-routing-tabs';
import { ApplicationSearchBar } from '../application-search-bar';
import { ApplicationSearchNoResult } from '../application-search-no-result';
import { IncludeOnlyBanner } from '../include-only-banner';
import { TabHeader } from '../tab-header';
import {
  HeaderDescription,
  LaunchErrorDialog,
  LinuxApplicationList,
  OpenFilePickerButton,
  UnsupportedDialog,
} from './components';
import { useShowLinuxApplicationList, useShowNoSearchResult, useShowSpinner } from './hooks';
import {
  type LinuxLaunchMode,
  LinuxSettingsContextProvider,
  useLinuxSettingsContext,
} from './LinuxSettingsContext';

// Bypass VPN is launch based on Linux and has no mode to switch; VPN only for
// needs the mode on, since it turns the tunnel into an opt-in one.
function LinuxHeader() {
  const { launchMode, splitTunnelingSupported } = useLinuxSettingsContext();
  const { routing } = useAppRouting();
  const { requestSplitMode } = useSplitTunnelingContext();

  const setIncludeOnly = React.useCallback(
    (value: boolean) => requestSplitMode(value ? 'include-only' : 'off'),
    [requestSplitMode],
  );

  if (launchMode === 'exclude') {
    return (
      <TabHeader label={tabLabel('bypass')} description={<HeaderDescription />}>
        {routing.splitMode === 'include-only' && (
          <HeaderSubTitle role="note">
            {messages.pgettext(
              'split-tunneling-view',
              'VPN only for is on: every app you did not choose there already bypasses the VPN.',
            )}
          </HeaderSubTitle>
        )}
      </TabHeader>
    );
  }

  const checked = routing.splitMode === 'include-only';
  return (
    <TabHeader
      label={tabLabel('include-only')}
      description={<HeaderDescription />}
      checked={checked}
      disabled={!checked && splitTunnelingSupported === false}
      onCheckedChange={setIncludeOnly}>
      {checked && <IncludeOnlyBanner />}
    </TabHeader>
  );
}

function LinuxSettingsInner() {
  const { getSplitTunnelingSupported, getLinuxSplitTunnelingApplications } = useAppContext();
  const {
    splitTunnelingSupported,
    searchTerm,
    setApplications,
    setSearchTerm,
    setSplitTunnelingSupported,
  } = useLinuxSettingsContext();
  const runAfterTransition = useAfterTransition();
  const showLinuxApplicationList = useShowLinuxApplicationList();
  const showNoSearchResult = useShowNoSearchResult();
  const showSpinner = useShowSpinner();

  const onMount = useEffectEvent(() => {
    runAfterTransition(async () => {
      const splitTunnelingSupported = await getSplitTunnelingSupported();
      setSplitTunnelingSupported(splitTunnelingSupported);
    });

    runAfterTransition(async () => {
      const applications = await getLinuxSplitTunnelingApplications();
      setApplications(applications);
    });
  });

  // These lint rules are disabled for now because the react plugin for eslint does
  // not understand that useEffectEvent should not be added to the dependency array.
  // Enable these rules again when eslint can lint useEffectEvent properly.
  // eslint-disable-next-line react-compiler/react-compiler
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => void onMount(), []);

  return (
    <>
      <LinuxHeader />
      <ApplicationSearchBar
        disabled={!splitTunnelingSupported}
        searchTerm={searchTerm}
        onSearch={setSearchTerm}
      />
      {showNoSearchResult && <ApplicationSearchNoResult searchTerm={searchTerm} />}
      <FlexColumn gap="medium">
        {showLinuxApplicationList && <LinuxApplicationList />}
        {showSpinner && (
          <Flex justifyContent="center" margin={{ top: 'large' }}>
            <Spinner size="big" />
          </Flex>
        )}
        <Flex margin={{ horizontal: 'medium', bottom: 'large' }}>
          <OpenFilePickerButton />
        </Flex>
      </FlexColumn>
      <LaunchErrorDialog />
      <UnsupportedDialog />
    </>
  );
}

export function LinuxSettings({ launchMode }: { launchMode: LinuxLaunchMode }) {
  return (
    <LinuxSettingsContextProvider launchMode={launchMode}>
      <LinuxSettingsInner />
    </LinuxSettingsContextProvider>
  );
}
