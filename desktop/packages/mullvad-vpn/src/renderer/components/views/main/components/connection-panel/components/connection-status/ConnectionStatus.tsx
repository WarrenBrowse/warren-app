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

// Right-aligned; leaves room for the expand chevron when the card is
// expandable (connected/connecting), which sits absolutely at the top right.
const StyledFlagSlot = styled.div<{ $chevronRoom: boolean }>((props) => ({
  marginLeft: 'auto',
  display: 'flex',
  alignItems: 'center',
  marginRight: props.$chevronRoom ? '36px' : 0,
}));

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

  const chevronRoom = tunnelState.state === 'connected' || tunnelState.state === 'connecting';

  return (
    <StyledRow role="status">
      <Icon icon={eyeIcon} color={colorName} size="large" />
      <StyledTextColumn>
        <StyledTitle $color={colors[colorName]}>
          {getConnectionStatusLabelText(tunnelState)}
        </StyledTitle>
        {subtitle ? <StyledSubtitle>{subtitle}</StyledSubtitle> : null}
      </StyledTextColumn>
      <StyledFlagSlot $chevronRoom={chevronRoom}>
        <CurrentCountryFlag />
      </StyledFlagSlot>
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
      // Leaking (blockingError) reads like the exposed state; a held block reads
      // like the locked-down state. The banner carries the specific cause.
      return tunnelState.details.blockingError
        ? messages.pgettext('tunnel-control', 'You are visible')
        : messages.gettext('BLOCKED CONNECTION');
  }
}

function getConnectionStatusSubtitle(tunnelState: TunnelState) {
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
