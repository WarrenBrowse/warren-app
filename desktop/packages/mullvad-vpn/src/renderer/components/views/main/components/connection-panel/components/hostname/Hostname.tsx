import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../../../../shared/gettext';
import { FontFamilies, surfaces } from '../../../../../../../lib/foundations';
import { IConnectionReduxState } from '../../../../../../../redux/connection/reducers';
import { useSelector } from '../../../../../../../redux/store';
import Marquee from '../../../../../../Marquee';
import { StyledExitLoadBadge, StyledExitLoadPart } from '../../../../../../network-stats';
import { ConnectionPanelAccordion } from '../../../../styles';
import { ConnectedExitLoad } from '../connected-exit-load';

const StyledAccordion = styled(ConnectionPanelAccordion)({
  flexShrink: 0,
});

// The load badge takes the card's text colours; its ring takes the phase
// colours of the card, so a quiet exit reads in the same green as the title.
const StyledHostnameRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  paddingTop: '2px',
  '--load-ring-low': surfaces.protected,
  '--load-ring-moderate': surfaces.pill,
  '--load-ring-high': surfaces.connecting,
  '--load-ring-saturated': surfaces.exposed,
  '--load-ring-track': `color-mix(in srgb, ${surfaces.textMuted} 30%, transparent)`,

  [`${StyledExitLoadBadge}`]: {
    gap: '7px',
    color: surfaces.textSecondary,
    fontSize: '11px',
    lineHeight: '15px',
  },
  [`${StyledExitLoadPart}`]: {
    gap: '3px',
  },
});

const StyledHostname = styled.span({
  fontFamily: FontFamilies.openSans,
  fontSize: '12px',
  fontWeight: 400,
  lineHeight: '16.5px',
  color: surfaces.textMuted,
  flex: '1 1 auto',
  minWidth: 0,
  minHeight: '1em',
});

// A flex line is exactly its text's height: the marquee's inline-block, aligned
// in a line box of its own, added a pixel and a half under every line.
const StyledMarquee = styled(Marquee)({
  display: 'flex',
});

export function Hostname() {
  const tunnelState = useSelector((state) => state.connection.status.state);
  const connection = useSelector((state) => state.connection);
  const text = getHostnameText(connection);

  return (
    <StyledAccordion expanded={tunnelState === 'connecting' || tunnelState === 'connected'}>
      <StyledHostnameRow>
        <StyledHostname data-testid="hostname-line">
          <StyledMarquee>{text}</StyledMarquee>
        </StyledHostname>
        {tunnelState === 'connected' && <ConnectedExitLoad />}
      </StyledHostnameRow>
    </StyledAccordion>
  );
}

function getHostnameText(connection: IConnectionReduxState) {
  let hostname = '';
  if (connection.hostname && connection.entryHostname) {
    hostname = sprintf(
      // TRANSLATORS: The hostname line displayed below the country on the main screen
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(relay)s - the relay hostname
      // TRANSLATORS: %(entry)s - the entry relay hostname
      messages.pgettext('connection-info', '%(relay)s via %(entry)s'),
      {
        relay: connection.hostname,
        entry: connection.entryHostname,
      },
    );
  } else if (connection.hostname) {
    hostname = connection.hostname;
  }

  return hostname;
}
