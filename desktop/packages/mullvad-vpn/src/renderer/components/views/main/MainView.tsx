import styled from 'styled-components';

import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { AppMainHeader } from '../../app-main-header';
import CountryBackdrop from '../../CountryBackdrop';
import NotificationArea from '../../NotificationArea';
import { ConnectionPanel } from './components';

// Everything except the backdrop lives in this layer, above the full-bleed
// scenery.
const Foreground = styled(FlexColumn)`
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
`;

const StyledContent = styled(FlexColumn)`
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
`;

const StyledNotificationArea = styled(NotificationArea)`
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  z-index: 1;
`;

const StyledMain = styled.main`
  display: flex;
  flex-direction: column;
  flex: 1;
  max-height: 100%;
`;

export function MainView() {
  return (
    <View style={{ position: 'relative' }}>
      <CountryBackdrop />
      <Foreground>
        <AppMainHeader size="1" variant="transparent" tone="dark">
          <AppMainHeader.AccountButton />
          <AppMainHeader.SettingsButton />
        </AppMainHeader>
        <StyledContent>
          <StyledNotificationArea />
          <StyledMain>
            <ConnectionPanel />
          </StyledMain>
        </StyledContent>
        <AppMainHeader.Footer />
      </Foreground>
    </View>
  );
}
