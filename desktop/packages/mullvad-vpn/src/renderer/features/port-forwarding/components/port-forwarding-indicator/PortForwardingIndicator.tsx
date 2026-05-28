import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { NatPmpProto } from '../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../shared/gettext';
import { FeatureIndicator } from '../../../../lib/components';
import { usePortForwarding } from '../../hooks';

const StyledWrapper = styled.div({
  display: 'flex',
  marginTop: '8px',
});

/**
 * Home-screen badge shown while a NAT-PMP port forward is live. Reuses
 * the shared `<FeatureIndicator>` chip — same look as the "Local network
 * sharing" indicator — and surfaces the granted public port + protocol
 * so the user can read their forwarded port at a glance without opening
 * the settings view. Renders nothing unless the live status is `mapped`.
 */
export function PortForwardingIndicator() {
  const { status, settings } = usePortForwarding();

  if (status.state !== 'mapped') {
    return null;
  }

  const protocol = settings.protocol === NatPmpProto.tcp ? 'TCP' : 'UDP';
  const label = sprintf(
    // TRANSLATORS: Active-feature chip on the main screen, shown when
    // TRANSLATORS: port forwarding is active. Available placeholders:
    // TRANSLATORS: %(port)d - the granted public port (e.g. 53451)
    // TRANSLATORS: %(protocol)s - the transport protocol, "UDP" or "TCP"
    messages.pgettext('connect-view', 'Port forwarding: %(port)d %(protocol)s'),
    { port: status.externalPort, protocol },
  );

  return (
    <StyledWrapper>
      <FeatureIndicator>
        <FeatureIndicator.Text>{label}</FeatureIndicator.Text>
      </FeatureIndicator>
    </StyledWrapper>
  );
}
