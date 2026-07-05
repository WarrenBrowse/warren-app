import styled from 'styled-components';

import { TunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import { getConnectionPhase, getPhaseAccentColor } from '../../../../../../../lib/connection-phase';
import { colors } from '../../../../../../../lib/foundations';
import { getReduceMotion } from '../../../../../../../lib/functions';
import { useSelector } from '../../../../../../../redux/store';
import { largeText, smallText } from '../../../../../../common-styles';
import { StatusEye } from './StatusEye';

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

const StyledTitle = styled.span<{ $color: string }>(largeText, (props) => ({
  color: props.$color,
  lineHeight: '20px',
}));

const StyledSubtitle = styled.span(smallText, {
  color: colors.whiteAlpha60,
  fontWeight: '400',
});

export function ConnectionStatus() {
  const tunnelState = useSelector((state) => state.connection.status);

  const phase = getConnectionPhase(tunnelState.state);
  const color = getConnectionSTatusLabelColor(tunnelState);
  const animate = !getReduceMotion();

  return (
    <StyledRow role="status">
      <StatusEye color={color} closed={phase === 'protected'} animate={animate} />
      <StyledTextColumn>
        <StyledTitle $color={color}>{getConnectionStatusLabelText(tunnelState)}</StyledTitle>
        <StyledSubtitle>{getConnectionStatusSubtitle(tunnelState)}</StyledSubtitle>
      </StyledTextColumn>
    </StyledRow>
  );
}

function getConnectionSTatusLabelColor(tunnelState: TunnelState) {
  const phase = getConnectionPhase(tunnelState.state);
  // A locked-down disconnected state is a deliberate block, not raw exposure, so
  // it stays neutral rather than shouting red.
  if (tunnelState.state === 'disconnected' && tunnelState.lockedDown) {
    return colors.white;
  }
  return getPhaseAccentColor(phase);
}

function getConnectionStatusLabelText(tunnelState: TunnelState) {
  switch (tunnelState.state) {
    case 'connected':
      // TRANSLATORS: Bold status title shown when the tunnel is up.
      return messages.pgettext('tunnel-control', 'Connection established');
    case 'connecting':
    case 'disconnecting':
    case 'disconnected':
      // TRANSLATORS: Bold status title shown when traffic is not protected.
      return messages.pgettext('tunnel-control', 'You are visible');
    case 'error':
      return tunnelState.details.blockingError
        ? messages.gettext('FAILED TO SECURE CONNECTION')
        : messages.gettext('BLOCKED CONNECTION');
  }
}

function getConnectionStatusSubtitle(tunnelState: TunnelState) {
  switch (tunnelState.state) {
    case 'connected':
      // TRANSLATORS: Secondary line shown below the status title when protected.
      return messages.pgettext('tunnel-control', 'You are protected');
    case 'connecting':
    case 'disconnecting':
      // TRANSLATORS: Secondary line shown while the encrypted tunnel is coming up.
      return messages.pgettext('tunnel-control', 'Encrypting your connection');
    case 'disconnected':
      // TRANSLATORS: Secondary line shown when traffic is not encrypted.
      return messages.pgettext('tunnel-control', 'Your connection is not encrypted');
    case 'error':
      return '';
  }
}
