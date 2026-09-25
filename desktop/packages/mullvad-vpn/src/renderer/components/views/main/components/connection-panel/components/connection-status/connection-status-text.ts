import { TunnelState } from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import type { ConnectionPhase } from '../../../../../../../lib/connection-phase';

export function getConnectionStatusLabelText(tunnelState: TunnelState, phase: ConnectionPhase) {
  if (phase === 'interrupted') {
    // TRANSLATORS: Bold status title shown when the tunnel is up but the
    // TRANSLATORS: device has no internet connection.
    return messages.pgettext('tunnel-control', 'Connection interrupted');
  }
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

// `includeOnly`: only the chosen apps use the VPN, so "protected" would
// claim the whole device.
export function getConnectionStatusSubtitle(
  tunnelState: TunnelState,
  phase: ConnectionPhase,
  includeOnly: boolean,
) {
  // The interrupted phase is a connected state too.
  if (includeOnly && tunnelState.state === 'connected') {
    // TRANSLATORS: Secondary line shown below the status title while only
    // TRANSLATORS: the apps chosen in "VPN only for" use the VPN.
    return messages.pgettext('tunnel-control', 'Only selected apps are protected');
  }
  if (phase === 'interrupted') {
    // Still true during the hold: the kill switch keeps everything
    // fail-closed while the daemon waits for the network to come back.
    // TRANSLATORS: Secondary line shown below the status title when protected.
    return messages.pgettext('tunnel-control', 'You are protected');
  }
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
