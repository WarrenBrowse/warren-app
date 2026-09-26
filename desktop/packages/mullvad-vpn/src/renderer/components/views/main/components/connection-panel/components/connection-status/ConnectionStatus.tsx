import styled from 'styled-components';

import { Icon } from '../../../../../../../lib/components';
import {
  getConnectionPhase,
  getPhaseAccentColorName,
  getPhaseTitleColorName,
} from '../../../../../../../lib/connection-phase';
import { useExitEgressDead } from '../../../../../../../lib/exit-egress';
import { colors } from '../../../../../../../lib/foundations';
import { useHostOffline } from '../../../../../../../lib/host-offline';
import { useSelector } from '../../../../../../../redux/store';
import { largeText, smallText } from '../../../../../../common-styles';
import { CurrentCountryFlag } from '../../../../../../CurrentCountryFlag';
import {
  getConnectionStatusLabelText,
  getConnectionStatusSubtitle,
} from './connection-status-text';

const StyledRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
});

const StyledTextColumn = styled.div({
  display: 'flex',
  flexDirection: 'column',
  minWidth: 0,
});

// The flag hugs the end edge in EVERY state so it never appears to move; the
// expand chevron (only present when expandable) slots in before it instead.
const StyledFlagSlot = styled.div({
  marginInlineStart: 'auto',
  display: 'flex',
  alignItems: 'center',
});

// The eye sits in a tinted well rather than bare on the surface. It gives the
// phase colour a filled shape to live in, which is what carries the state at a
// glance; the title then only has to be readable, not loud.
const StyledIconWell = styled.div<{ $accent: string }>((props) => ({
  display: 'grid',
  placeItems: 'center',
  flexShrink: 0,
  width: '36px',
  height: '36px',
  borderRadius: '11px',
  backgroundColor: `color-mix(in srgb, ${props.$accent} 22%, transparent)`,
  border: `1px solid color-mix(in srgb, ${props.$accent} 45%, transparent)`,
  transition: 'background-color 300ms ease-out, border-color 300ms ease-out',
}));

const StyledTitle = styled.span<{ $color: string }>(largeText, (props) => ({
  color: props.$color,
  fontSize: '19px',
  lineHeight: '22px',
}));

const StyledSubtitle = styled.span(smallText, {
  color: colors.whiteAlpha80,
  fontSize: '13px',
  lineHeight: '18px',
  fontWeight: '400',
});

export function ConnectionStatus() {
  const tunnelState = useSelector((state) => state.connection.status);
  const hostOffline = useHostOffline();
  const exitEgressDead = useExitEgressDead();
  const includeOnly = useSelector(
    (state) => state.settings.appRouting.splitMode === 'include-only',
  );

  const phase = getConnectionPhase(tunnelState, hostOffline, exitEgressDead);
  const colorName = getPhaseAccentColorName(phase);
  // A crossed-out eye ("hide") reads as protected/hidden in the burrow (secured,
  // blocked, or the interrupted hold where the kill switch keeps everything
  // fail-closed); an open eye ("show") reads as exposed/visible.
  const eyeIcon =
    phase === 'protected' || phase === 'blocked' || phase === 'interrupted' ? 'hide' : 'show';
  const subtitle = getConnectionStatusSubtitle(tunnelState, phase, includeOnly);

  return (
    <StyledRow role="status">
      <StyledIconWell $accent={colors[colorName]}>
        <Icon icon={eyeIcon} color={colorName} size="small" />
      </StyledIconWell>
      <StyledTextColumn>
        <StyledTitle $color={colors[getPhaseTitleColorName(phase)]}>
          {getConnectionStatusLabelText(tunnelState, phase)}
        </StyledTitle>
        {subtitle ? <StyledSubtitle>{subtitle}</StyledSubtitle> : null}
      </StyledTextColumn>
      <StyledFlagSlot>
        <CurrentCountryFlag />
      </StyledFlagSlot>
    </StyledRow>
  );
}
