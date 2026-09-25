import styled from 'styled-components';

import { messages } from '../../../../../../shared/gettext';
import { Icon } from '../../../../../lib/components';
import { colors, spacings } from '../../../../../lib/foundations';
import { smallNormalText } from '../../../../common-styles';

const StyledBanner = styled.div({
  ...smallNormalText,
  display: 'flex',
  gap: spacings.small,
  alignItems: 'flex-start',
  padding: `${spacings.small} ${spacings.medium} ${spacings.small} ${spacings.small}`,
  borderRadius: '8px',
  border: `1px solid color-mix(in srgb, ${colors.yellow} 50%, transparent)`,
  backgroundColor: `color-mix(in srgb, ${colors.yellow} 10%, transparent)`,
  color: colors.white,
});

// Stays for as long as include-only is on: the rest of the device is not
// protected, and that should never be a surprise.
export function IncludeOnlyBanner() {
  return (
    <StyledBanner role="status" data-testid="include-only-banner">
      <Icon icon="alert-circle" size="small" color="yellow" aria-hidden />
      <span>
        {messages.pgettext(
          'split-tunneling-view',
          'Only these apps are protected. Everything else on this device uses your normal connection.',
        )}
      </span>
    </StyledBanner>
  );
}
