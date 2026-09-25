import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import SettingsHeader, { HeaderSubTitle, HeaderTitle } from '../../SettingsHeader';
import {
  AppRoutingTabs,
  CountryPerAppSettings,
  IncludeOnlySettings,
  LinuxSettings,
  ModeChangeDialog,
  Settings,
  tabId,
  tabPanelId,
} from './components';
import { SplitTunnelingContextProvider, useSplitTunnelingContext } from './SplitTunnelingContext';

const StyledPageCover = styled.div<{ $show: boolean }>((props) => ({
  position: 'absolute',
  zIndex: 2,
  top: 0,
  left: 0,
  right: 0,
  bottom: 0,
  opacity: 0.5,
  display: props.$show ? 'block' : 'none',
}));

const StyledNavigationScrollbars = styled(NavigationScrollbars)({
  flex: 1,
});

function TabPanel() {
  const { tab } = useSplitTunnelingContext();
  const linux = window.env.platform === 'linux';

  let content;
  switch (tab) {
    case 'bypass':
      content = linux ? <LinuxSettings launchMode="exclude" /> : <Settings />;
      break;
    case 'countries':
      content = <CountryPerAppSettings />;
      break;
    case 'include-only':
      content = linux ? <LinuxSettings launchMode="include" /> : <IncludeOnlySettings />;
      break;
  }

  return (
    <div role="tabpanel" id={tabPanelId(tab)} aria-labelledby={tabId(tab)}>
      {content}
    </div>
  );
}

function SplitTunnelingInner() {
  const { pop } = useHistory();
  const { browsing, scrollbarsRef } = useSplitTunnelingContext();
  const title = messages.pgettext('split-tunneling-view', 'App routing');

  return (
    <>
      <StyledPageCover $show={browsing} />
      <View backgroundColor="darkBlue">
        <BackAction action={pop}>
          <NavigationContainer>
            <AppNavigationHeader title={title} />
            <StyledNavigationScrollbars ref={scrollbarsRef}>
              <View.Content>
                <SettingsHeader>
                  <HeaderTitle>{title}</HeaderTitle>
                  <HeaderSubTitle>
                    {messages.pgettext('split-tunneling-view', 'Choose how each app connects.')}
                  </HeaderSubTitle>
                </SettingsHeader>
                <AppRoutingTabs />
                <TabPanel />
              </View.Content>
            </StyledNavigationScrollbars>
          </NavigationContainer>
        </BackAction>
      </View>
      <ModeChangeDialog />
    </>
  );
}

export function SplitTunnelingView() {
  return (
    <SplitTunnelingContextProvider>
      <SplitTunnelingInner />
    </SplitTunnelingContextProvider>
  );
}
