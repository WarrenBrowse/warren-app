import styled from 'styled-components';

import { TunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import { Icon } from '../../../../../../../lib/components';
import {
  getConnectionPhase,
  getPhaseAccentColorName,
} from '../../../../../../../lib/connection-phase';
import { Colors, colors } from '../../../../../../../lib/foundations';
import { useSelector } from '../../../../../../../redux/store';
import { largeText, smallText } from '../../../../../../common-styles';

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
  const colorName = getStatusColorName(tunnelState);
  // An open eye ("show") reads as exposed/visible; the crossed-out eye ("hide")
  // reads as protected/hidden in the burrow.
  const eyeIcon = phase === 'protected' ? 'hide' : 'show';

  return (
    <StyledRow role="status">
      <Icon icon={eyeIcon} color={colorName} size="large" />
      <StyledTextColumn>
        <StyledTitle $color={colors[colorName]}>
          {getConnectionStatusLabelText(tunnelState)}
        </StyledTitle>
        <StyledSubtitle>{getConnectionStatusSubtitle(tunnelState)}</StyledSubtitle>
      </StyledTextColumn>
    </StyledRow>
  );
}

function getStatusColorName(tunnelState: TunnelState): Colors {
  // A locked-down disconnected state is a deliberate block, not raw exposure, so
  // it stays neutral rather than shouting red.
  if (tunnelState.state === 'disconnected' && tunnelState.lockedDown) {
    return 'white';
  }
  return getPhaseAccentColorName(getConnectionPhase(tunnelState.state));
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
