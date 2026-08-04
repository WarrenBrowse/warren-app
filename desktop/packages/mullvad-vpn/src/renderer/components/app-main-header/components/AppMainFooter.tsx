import styled from 'styled-components';

import { Flex, MainHeaderToneProvider } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import { AppMainHeaderPubKey } from './AppMainHeaderPubKey';
import { AppMainHeaderTimeLeft } from './AppMainHeaderTimeLeft';

// The footer carries its own surface instead of a scrim over the artwork. A
// gradient only works when the art behind it is dark, and the scenery bottom
// edge ranges from dark grass to near-white watercolor paper, so the account
// address and the remaining time faded out entirely on half the plates. The
// same smoked glass as the connection panel keeps the two readings legible on
// every landscape and reads as the same material as the card above it.
const StyledFooter = styled.footer`
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 7px 16px;
  background-color: ${colors.blackAlpha60};
  backdrop-filter: blur(10px);
  border-top: 1px solid ${colors.whiteAlpha20};
`;

export function AppMainFooter() {
  return (
    <StyledFooter>
      <MainHeaderToneProvider value="light">
        <Flex justifyContent="space-between" alignItems="center" gap="small" flexGrow={1}>
          <AppMainHeaderPubKey />
          <AppMainHeaderTimeLeft />
        </Flex>
      </MainHeaderToneProvider>
    </StyledFooter>
  );
}
