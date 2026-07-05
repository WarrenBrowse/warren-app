import { TunnelState } from '../../shared/daemon-rpc-types';
import { Colors, colors } from './foundations';

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
  }
}
