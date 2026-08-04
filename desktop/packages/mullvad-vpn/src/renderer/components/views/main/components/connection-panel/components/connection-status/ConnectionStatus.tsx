import styled from 'styled-components';

import { TunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import { Icon } from '../../../../../../../lib/components';
import {
  ConnectionPhase,
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

// The flag hugs the right edge in EVERY state so it never appears to move; the
// expand chevron (only present when expandable) slots in on its left instead.
const StyledFlagSlot = styled.div({
  marginLeft: 'auto',
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

  const phase = getConnectionPhase(tunnelState, hostOffline, exitEgressDead);
  const colorName = getPhaseAccentColorName(phase);
  // A crossed-out eye ("hide") reads as protected/hidden in the burrow (secured,
  // blocked, or the interrupted hold where the kill switch keeps everything
  // fail-closed); an open eye ("show") reads as exposed/visible.
  const eyeIcon =
    phase === 'protected' || phase === 'blocked' || phase === 'interrupted' ? 'hide' : 'show';
  const subtitle = getConnectionStatusSubtitle(tunnelState, phase);

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

function getConnectionStatusLabelText(tunnelState: TunnelState, phase: ConnectionPhase) {
  if (phase === 'interrupted') {
    // TRANSLATORS: Bold status title shown when the tunnel is up but the
    // TRANSLATORS: device has no internet connection.
    return messages.pgettext('tunnel-control', 'Connection interrupted');
  }
  switch (tunnelState.state) {
    case 'connected':
      // TRANSLATORS: Bold status title shown when the tunnel is up.
      return messages.pgettext('tunnel-control', 'Connection established');
    case 'connecting':
    case 'disconnecting':
      // TRANSLATORS: Bold status title shown when traffic is not protected.
      return messages.pgettext('tunnel-control', 'You are visible');
    case 'disconnected':
      return tunnelState.lockedDown
        ? messages.gettext('BLOCKED CONNECTION')
        : messages.pgettext('tunnel-control', 'You are visible');
    case 'error':
      // Leaking (blockingError) reads like the exposed state; a held block reads
      // like the locked-down state. The banner carries the specific cause.
      return tunnelState.details.blockingError
        ? messages.pgettext('tunnel-control', 'You are visible')
        : messages.gettext('BLOCKED CONNECTION');
  }
}

function getConnectionStatusSubtitle(tunnelState: TunnelState, phase: ConnectionPhase) {
  if (phase === 'interrupted') {
    // Still true during the hold: the kill switch keeps everything
    // fail-closed while the daemon waits for the network to come back.
    // TRANSLATORS: Secondary line shown below the status title when protected.
    return messages.pgettext('tunnel-control', 'You are protected');
  }
  switch (tunnelState.state) {
    case 'connected':
      // TRANSLATORS: Secondary line shown below the status title when protected.
      return messages.pgettext('tunnel-control', 'You are protected');
    case 'connecting':
      // TRANSLATORS: Secondary line shown while the tunnel is coming up.
      return messages.pgettext('tunnel-control', 'Connection in progress');
    case 'disconnecting':
      // TRANSLATORS: Secondary line shown while the tunnel is being torn down.
      return messages.pgettext('tunnel-control', 'Disconnecting...');
    case 'disconnected':
      return tunnelState.lockedDown
        ? ''
        : // TRANSLATORS: Secondary line shown when traffic is not encrypted.
          messages.pgettext('tunnel-control', 'Your connection is not encrypted');
    case 'error':
      return tunnelState.details.blockingError
        ? messages.pgettext('tunnel-control', 'Your connection is not encrypted')
        : '';
  }
}
