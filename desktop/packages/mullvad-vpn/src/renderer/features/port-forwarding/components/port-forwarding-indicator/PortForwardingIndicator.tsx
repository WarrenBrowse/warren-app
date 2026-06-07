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
 * Home-screen badge shown while at least one NAT-PMP port forward is
 * live. Reuses the shared `<FeatureIndicator>` chip and lists the granted
 * public port(s) + protocol so the user can read their forwarded ports at
 * a glance. Renders nothing unless at least one mapping is active.
 */
export function PortForwardingIndicator() {
  const { mappings } = usePortForwarding();

  const open = mappings.flatMap((m) =>
    m.status.state === 'mapped' ? [{ port: m.status.externalPort, protocol: m.protocol }] : [],
  );

  if (open.length === 0) {
    return null;
  }

  const ports = open
    .map((m) => `${m.port} ${m.protocol === NatPmpProto.tcp ? 'TCP' : 'UDP'}`)
    .join(', ');
  const label = sprintf(
    // TRANSLATORS: Active-feature chip on the main screen, shown when
    // TRANSLATORS: port forwarding is active. Available placeholders:
    // TRANSLATORS: %(ports)s - comma-separated "port PROTO" list
    messages.pgettext('connect-view', 'Port forwarding: %(ports)s'),
    { ports },
  );

  return (
    <StyledWrapper>
      <FeatureIndicator>
        <FeatureIndicator.Text>{label}</FeatureIndicator.Text>
      </FeatureIndicator>
    </StyledWrapper>
  );
}
