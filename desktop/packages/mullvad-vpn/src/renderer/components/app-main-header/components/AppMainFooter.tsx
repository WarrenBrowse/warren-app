import styled from 'styled-components';

import { Flex, MainHeaderToneProvider } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import { AppMainHeaderPubKey } from './AppMainHeaderPubKey';
import { AppMainHeaderTimeLeft } from './AppMainHeaderTimeLeft';

// The pubkey and time-left sit over the dark, busy bottom of the scenery (the
// burrow foreground), where plain text is hard to read. Each is set on its own
// frosted, fully-rounded glass pill so it stays legible without darkening the
// whole footer, keeping the light-text-on-dark-glass language of the panel.
const StyledFooter = styled.footer`
  flex-shrink: 0;
  padding: 6px 16px 10px;
`;

const Pill = styled.div`
  display: inline-flex;
  align-items: center;
  min-width: 0;
  max-width: 100%;
  padding: 3px 10px;
  border-radius: 999px;
  background-color: ${colors.blackAlpha50};
  backdrop-filter: blur(8px);

  /* The pubkey/time-left labels render null when absent (no account, expired);
     collapse the empty pill so no stray chip is left floating. */
  &:empty {
    display: none;
  }
`;

export function AppMainFooter() {
  return (
    <StyledFooter>
      <MainHeaderToneProvider value="light">
        <Flex justifyContent="space-between" alignItems="center" gap="small">
          <Pill>
            <AppMainHeaderPubKey />
          </Pill>
          <Pill>
            <AppMainHeaderTimeLeft />
          </Pill>
        </Flex>
      </MainHeaderToneProvider>
    </StyledFooter>
  );
}
