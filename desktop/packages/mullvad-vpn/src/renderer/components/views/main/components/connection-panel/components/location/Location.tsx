import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { TunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages, relayLocations } from '../../../../../../../../shared/gettext';
import { colors } from '../../../../../../../lib/foundations';
import { useSelector } from '../../../../../../../redux/store';
import { largeText } from '../../../../../../common-styles';
import Marquee from '../../../../../../Marquee';
import { ConnectionPanelAccordion } from '../../../../styles';

const StyledLocation = styled.span(largeText, {
  color: colors.white,
  flexShrink: 0,
});

export function Location() {
  const connection = useSelector((state) => state.connection);
  const text = getLocationText(connection.status, connection.country, connection.city);

  // Only surface a location line once a tunnel is up or coming up (the exit
  // location). When disconnected the panel stays compact so more of the scenery
  // and Bula show, matching the art direction.
  const showLocation =
    connection.status.state === 'connected' || connection.status.state === 'connecting';

  return (
    <ConnectionPanelAccordion expanded={showLocation}>
      <StyledLocation>
        <Marquee>{text}</Marquee>
      </StyledLocation>
    </ConnectionPanelAccordion>
  );
}

function getLocationText(tunnelState: TunnelState, country?: string, city?: string): string {
  // The daemon reports English names; display them through the relay-locations
  // catalog like the location selector does, so the whole app speaks one language.
  country = country ? relayLocations.gettext(country) : '';
  city = city ? relayLocations.gettext(city) : city;

  switch (tunnelState.state) {
    case 'connected':
    case 'connecting':
      return city
        ? sprintf(
            // TRANSLATORS: The exit location under the connection status. Available placeholders:
            // TRANSLATORS: %(country)s - the country of the exit, e.g. Sweden
            // TRANSLATORS: %(city)s - its city, e.g. Gothenburg
            messages.pgettext('connect-view', '%(country)s, %(city)s'),
            { country, city },
          )
        : country;
    case 'disconnecting':
    case 'disconnected':
      return country;
    case 'error':
      return '';
  }
}
