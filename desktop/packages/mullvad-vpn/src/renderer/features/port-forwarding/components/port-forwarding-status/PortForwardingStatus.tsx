import React from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../shared/gettext';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { usePortForwarding } from '../../hooks';

/**
 * Live readout of the NAT-PMP refresh-loop status.
 *
 * Renders one of:
 * - `Disabled`: the row is hidden entirely (nothing to show).
 * - `Requesting`: "Requesting port mapping..." placeholder.
 * - `Mapped { externalPort, lifetimeRemainingSecs }`: the
 *   user-facing public port + a live mm:ss countdown to the next
 *   renewal (lifetime / 2 of the granted lifetime).
 * - `Failed { errorMessage }`: red error block with the underlying
 *   `request_map` failure string.
 */
export function PortForwardingStatus() {
  const { status } = usePortForwarding();

  // Bookkeeping for the live countdown: store the wall-clock instant
  // at which the last status snapshot arrived so we can derive
  // `remaining = lifetime / 2 - elapsed` on every tick.
  const [now, setNow] = React.useState(() => Date.now());
  const lastSnapshotAt = React.useRef(Date.now());
  React.useEffect(() => {
    lastSnapshotAt.current = Date.now();
    setNow(Date.now());
  }, [status]);

  React.useEffect(() => {
    if (status.state !== 'mapped') {
      return undefined;
    }
    const intervalId = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(intervalId);
  }, [status]);

  if (status.state === 'disabled') {
    return null;
  }

  if (status.state === 'requesting') {
    return (
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="whiteAlpha60">
          {messages.pgettext('port-forwarding-view', 'Requesting port mapping...')}
        </Text>
      </FlexColumn>
    );
  }

  if (status.state === 'failed') {
    return (
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="red">
          {sprintf(messages.pgettext('port-forwarding-view', 'Mapping failed: %(error)s'), {
            error: status.errorMessage,
          })}
        </Text>
      </FlexColumn>
    );
  }

  // status.state === 'mapped'
  const renewAtMs = lastSnapshotAt.current + Math.floor((status.lifetimeRemainingSecs * 1000) / 2);
  const remainingMs = Math.max(0, renewAtMs - now);
  const remainingSecs = Math.floor(remainingMs / 1000);
  const mm = Math.floor(remainingSecs / 60)
    .toString()
    .padStart(2, '0');
  const ss = (remainingSecs % 60).toString().padStart(2, '0');

  return (
    <FlexColumn gap="small">
      <Text variant="labelTiny" color="whiteAlpha60">
        {sprintf(messages.pgettext('port-forwarding-view', 'Public port: %(port)s'), {
          port: status.externalPort,
        })}
      </Text>
      <Text variant="labelTiny" color="whiteAlpha60">
        {sprintf(messages.pgettext('port-forwarding-view', 'Renewing in %(countdown)s'), {
          countdown: `${mm}:${ss}`,
        })}
      </Text>
    </FlexColumn>
  );
}
