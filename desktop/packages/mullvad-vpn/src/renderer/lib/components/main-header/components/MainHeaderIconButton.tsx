import styled from 'styled-components';

import { IconButton, IconButtonProps } from '../../icon-button';
import { StyledIconButtonIcon } from '../../icon-button/components';
import { useMainHeaderTone } from '../MainHeaderContext';

// The outline glyphs carry less visual mass than the old filled ones (a stroke
// weighs less than a solid), so they take the full 32px large step to hold
// their own next to the wordmark. Done as a local override rather than a
// one-off entry in the shared size scale.
const HEADER_ICON_SIZE = '32px';

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
