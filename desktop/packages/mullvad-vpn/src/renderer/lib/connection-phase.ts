import { TunnelState } from '../../shared/daemon-rpc-types';
import { Colors, colors } from './foundations';

// The connect screen collapses the daemon tunnel states into four visual phases,
// each with its own accent colour. This is the single source of truth so the
// backdrop wash, the eye icon, the status label and the action button never
// drift apart.
//   exposed    : leaking, traffic in the clear (red)
//   connecting : tunnel coming up or down (orange)
//   protected  : tunnel up (green)
//   blocked    : kill switch active, nothing leaks but nothing flows (neutral)
export type ConnectionPhase = 'exposed' | 'connecting' | 'protected' | 'blocked';

export function getConnectionPhase(tunnelState: TunnelState): ConnectionPhase {
  switch (tunnelState.state) {
    case 'connected':
      return 'protected';
    case 'connecting':
    case 'disconnecting':
      return 'connecting';
    case 'disconnected':
      // Locked down = kill switch holding traffic, not raw exposure.
      return tunnelState.lockedDown ? 'blocked' : 'exposed';
    case 'error':
      // blockingError = the daemon failed to install the block, so traffic may
      // be leaking (exposed). Without it the kill switch holds: secured but
      // offline (blocked). An error is never "connected".
      return tunnelState.details.blockingError ? 'exposed' : 'blocked';
  }
}

export function getPhaseAccentColor(phase: ConnectionPhase): string {
  return colors[getPhaseAccentColorName(phase)];
}

// Same accent as a colour-token name, for APIs (like <Icon color>) that take a
// token key rather than a resolved value.
export function getPhaseAccentColorName(phase: ConnectionPhase): Colors {
  switch (phase) {
    case 'protected':
      return 'green';
    case 'connecting':
      return 'orange';
    case 'exposed':
      return 'red';
    case 'blocked':
      return 'white';
  }
}
