import styled from 'styled-components';

import { IconButton, IconButtonProps } from '../../icon-button';
import { StyledIconButtonIcon } from '../../icon-button/components';
import { useMainHeaderTone } from '../MainHeaderContext';

// A touch bigger than the 24px medium default, but short of the 32px large step,
// so the account/settings icons carry over the artwork without dominating. Done
// as a local override rather than a one-off entry in the shared size scale.
const HEADER_ICON_SIZE = '28px';

const StyledMainHeaderIconButton = styled(IconButton)`
  && {
    width: ${HEADER_ICON_SIZE};
    height: ${HEADER_ICON_SIZE};
  }
  ${StyledIconButtonIcon} {
    width: ${HEADER_ICON_SIZE};
    height: ${HEADER_ICON_SIZE};
  }
`;

export const MainHeaderIconButton = (props: IconButtonProps) => {
  const tone = useMainHeaderTone();
  // Primary (solid) rather than the softer secondary: over the bright scenery the
  // account/settings icons need to read as crisp flat black, per the mockups.
  return <StyledMainHeaderIconButton variant="primary" tone={tone} {...props} />;
};
