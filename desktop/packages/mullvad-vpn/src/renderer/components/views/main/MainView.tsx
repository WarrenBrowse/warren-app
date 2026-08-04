import styled from 'styled-components';

import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { AppMainHeader } from '../../app-main-header';
import { BetaBadge } from '../../beta-badge';
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

// The notification cards and the beta marker both belong at the top of the
// content area. They used to be positioned independently, which made the
// marker sit ON TOP of any notification that appeared (the marker assumed
// the cards were right-aligned; they are full width). One stack instead:
// the cards keep their place and the marker flows below them.
const StyledTopStack = styled(FlexColumn)`
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

// Left-aligned under whatever the stack holds above it, so it never covers a
// notification. Renders nothing outside beta builds.
const StyledBetaBadge = styled.div`
  align-self: flex-start;
  margin: 16px 0 0 16px;
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
          <StyledTopStack>
            <NotificationArea />
            <StyledBetaBadge>
              <BetaBadge variant="overlay" />
            </StyledBetaBadge>
          </StyledTopStack>
          <StyledMain>
            <ConnectionPanel />
          </StyledMain>
        </StyledContent>
        <AppMainHeader.Footer />
      </Foreground>
    </View>
  );
}
