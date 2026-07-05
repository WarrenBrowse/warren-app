import styled from 'styled-components';

import { Flex, MainHeaderToneProvider } from '../../../lib/components';
import { AppMainHeaderPubKey } from './AppMainHeaderPubKey';
import { AppMainHeaderTimeLeft } from './AppMainHeaderTimeLeft';

// The pubkey and time-left moved out of the header into a footer over the bottom
// of the scenery, where the mockups place them. They sit on bright grass, so the
// tone is dark. Reuses the same tone-aware labels the header used.
const StyledFooter = styled.footer`
  flex-shrink: 0;
  padding: 6px 18px 10px;
`;

export function AppMainFooter() {
  return (
    <StyledFooter>
      <MainHeaderToneProvider value="dark">
        <Flex justifyContent="space-between" alignItems="center">
          <AppMainHeaderPubKey />
          <AppMainHeaderTimeLeft />
        </Flex>
      </MainHeaderToneProvider>
    </StyledFooter>
  );
}
