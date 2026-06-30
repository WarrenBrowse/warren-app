import { useCallback } from 'react';
import styled from 'styled-components';

import { isMultihopTunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../../../shared/routes';
import { FeatureIndicator } from '../../../../../../../lib/components';
import { TransitionType, useHistory } from '../../../../../../../lib/history';
import { useSelector } from '../../../../../../../redux/store';

const StyledWrapper = styled.div({
  display: 'flex',
  marginTop: '8px',
});

/**
 * Home-screen badge shown while the active circuit is multi-hop (2 hops),
 * i.e. the relay node and the exit node are different nodes. Mirrors the
 * `<PortForwardingIndicator>` chip so the collapsed connection card and
 * the expanded connection-details panel surface the same information.
 * Multi-hop detection is shared via `isMultihopTunnelState` so both
 * surfaces agree; a 1-hop or single-hop circuit renders nothing.
 */
export function MultihopIndicator() {
  const tunnelState = useSelector((state) => state.connection.status);
  const history = useHistory();
  // Clicking the chip opens the multi-hop settings screen, mirroring the
  // upstream feature chips (e.g. connection sharing -> VPN settings). Defined
  // before the early return so the hook order stays stable.
  const openSettings = useCallback(
    () => history.push(RoutePath.warrenMultiHopSettings, { transition: TransitionType.show }),
    [history],
  );

  if (!isMultihopTunnelState(tunnelState)) {
    return null;
  }

  return (
    <StyledWrapper>
      <FeatureIndicator onClick={openSettings}>
        <FeatureIndicator.Text>
          {messages.pgettext('connection-info', 'Multihop (2 hops)')}
        </FeatureIndicator.Text>
      </FeatureIndicator>
    </StyledWrapper>
  );
}
