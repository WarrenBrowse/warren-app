import React from 'react';
import styled from 'styled-components';

import { Icon } from '../../../../../../../lib/components';
import { getConnectionPhase, getPhaseCardColors } from '../../../../../../../lib/connection-phase';
import { useExitEgressDead } from '../../../../../../../lib/exit-egress';
import { FontFamilies, surfaces } from '../../../../../../../lib/foundations';
import { useHostOffline } from '../../../../../../../lib/host-offline';
import { useSelector } from '../../../../../../../redux/store';
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
  gap: '1px',
  flexGrow: 1,
  minWidth: 0,
});

// The flag hugs the end edge in EVERY state so it never appears to move; the
// expand chevron (only present when expandable) slots in before it instead.
const StyledTrailing = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  flexShrink: 0,
});

// The eye sits in a filled well rather than bare on the surface. The well gives
// the phase colour a shape to live in, which is what carries the state at a
// glance; the title then only has to be readable, not loud.
const StyledIconWell = styled.div<{ $well: string }>((props) => ({
  display: 'grid',
  placeItems: 'center',
  flexShrink: 0,
  width: '34px',
  height: '34px',
  borderRadius: '8px',
  backgroundColor: props.$well,
  transition: 'background-color 300ms ease-out',
}));

const StyledEye = styled(Icon)<{ $color: string }>((props) => ({
  backgroundColor: props.$color,
}));

const StyledTitle = styled.span<{ $color: string }>((props) => ({
  fontFamily: FontFamilies.openSans,
  fontSize: '16px',
  fontWeight: 600,
  lineHeight: '19.2px',
  color: props.$color,
}));

const StyledSubtitle = styled.span({
  fontFamily: FontFamilies.openSans,
  fontSize: '11.5px',
  fontWeight: 400,
  lineHeight: '15px',
  color: surfaces.textSecondary,
});

export type ConnectionStatusProps = {
  // Drawn before the flag, in the same row: the card's expand chevron.
  trailing?: React.ReactNode;
};

export function ConnectionStatus({ trailing }: ConnectionStatusProps) {
  const tunnelState = useSelector((state) => state.connection.status);
  const hostOffline = useHostOffline();
  const exitEgressDead = useExitEgressDead();
  const includeOnly = useSelector(
    (state) => state.settings.appRouting.splitMode === 'include-only',
  );

  const phase = getConnectionPhase(tunnelState, hostOffline, exitEgressDead);
  const { title, well } = getPhaseCardColors(phase);
  // A crossed-out eye ("hide") reads as protected/hidden in the burrow (secured,
  // blocked, or the interrupted hold where the kill switch keeps everything
  // fail-closed); an open eye ("show") reads as exposed/visible.
  const eyeIcon =
    phase === 'protected' || phase === 'blocked' || phase === 'interrupted' ? 'hide' : 'show';
  const subtitle = getConnectionStatusSubtitle(tunnelState, phase, includeOnly);

  return (
    <StyledRow role="status">
      <StyledIconWell $well={well}>
        <StyledEye icon={eyeIcon} size="small" $color={title} />
      </StyledIconWell>
      <StyledTextColumn>
        <StyledTitle $color={title}>{getConnectionStatusLabelText(tunnelState, phase)}</StyledTitle>
        {subtitle ? <StyledSubtitle>{subtitle}</StyledSubtitle> : null}
      </StyledTextColumn>
      <StyledTrailing>
        {trailing}
        <CurrentCountryFlag />
      </StyledTrailing>
    </StyledRow>
  );
}
