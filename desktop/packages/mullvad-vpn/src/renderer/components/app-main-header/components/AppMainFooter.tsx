import styled from 'styled-components';

import { Flex, MainHeaderToneProvider } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import { AppMainHeaderPubKey } from './AppMainHeaderPubKey';
import { AppMainHeaderTimeLeft } from './AppMainHeaderTimeLeft';

// The pubkey and time-left sit over the dark, busy bottom of the scenery. Rather
// than framing them in chips, we lay a soft bottom-up gradient scrim so the light
// text reads without any visible chrome, blending into the landscape. A small
// text shadow guarantees legibility over the brighter patches of grass.
const StyledFooter = styled.footer`
  flex-shrink: 0;
  min-height: 54px;
  display: flex;
  align-items: flex-end;
  padding: 0 18px 10px;
  background: linear-gradient(
    to top,
    ${colors.blackAlpha60} 0%,
    ${colors.blackAlpha40} 45%,
    transparent 100%
  );
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.55);
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
