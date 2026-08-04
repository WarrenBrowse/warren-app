import { useCallback } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { RoutePath } from '../../../../../shared/routes';
import { FeatureIndicator } from '../../../../lib/components';
import { TransitionType, useHistory } from '../../../../lib/history';
import { useSelector } from '../../../../redux/store';
import { usePortForwarding } from '../../hooks';
import { protocolLabel } from '../../mapping';

const StyledWrapper = styled.div({
  display: 'flex',
});

/**
 * Home-screen badge for NAT-PMP port forwarding. Reuses the shared
 * `<FeatureIndicator>` chip and clicking it opens the port-forwarding
 * settings (mirroring the upstream feature chips). It surfaces:
 * - the granted public port(s) + protocol while at least one mapping is
 *   live (default chip), or
 * - an alerting RED chip when a mapping is blocked/in conflict (so the
 *   user notices the problem on the main screen without opening settings).
 * Renders nothing only when there is no live and no blocked mapping.
 */
export function PortForwardingIndicator() {
  const { mappings } = usePortForwarding();
  const tunnelState = useSelector((state) => state.connection.status.state);
  const history = useHistory();

  const openSettings = useCallback(
    () => history.push(RoutePath.portForwardingSettings, { transition: TransitionType.show }),
    [history],
  );

  // The mapping list keeps its last state across a disconnect, so without a
  // tunnel the chip would advertise a port that no longer forwards anything.
  if (tunnelState !== 'connected') {
    return null;
  }

  const open = mappings.flatMap((m) =>
    m.status.state === 'mapped' ? [{ port: m.status.externalPort, protocol: m.protocol }] : [],
  );
  const blocked = mappings.filter((m) => m.status.state === 'failed');

  // Error state takes priority over the success listing: a blocked port
  // forward must be visible and alarming even when no other mapping is live.
  if (blocked.length > 0) {
    const portInUse = blocked.some(
      (m) => m.status.state === 'failed' && m.status.errorReason === 'suggested-port-in-use',
    );
    const label = portInUse
      ? // TRANSLATORS: Alerting chip on the main screen when the requested
        // TRANSLATORS: public port is already taken on the connected exit.
        messages.pgettext('connect-view', 'Port forwarding: port in use')
      : // TRANSLATORS: Alerting chip on the main screen when a port forward
        // TRANSLATORS: could not be established.
        messages.pgettext('connect-view', 'Port forwarding: blocked');
    return (
      <StyledWrapper>
        <FeatureIndicator variant="error" onClick={openSettings}>
          <FeatureIndicator.Text>{label}</FeatureIndicator.Text>
        </FeatureIndicator>
      </StyledWrapper>
    );
  }

  if (open.length === 0) {
    return null;
  }

  const ports = open.map((m) => `${m.port} ${protocolLabel(m.protocol)}`).join(', ');
  const label = sprintf(
    // TRANSLATORS: Active-feature chip on the main screen, shown when
    // TRANSLATORS: port forwarding is active. Available placeholders:
    // TRANSLATORS: %(ports)s - comma-separated "port PROTO" list
    messages.pgettext('connect-view', 'Port forwarding: %(ports)s'),
    { ports },
  );

  return (
    <StyledWrapper>
      <FeatureIndicator onClick={openSettings}>
        <FeatureIndicator.Text>{label}</FeatureIndicator.Text>
      </FeatureIndicator>
    </StyledWrapper>
  );
}
