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
  AddAppScreen,
  AppRulesSection,
  CountryScreen,
  DefaultRouteSection,
  RouteScreen,
} from './components/app-routing';
import { useFullDiskAccessCheck } from './components/split-tunneling-settings/hooks';
import { SplitTunnelingSettingsContextProvider } from './components/split-tunneling-settings/SplitTunnelingSettingsContext';
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

function RulesList() {
  const { pop } = useHistory();
  const { scrollbarsRef } = useSplitTunnelingContext();
  const title = messages.pgettext('split-tunneling-view', 'App routing');

  return (
    <BackAction action={pop}>
      <NavigationContainer>
        <AppNavigationHeader title={title} />
        <StyledNavigationScrollbars ref={scrollbarsRef}>
          <View.Content>
            <SettingsHeader>
              <HeaderTitle>{title}</HeaderTitle>
              <HeaderSubTitle>
                {messages.pgettext('split-tunneling-view', 'Choose where each app goes.')}
              </HeaderSubTitle>
            </SettingsHeader>
            <DefaultRouteSection />
            <AppRulesSection />
          </View.Content>
        </StyledNavigationScrollbars>
      </NavigationContainer>
    </BackAction>
  );
}

// App routing: one list of rules, one route per app, and what the other apps
// do. The add, route and country screens take the whole window in turn.
function SplitTunnelingInner() {
  const { browsing, screen } = useSplitTunnelingContext();
  useFullDiskAccessCheck();

  return (
    <>
      <StyledPageCover $show={browsing} />
      <View backgroundColor="darkBlue">
        {screen === 'list' && <RulesList />}
        {screen === 'add' && <AddAppScreen />}
        {screen === 'route' && <RouteScreen />}
        {screen === 'country' && <CountryScreen />}
      </View>
    </>
  );
}

export function SplitTunnelingView() {
  return (
    <SplitTunnelingSettingsContextProvider>
      <SplitTunnelingContextProvider>
        <SplitTunnelingInner />
      </SplitTunnelingContextProvider>
    </SplitTunnelingSettingsContextProvider>
  );
}
