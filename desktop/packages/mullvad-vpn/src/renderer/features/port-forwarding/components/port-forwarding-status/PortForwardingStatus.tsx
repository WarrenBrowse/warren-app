import { messages } from '../../../../../shared/gettext';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { usePortForwarding } from '../../hooks';

/**
 * Top-level NAT-PMP status hint. The per-rule live status (public port,
 * requesting, failed) is rendered inline on each row in
 * `PortForwardingAdvanced`; this component only surfaces the one
 * whole-feature condition the rows cannot express on their own:
 *
 * - Port forwarding is ON and the user has at least one rule, but NO
 *   mapping has come back yet - typically because the tunnel is DOWN (the
 *   daemon's in-tunnel controller only maps through an active tunnel). The
 *   honest message is "waiting for the tunnel", not "reconnect" (live
 *   reconfig means an already-up tunnel never needs a reconnect).
 *
 * In every other case it renders nothing.
 */
export function PortForwardingStatus() {
  const { settings, rules, mappings } = usePortForwarding();

  if (!settings.enabled || rules.length === 0 || mappings.length > 0) {
    return null;
  }

  return (
    <FlexColumn gap="small">
      <Text variant="labelTiny" color="whiteAlpha60">
        {messages.pgettext(
          'port-forwarding-view',
          'Status: waiting for an active tunnel connection to set up the port mappings.',
        )}
      </Text>
    </FlexColumn>
  );
}
