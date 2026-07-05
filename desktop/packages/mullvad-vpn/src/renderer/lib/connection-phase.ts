import { TunnelState } from '../../shared/daemon-rpc-types';
import { colors } from './foundations';

// The connect screen collapses the five daemon tunnel states into three visual
// phases, each with its own accent colour. This is the single source of truth so
// the backdrop wash, the eye icon, the status label and the action button never
// drift apart.
export type ConnectionPhase = 'exposed' | 'connecting' | 'protected';

export function getConnectionPhase(state: TunnelState['state']): ConnectionPhase {
  switch (state) {
    case 'connected':
      return 'protected';
    case 'connecting':
    case 'disconnecting':
      return 'connecting';
    case 'disconnected':
    case 'error':
      return 'exposed';
  }
}

export function getPhaseAccentColor(phase: ConnectionPhase): string {
  switch (phase) {
    case 'protected':
      return colors.green;
    case 'connecting':
      return colors.orange;
    case 'exposed':
      return colors.red;
  }
}
