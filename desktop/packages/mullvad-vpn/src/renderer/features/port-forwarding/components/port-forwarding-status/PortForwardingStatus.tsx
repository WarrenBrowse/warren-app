import React from 'react';
import { sprintf } from 'sprintf-js';

import { NatPmpErrorReason } from '../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../shared/gettext';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { formatCountdown, useNatPmpPortBlock, usePortForwarding } from '../../hooks';

/**
 * Live readout of the NAT-PMP refresh-loop status.
 *
 * Renders one block in every state — the previous design returned
 * `null` when the live cache said `disabled`, which left users
 * staring at a toggle they had just flipped on with no feedback that
 * anything was happening (or that anything was expected of them). Now:
 *
 * - `Disabled` AND toggle OFF: still null (the user opted out, no
 *   need to show anything).
 * - `Disabled` AND toggle ON: "Inactive — disconnect and reconnect to
 *   activate". The daemon does NOT live-renegotiate the mapping when
 *   the toggle flips; the `NatPmpManager` is spawned once at tunnel
 *   start, so changes only take effect on the next reconnect.
 * - `Requesting`: "Status: requesting port mapping..." spinner-less
 *   placeholder.
 * - `Mapped { externalPort, lifetimeGrantedSecs }`: "Status: active"
 *   + the granted public port + a live mm:ss countdown to renewal
 *   (lifetime / 2 of the granted lifetime). The exit picks the port
 *   from its pool; when the user left the preferred-port input empty
 *   ("auto"), this is where they discover what was assigned.
 * - `Failed { errorMessage }`: red error block with the underlying
 *   `request_map` failure string.
 */
export function PortForwardingStatus() {
  const { settings, status } = usePortForwarding();
  // Shared rate-limit countdown / warning state (also drives the port
  // input's disabled state in `PortForwardingAdvanced`).
  const block = useNatPmpPortBlock();

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
    // The user has the toggle OFF — nothing to surface.
    if (!settings.enabled) {
      return null;
    }
    // Toggle ON but no mapping reported yet. With live reconfig
    // (M5.D.x) the daemon pre-sets the cache to `requesting` the
    // moment the toggle flips, so this transient `disabled` window
    // is brief. It can still appear when the tunnel is DOWN (the
    // controller task hasn't spawned because there's no tunnel to
    // map through) — in that case the honest message is "waiting
    // for the tunnel", NOT "reconnect" (reconnecting an already-up
    // tunnel is no longer required).
    return (
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="whiteAlpha60">
          {messages.pgettext(
            'port-forwarding-view',
            'Status: waiting for an active tunnel connection to set up the port mapping.',
          )}
        </Text>
      </FlexColumn>
    );
  }

  if (status.state === 'requesting') {
    return (
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="whiteAlpha60">
          {messages.pgettext('port-forwarding-view', 'Status: requesting port mapping...')}
        </Text>
      </FlexColumn>
    );
  }

  if (status.state === 'rate-limited') {
    // The exit banned further port changes for a short window. The
    // daemon retries automatically; surface a countdown so the user
    // knows exactly how long to wait (the port input is disabled in
    // tandem by `PortForwardingAdvanced`).
    return (
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="red">
          {block.remainingSecs > 0
            ? sprintf(
                messages.pgettext(
                  'port-forwarding-view',
                  'Too many port changes. Wait %(countdown)s before changing the port again.',
                ),
                { countdown: formatCountdown(block.remainingSecs) },
              )
            : messages.pgettext(
                'port-forwarding-view',
                'Too many port changes. Retrying the port mapping now…',
              )}
        </Text>
      </FlexColumn>
    );
  }

  if (status.state === 'failed') {
    return (
      <FlexColumn gap="small">
        <Text variant="labelTiny" color="red">
          {natPmpFailureMessage(
            status.errorReason,
            settings.suggestedExternalPort,
            status.errorMessage,
          )}
        </Text>
      </FlexColumn>
    );
  }

  // status.state === 'mapped'
  // Renewal fires at granted/2 (RFC 6886 §3.7), measured from when this
  // snapshot's Mapped/Renewed event arrived. `lifetimeGrantedSecs` is
  // the granted lifetime (static per event), not a decreasing remaining.
  const renewAtMs = lastSnapshotAt.current + Math.floor((status.lifetimeGrantedSecs * 1000) / 2);
  const remainingMs = Math.max(0, renewAtMs - now);
  const remainingSecs = Math.floor(remainingMs / 1000);
  const mm = Math.floor(remainingSecs / 60)
    .toString()
    .padStart(2, '0');
  const ss = (remainingSecs % 60).toString().padStart(2, '0');

  // When the user picked "auto" (suggested == 0) and the exit
  // granted a port, surface the assignment explicitly so the user
  // can copy the value into whichever app needs it (torrent,
  // Minecraft, …). When the user pinned a specific port and the
  // exit honoured it, the granted port and the suggestion match;
  // when the exit reassigned (suggestion was taken), the live
  // `externalPort` is the source of truth and the suggestion the
  // user typed is irrelevant.
  return (
    <FlexColumn gap="small">
      <Text variant="labelTiny" color="green">
        {messages.pgettext('port-forwarding-view', 'Status: active')}
      </Text>
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
      {block.reason === 'last-chance' ? (
        <Text variant="labelTiny" color="yellow">
          {messages.pgettext(
            'port-forwarding-view',
            'Last port change before a temporary block. Wait a moment before changing it again.',
          )}
        </Text>
      ) : null}
      {block.reason === 'budget-exhausted' ? (
        <Text variant="labelTiny" color="yellow">
          {sprintf(
            messages.pgettext(
              'port-forwarding-view',
              'Too many recent changes. You can change the port again in %(countdown)s.',
            ),
            { countdown: formatCountdown(block.remainingSecs) },
          )}
        </Text>
      ) : null}
    </FlexColumn>
  );
}

/**
 * Localised, reason-specific failure message. Keyed on the structured
 * `errorReason` from the daemon so the user sees an actionable sentence
 * (e.g. "port already in use, pick another") rather than the raw
 * English `errorMessage`. The raw string is only used as a last-resort
 * fallback for the uncategorised `unknown` case.
 */
function natPmpFailureMessage(
  reason: NatPmpErrorReason,
  suggestedPort: number,
  rawError: string,
): string {
  switch (reason) {
    case 'suggested-port-in-use':
      return suggestedPort > 0
        ? sprintf(
            messages.pgettext(
              'port-forwarding-view',
              'Port %(port)d is already in use. Choose another one.',
            ),
            { port: suggestedPort },
          )
        : messages.pgettext(
            'port-forwarding-view',
            'The requested port is already in use. Choose another one.',
          );
    case 'out-of-resources':
      return messages.pgettext(
        'port-forwarding-view',
        'No port is available right now. Try again in a moment.',
      );
    case 'not-authorized':
      return messages.pgettext(
        'port-forwarding-view',
        'Port forwarding is not allowed on this server.',
      );
    case 'unknown':
    default:
      return sprintf(messages.pgettext('port-forwarding-view', 'Mapping failed: %(error)s'), {
        error: rawError,
      });
  }
}
