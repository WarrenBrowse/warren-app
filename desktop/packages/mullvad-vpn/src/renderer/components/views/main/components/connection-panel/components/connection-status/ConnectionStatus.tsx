import styled from 'styled-components';

import { TunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import { Icon } from '../../../../../../../lib/components';
import {
  getConnectionPhase,
  getPhaseAccentColorName,
} from '../../../../../../../lib/connection-phase';
import { colors } from '../../../../../../../lib/foundations';
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

  const phase = getConnectionPhase(tunnelState);
  const colorName = getPhaseAccentColorName(phase);
  // A crossed-out eye ("hide") reads as protected/hidden in the burrow (secured
  // or blocked); an open eye ("show") reads as exposed/visible.
  const eyeIcon = phase === 'protected' || phase === 'blocked' ? 'hide' : 'show';
  const subtitle = getConnectionStatusSubtitle(tunnelState);

  return (
    <StyledRow role="status">
      <Icon icon={eyeIcon} color={colorName} size="large" />
      <StyledTextColumn>
        <StyledTitle $color={colors[colorName]}>
          {getConnectionStatusLabelText(tunnelState)}
        </StyledTitle>
        {subtitle ? <StyledSubtitle>{subtitle}</StyledSubtitle> : null}
      </StyledTextColumn>
    </StyledRow>
  );
}

function getConnectionStatusLabelText(tunnelState: TunnelState) {
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
      return tunnelState.details.blockingError
        ? messages.gettext('FAILED TO SECURE CONNECTION')
        : messages.pgettext('tunnel-control', 'Connection established');
  }
}

function getConnectionStatusSubtitle(tunnelState: TunnelState) {
  switch (tunnelState.state) {
    case 'connected':
      // TRANSLATORS: Secondary line shown below the status title when protected.
      return messages.pgettext('tunnel-control', 'You are protected');
    case 'connecting':
      // TRANSLATORS: Secondary line shown while the encrypted tunnel is coming up.
      return messages.pgettext('tunnel-control', 'Encrypting your connection');
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
        ? ''
        : messages.pgettext('tunnel-control', 'You are protected');
  }
}
