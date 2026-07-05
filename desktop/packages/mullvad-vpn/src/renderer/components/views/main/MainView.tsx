import styled from 'styled-components';

import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { AppMainHeader } from '../../app-main-header';
import CountryBackdrop from '../../CountryBackdrop';
import NotificationArea from '../../NotificationArea';
import { ConnectionPanel } from './components';

const StyledContent = styled(FlexColumn)`
  position: relative;
  overflow: hidden;
`;

const StyledMapOverlay = styled(FlexColumn)`
  position: relative;
  z-index: 1;
  max-height: 100%;
`;

const StyledNotificationArea = styled(NotificationArea)`
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
`;

const StyledMain = styled.main`
  display: flex;
  flex-direction: column;
  flex: 1;
  max-height: 100%;
`;

export function MainView() {
  return (
    <View>
      <AppMainHeader size="basedOnLoginStatus" variant="basedOnConnectionStatus">
        <AppMainHeader.AccountButton />
        <AppMainHeader.SettingsButton />
      </AppMainHeader>
      <StyledContent flexGrow={1}>
        <CountryBackdrop />
        <StyledMapOverlay flexGrow={1}>
          <StyledNotificationArea />
          <StyledMain>
            <ConnectionPanel />
          </StyledMain>
        </StyledMapOverlay>
      </StyledContent>
    </View>
  );
}
